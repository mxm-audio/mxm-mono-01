//! The keyboard cursor, driven through mxm-mono-01's real editor.
//!
//! mxm-kit's `crates/ui` tests prove the cursor's arithmetic against a synthetic registry. This
//! proves the half that only exists once a real panel has drawn: that controls register themselves,
//! that the cursor reaches them, that VALUE and an arrow move the selected parameter by its own
//! step, and that a held key is one automation gesture rather than a hundred.
//!
//! The keys are the keyboard language's (design system §11, every editor since 2026-10-08), in the
//! default keymap: a bare arrow moves the cursor inside the card, VIEW and an arrow card to card
//! (2026-10-09), and a step key and an arrow, or VALUE's, edit the parameter the cursor is on —
//! FINE, COARSE or MICRO — and OUT (`Tab`) or letting go of a held key keeps the edit. A key tapped in sequence and a key held
//! in a chord are the same gesture, so most tests tap.
//!
//! Layout is not asserted here. Which card a knob lands on depends on the width the pack chose,
//! and `editor.rs`'s own paging test owns that. What is asserted is where the cursor **starts**:
//! on the first card's first parameter, the LFO's rate, and not on the app bar's output level,
//! which is drawn before any card (design system §3.1 item 6 puts it there; the owner, 2026-10-07,
//! of the language's pilot: it started on Output).

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use egui::Key;
use mxm_mono_01::editor::sections::{ASSIGNMENT, binding_for};
use mxm_mono_01::editor::{PresetUi, panel, set_advanced};
use mxm_mono_01::params::MxmMono01Params;
use mxm_mono_01::telemetry::Telemetry;
use mxm_plugin_test::keyboard_checks::{COARSE, OUT, VALUE, VIEW, key_of};
use nice_plug::params::{Param, internals::ParamPtr};
use nice_plug::prelude::{ParamSetter, PluginApi, PluginState};

/// The cursor reports a `String`; `binding_for` wants the permanent `&'static str` the parameter
/// was declared with. `ASSIGNMENT` is the list of those, so resolving through it also asserts the
/// cursor never invented an id.
fn permanent(id: &str) -> &'static str {
    ASSIGNMENT
        .iter()
        .map(|(id, _, _)| *id)
        .find(|known| *known == id)
        .expect("the cursor landed on a declared parameter")
}

/// A host that applies what the editor reports **and keeps the bracketing**, so a test can ask
/// both "did the value move" and "was the gesture balanced" — the second being the one that is
/// silent when it breaks.
///
/// **It can also lag**, holding every set back until told to apply it: CLAP applies an editor's
/// writes on the audio clock, not the editor's, and a keyboard law that chains from the readback
/// repeats or drops steps while it is behind. Every value sent is logged either way.
#[derive(Default)]
struct Recorder {
    begins: AtomicUsize,
    sets: AtomicUsize,
    ends: AtomicUsize,
    lagging: AtomicBool,
    pending: Mutex<Vec<Pending>>,
    sent: Mutex<Vec<f32>>,
}

/// One set a lagging host has not applied yet.
struct Pending(ParamPtr, f32);

// SAFETY: test-only. The pointer is to a parameter the test owns for longer than the host, and the
// tests are single-threaded; `Send` is needed only so the queue can sit in a `Mutex`.
unsafe impl Send for Pending {}

impl Recorder {
    fn begins(&self) -> usize {
        self.begins.load(Ordering::Relaxed)
    }
    fn ends(&self) -> usize {
        self.ends.load(Ordering::Relaxed)
    }
    fn sets(&self) -> usize {
        self.sets.load(Ordering::Relaxed)
    }
    /// From now on, hold every set back until [`Recorder::catch_up`].
    fn lag(&self) {
        self.lagging.store(true, Ordering::Relaxed);
    }
    /// Apply everything held back, in order, as the audio thread would on its next block.
    fn catch_up(&self) {
        for Pending(param, normalized) in self.pending.lock().unwrap().drain(..) {
            // SAFETY: the parameter outlives the host, which the test holds for less time.
            unsafe { param._internal_set_normalized_value(normalized) };
        }
    }
    /// Every normalised value the editor has sent, in order.
    fn sent(&self) -> Vec<f32> {
        self.sent.lock().unwrap().clone()
    }
}

impl nice_plug::context::gui::GuiContextInner for Recorder {
    // A test double has no host to ask for a restart (nice-plug 0.4).
    fn request_restart(&self) {}
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {
        self.begins.fetch_add(1, Ordering::Relaxed);
    }
    unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
        self.sets.fetch_add(1, Ordering::Relaxed);
        self.sent.lock().unwrap().push(normalized);
        if self.lagging.load(Ordering::Relaxed) {
            self.pending
                .lock()
                .unwrap()
                .push(Pending(param, normalized));
            return;
        }
        // SAFETY: the parameter outlives the setter, which borrows this host for the call.
        unsafe { param._internal_set_normalized_value(normalized) };
    }
    unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {
        self.ends.fetch_add(1, Ordering::Relaxed);
    }
    fn get_state(&self) -> PluginState {
        PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        }
    }
    fn set_state(&self, _state: PluginState) {}
}

/// One editor, driven frame by frame with whatever keys a test hands it.
struct Editor {
    ctx: egui::Context,
    view: usize,
    entries: HashMap<&'static str, Option<String>>,
    nav: mxm_ui::navigation::State,
    presets: PresetUi,
    telemetry: Telemetry,
}

impl Editor {
    fn new(params: &MxmMono01Params) -> Self {
        let ctx = egui::Context::default();
        mxm_ui::theme::apply(&ctx);
        mxm_ui::typography::apply(&ctx);
        Self {
            ctx,
            view: 0,
            entries: HashMap::new(),
            nav: mxm_ui::navigation::State::default(),
            presets: PresetUi::at(mxm_mono_01::preset::Library::at(None), params),
            telemetry: Telemetry::default(),
        }
    }

    /// Runs one frame with the given events, at a width wide enough for one page.
    fn frame(&mut self, params: &MxmMono01Params, host: &Recorder, events: Vec<egui::Event>) {
        let setter = ParamSetter::new(host);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1880.0, 1040.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = self.ctx.run_ui(input, |ui| {
            panel(
                ui,
                params,
                &self.telemetry,
                &setter,
                &mut self.view,
                &mut self.entries,
                &mut self.presets,
                &mut self.nav,
            );
        });
        output.textures_delta.clear();
    }

    fn settle(&mut self, params: &MxmMono01Params, host: &Recorder) {
        // The registry is built by drawing, so the cursor has nothing to act on until a frame has
        // been painted. Three frames also let the paging renderer finish measuring.
        for _ in 0..4 {
            self.frame(params, host, Vec::new());
        }
    }

    /// Asserts the cursor is where it lands, the first card's first parameter, which a test about
    /// the cards starts from.
    fn on_the_first_card(&self) {
        assert_eq!(
            self.nav.card(),
            Some(0),
            "the cursor lands on LFO and envelope"
        );
        assert_eq!(self.nav.parameter(), Some("lforate"));
    }
}

/// One key going down (or repeating, held) or coming up, with no modifier: the language's keys
/// are letters, arrows, `Tab` and `Escape`, and a chord with `Command` or `Alt` is not the cursor's.
fn key(key: Key, pressed: bool, repeat: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat,
        modifiers: egui::Modifiers::NONE,
    }
}

/// A tap of one key: its press and its release, in one frame.
fn tap(k: Key) -> Vec<egui::Event> {
    vec![key(k, true, false), key(k, false, false)]
}

/// Keys tapped one after another, all in one frame: `taps(&[key_of(VALUE), Key::ArrowUp,
/// key_of(OUT)])` is a complete fine edit.
fn taps(keys: &[Key]) -> Vec<egui::Event> {
    keys.iter().flat_map(|&k| tap(k)).collect()
}

/// VALUE, the arrow, OUT: one fine step that way, kept.
fn fine(arrow: Key) -> Vec<egui::Event> {
    taps(&[key_of(VALUE), arrow, key_of(OUT)])
}

/// VALUE, COARSE, the arrow, OUT: one coarse step that way, kept.
fn coarse(arrow: Key) -> Vec<egui::Event> {
    taps(&[key_of(VALUE), key_of(COARSE), arrow, key_of(OUT)])
}

/// The cursor lands on something without being aimed, so the first keystroke is never spent
/// arriving — the property the whole feature is for.
///
/// It lands on the first card's first parameter: a cursor that has never moved starts on the first
/// control a card painted. The app bar's output level is painted before it, and is not where the
/// cursor starts (the owner, 2026-10-07, of the language's pilot: it started on Output).
#[test]
fn the_cursor_starts_on_a_real_parameter() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);

    assert!(
        editor.nav.parameter().is_some(),
        "a cursor with nowhere to be would make every first press a wasted one"
    );
    editor.on_the_first_card();
}

/// VALUE + COARSE + ↑ is the coarse step, and it moves the parameter the cursor is on — not
/// whichever control egui's focus ring happened to be near.
#[test]
fn value_coarse_up_moves_the_selected_parameter_by_its_coarse_step() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);

    let selected = editor
        .nav
        .parameter()
        .expect("the cursor landed")
        .to_owned();
    let bound = binding_for(permanent(&selected), &params);
    let before = bound.param.normalised();
    let coarse_up = bound
        .param
        .step_from(f64::from(before), COARSE_UP, bound.law)
        - f64::from(before);

    editor.frame(&params, &host, coarse(Key::ArrowUp));

    let after = binding_for(permanent(&selected), &params)
        .param
        .normalised();
    if coarse_up <= 0.0 {
        assert_eq!(
            after, before,
            "a parameter already at its maximum has nowhere to go"
        );
        return;
    }
    assert!(
        (f64::from(after - before) - coarse_up).abs() < 1e-4,
        "{selected} moved by {} where its own coarse step is {coarse_up}",
        after - before
    );
}

/// Under VALUE ↑ goes up by the FINE step and → to the next line of it (2026-10-09), and the size
/// is COARSE's to change, never the arrow's. Fine is never bigger than coarse.
#[test]
fn value_steps_fine_up_snaps_fine_right_and_coarse_only_with_coarse() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);

    let selected = editor.nav.parameter().expect("landed").to_owned();
    let read = || {
        f64::from(
            binding_for(permanent(&selected), &params)
                .param
                .normalised(),
        )
    };
    let step = |press: mxm_ui::control::Press| {
        let bound = binding_for(permanent(&selected), &params);
        let here = f64::from(bound.param.normalised());
        bound.param.step_from(here, press, bound.law) - here
    };
    let (fine_up, coarse_up) = (step(FINE_UP), step(COARSE_UP));
    assert!(
        fine_up <= coarse_up,
        "{selected}: fine ({fine_up}) must never exceed coarse ({coarse_up})"
    );

    let fine_snap = mxm_ui::control::Press {
        snap: true,
        ..FINE_UP
    };
    for (arrow, press) in [(Key::ArrowUp, FINE_UP), (Key::ArrowRight, fine_snap)] {
        let (start, moved) = (read(), step(press));
        editor.frame(&params, &host, fine(arrow));
        assert!(
            (read() - start - moved).abs() < 1e-4,
            "VALUE + {arrow:?} is {press:?}"
        );
    }

    let (start, coarse_up) = (read(), step(COARSE_UP));
    editor.frame(&params, &host, coarse(Key::ArrowUp));
    assert!(
        (read() - start - coarse_up).abs() < 1e-4,
        "VALUE + COARSE + ↑ is the coarse step"
    );
}

/// **A held key is one gesture.** Otherwise a two-second hold writes a hundred begin/end pairs
/// into the host's automation lane, which is silent until somebody records over it. It ends when
/// the held VALUE is let go, not the arrow.
#[test]
fn a_held_value_edit_is_one_balanced_gesture() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);

    let selected = editor.nav.parameter().expect("landed").to_owned();
    let bound = binding_for(permanent(&selected), &params);
    let once = bound
        .param
        .step_from(f64::from(bound.param.normalised()), FINE_UP, bound.law);
    let twice = bound.param.step_from(once, FINE_UP, bound.law);
    let (before_begins, before_sets, before_ends) = (host.begins(), host.sets(), host.ends());
    editor.frame(
        &params,
        &host,
        vec![
            key(key_of(VALUE), true, false),
            key(Key::ArrowUp, true, false),
            key(Key::ArrowUp, true, true),
        ],
    );
    assert_eq!(host.begins() - before_begins, 1, "the hold opens once");
    assert_eq!(host.ends() - before_ends, 0, "it remains open while held");
    let after_batched = binding_for(permanent(&selected), &params)
        .param
        .normalised();
    assert!(
        (f64::from(after_batched) - twice).abs() < 1e-4,
        "both same-frame presses contribute to the value"
    );

    editor.frame(&params, &host, vec![key(Key::ArrowUp, true, true)]);
    assert_eq!(
        host.begins() - before_begins,
        1,
        "a repeat does not open another gesture"
    );
    assert_eq!(
        host.sets() - before_sets,
        2,
        "both key-down frames set the value"
    );

    editor.frame(&params, &host, vec![key(Key::ArrowUp, false, false)]);
    assert_eq!(
        host.ends() - before_ends,
        0,
        "letting go of the arrow leaves the gesture open while VALUE is held"
    );

    editor.frame(&params, &host, vec![key(key_of(VALUE), false, false)]);
    assert_eq!(
        host.ends() - before_ends,
        1,
        "letting go of VALUE closes it exactly once"
    );
    assert_eq!(host.begins(), host.ends(), "the gesture is balanced");
}

#[test]
fn view_right_moves_to_the_card_painted_to_the_right_in_paging_order() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.on_the_first_card();

    editor.frame(&params, &host, taps(&[key_of(VIEW), Key::ArrowRight]));
    // Since the LFO, the envelope and the amplifier became one card, the painted order is the
    // section order; the shared navigation's own tests hold the case where the two differ.
    assert_eq!(
        editor.nav.card(),
        Some(1),
        "the Oscillator is painted to the right of LFO and envelope"
    );
}

#[test]
fn a_waveform_segmented_parameter_can_be_edited_from_the_keyboard() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.on_the_first_card();

    // Past the rate's tempo sync, which sits between the rate and its shapes, onto the first
    // shape's cell: each cell of a segmented control is a stop for the bare arrows.
    for expected in ["lfosync", "lfowave"] {
        editor.frame(&params, &host, tap(Key::ArrowRight));
        editor.frame(&params, &host, Vec::new());
        assert_eq!(editor.nav.parameter(), Some(expected));
    }

    // Native custom-painted segment responses do not reliably retain egui focus between frames.
    // The navigation cursor remains the authority, so value editing must survive that loss.
    if let Some(focused) = editor.ctx.memory(|memory| memory.focused()) {
        editor
            .ctx
            .memory_mut(|memory| memory.surrender_focus(focused));
    }

    let before = params.lfo_wave.unmodulated_normalized_value();
    let sets = host.sets();
    editor.frame(&params, &host, fine(Key::ArrowRight));
    assert!(
        params.lfo_wave.unmodulated_normalized_value() > before,
        "VALUE + Right advances the selected waveform"
    );
    assert_eq!(host.sets() - sets, 1, "one host-visible parameter edit");
}

#[test]
fn a_text_segmented_parameter_can_be_edited_from_the_keyboard() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.on_the_first_card();

    editor.frame(&params, &host, taps(&[key_of(VIEW), Key::ArrowRight]));
    editor.frame(&params, &host, Vec::new());
    assert_eq!(editor.nav.card(), Some(1));
    assert_eq!(editor.nav.parameter(), Some("oscrange"));

    let before = params.osc_range.unmodulated_normalized_value();
    let sets = host.sets();
    editor.frame(&params, &host, fine(Key::ArrowRight));
    assert!(
        params.osc_range.unmodulated_normalized_value() > before,
        "VALUE + Right advances the selected text choice"
    );
    assert_eq!(host.sets() - sets, 1, "one host-visible parameter edit");
}

/// VIEW + arrows are the card cursor's, and the cursor must actually move when pressed.
#[test]
fn view_and_an_arrow_move_the_card_cursor_rather_than_the_value() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.on_the_first_card();

    let first_card = editor.nav.card().expect("landed");
    let sets_before = host.sets();

    editor.frame(&params, &host, taps(&[key_of(VIEW), Key::ArrowRight]));
    editor.frame(&params, &host, Vec::new());

    assert_ne!(
        editor.nav.card(),
        Some(first_card),
        "VIEW + Right moves to the next card"
    );
    assert_eq!(
        host.sets(),
        sets_before,
        "and writes nothing to the host on the way"
    );
}

/// A bare arrow moves inside the card. The card must not change while it does, and nothing is
/// edited. On a card of more than one parameter, which the app bar's is not.
#[test]
fn a_bare_arrow_moves_within_the_card_and_leaves_the_card_alone() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.on_the_first_card();

    let card = editor.nav.card().expect("landed");
    let first = editor.nav.parameter().expect("landed").to_owned();
    let sets_before = host.sets();

    editor.frame(&params, &host, tap(Key::ArrowRight));
    editor.frame(&params, &host, Vec::new());

    assert_eq!(editor.nav.card(), Some(card), "still the same card");
    assert_ne!(
        editor.nav.parameter(),
        Some(first.as_str()),
        "but a different parameter in it"
    );
    assert_eq!(host.sets(), sets_before, "and a bare arrow edits nothing");
}

#[test]
fn the_cardless_parameters_surface_does_not_run_the_musician_cursor() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.view = mxm_ui::paging::PARAMETERS;

    // The control the cursor landed on keeps the egui focus it was given, and a focused control
    // taking a bare arrow is the editing a cardless surface keeps, not the cursor.
    // Release it, so the only thing left that could take the arrow is a cursor that should have
    // stopped.
    if let Some(focused) = editor.ctx.memory(|memory| memory.focused()) {
        editor
            .ctx
            .memory_mut(|memory| memory.surrender_focus(focused));
    }
    editor.frame(&params, &host, tap(Key::ArrowRight));
    let survived = editor.ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: Key::ArrowRight,
                    pressed: true,
                    ..
                }
            )
        })
    });
    assert!(survived, "no invisible card cursor consumed the arrow");
}

/// The space bar belongs to the DAW's transport (§11 forbids trapping it), so the editor must
/// leave it in the queue for the host.
#[test]
fn space_is_never_consumed() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);

    editor.frame(&params, &host, tap(Key::Space));

    let survived = editor.ctx.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Key {
                    key: Key::Space,
                    ..
                }
            )
        })
    });
    assert!(survived, "the transport key must reach the host untouched");
}

/// While the preset browser owns the keyboard, the cursor is inert — its arrows are the browser's.
#[test]
fn the_browser_keeps_its_own_arrows() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.on_the_first_card();

    editor.presets.set_browser_open(true);
    editor.frame(&params, &host, tap(Key::ArrowRight));

    assert_eq!(
        (editor.nav.card(), editor.nav.parameter()),
        (Some(0), Some("lforate")),
        "the cursor stayed put while another surface had the keyboard"
    );
}

// ---- The pointer picks the target, and pitch steps musically (the owner, 2026-09-23) ----

const FINE_UP: mxm_ui::control::Press = mxm_ui::control::Press {
    up: true,
    coarse: false,
    finer: false,
    snap: false,
};
const COARSE_UP: mxm_ui::control::Press = mxm_ui::control::Press {
    up: true,
    coarse: true,
    finer: false,
    snap: false,
};

/// Where a parameter's control was painted last frame, from the cursor's own registry.
fn centre_of(editor: &Editor, key: &str) -> egui::Pos2 {
    mxm_ui::navigation::spots(&editor.ctx)
        .into_iter()
        .find(|spot| spot.key == key)
        .unwrap_or_else(|| panic!("{key} is on screen"))
        .rect
        .center()
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

impl Editor {
    /// Clicks a parameter's control, as a person reaching for the mouse would, and lets the next
    /// frame's cursor catch up.
    fn click(&mut self, params: &MxmMono01Params, host: &Recorder, key: &str) {
        let at = centre_of(self, key);
        self.frame(
            params,
            host,
            vec![
                egui::Event::PointerMoved(at),
                button(at, true),
                button(at, false),
            ],
        );
        // Two frames, as a person's hand takes far longer than two to go from the mouse to the
        // keys: the first moves the cursor to what the pointer took and hands it egui focus, and
        // the second draws it holding that focus.
        self.frame(params, host, Vec::new());
        self.frame(params, host, Vec::new());
    }

    /// Sets a parameter as the host would, and lets the panel read it back.
    fn set_plain(
        &mut self,
        params: &MxmMono01Params,
        host: &Recorder,
        param: &nice_plug::params::FloatParam,
        plain: f32,
    ) {
        ParamSetter::new(host).set_parameter(param, plain);
        self.frame(params, host, Vec::new());
    }
}

/// **The owner's report**: a knob turned with the mouse was not the arrows' target, because egui
/// never focuses a painted control for a click and the cursor followed focus alone. Now the click
/// is the cursor's, and the cutoff steps by an octave, as one balanced gesture.
#[test]
fn a_clicked_knob_takes_the_next_edit_and_the_cutoff_steps_an_octave() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.set_plain(&params, &host, &params.cutoff, 500.0);
    assert_ne!(
        editor.nav.parameter(),
        Some("cutoff"),
        "the cursor starts elsewhere"
    );

    editor.click(&params, &host, "cutoff");
    assert_eq!(
        editor.nav.parameter(),
        Some("cutoff"),
        "the click took the cursor"
    );

    let before = params.cutoff.unmodulated_plain_value();
    let (begins, ends) = (host.begins(), host.ends());
    editor.frame(&params, &host, coarse(Key::ArrowUp));
    let after = params.cutoff.unmodulated_plain_value();
    assert!(
        (after / before - 2.0).abs() < 1e-3,
        "VALUE + COARSE + Up is an octave: {before} Hz became {after} Hz"
    );
    assert_eq!(host.begins() - begins, 1, "one gesture opened");
    assert_eq!(host.ends() - ends, 1, "and closed");
}

/// A drag is the pointer's too, and the fine tune then moves by exactly one cent.
#[test]
fn a_dragged_knob_takes_the_next_edit_and_the_tune_steps_a_cent() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);

    let at = centre_of(&editor, "tune");
    let to = at - egui::vec2(0.0, 30.0);
    editor.frame(
        &params,
        &host,
        vec![egui::Event::PointerMoved(at), button(at, true)],
    );
    editor.frame(&params, &host, vec![egui::Event::PointerMoved(to)]);
    editor.frame(&params, &host, vec![button(to, false)]);
    editor.frame(&params, &host, Vec::new());
    assert_eq!(
        editor.nav.parameter(),
        Some("tune"),
        "the drag took the cursor"
    );

    let before = params.tune.unmodulated_plain_value();
    assert!(before > 1.0, "the drag moved the tune: {before}");
    editor.frame(&params, &host, fine(Key::ArrowUp));
    let after = params.tune.unmodulated_plain_value();
    assert!(
        (after - before - 1.0).abs() < 1e-3,
        "VALUE + Up is one cent: {before} became {after}"
    );
}

/// VALUE + COARSE + ↑ on the bend range is an octave — twelve semitones — which on a 0–12 range is
/// its end.
#[test]
fn the_bend_range_steps_an_octave_to_its_end() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    set_advanced(&editor.ctx, true);
    editor.settle(&params, &host);

    editor.click(&params, &host, "bendrange");
    assert_eq!(editor.nav.parameter(), Some("bendrange"));
    let bound = binding_for("bendrange", &params);
    let here = f64::from(bound.param.normalised());
    assert!(
        bound.param.step_from(here, COARSE_UP, bound.law) > here,
        "there is room above for this to prove anything"
    );
    editor.frame(&params, &host, coarse(Key::ArrowUp));
    assert_eq!(params.bend_range.unmodulated_plain_value(), 12.0);
}

/// Two presses in one frame each start where the one before landed: two octaves, not two of the
/// first octave's normalised distance.
#[test]
fn two_presses_in_one_frame_are_two_octaves() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.set_plain(&params, &host, &params.cutoff, 500.0);
    editor.click(&params, &host, "cutoff");

    editor.frame(
        &params,
        &host,
        taps(&[
            key_of(VALUE),
            key_of(COARSE),
            Key::ArrowUp,
            Key::ArrowUp,
            key_of(OUT),
        ]),
    );
    let after = params.cutoff.unmodulated_plain_value();
    assert!(
        (after / 500.0 - 4.0).abs() < 1e-3,
        "500 Hz became {after} Hz"
    );
}

/// **A held key chains from what it last sent.** The host here applies nothing until the held
/// VALUE is let go, and every repeat still moves exactly one more cent.
#[test]
fn a_held_edit_advances_one_cent_per_repeat_while_the_host_lags() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.set_plain(&params, &host, &params.tune, 10.0);
    editor.click(&params, &host, "tune");
    assert_eq!(editor.nav.parameter(), Some("tune"));

    let start = params.tune.unmodulated_plain_value();
    host.lag();
    let mut landed = Vec::new();
    for repeat in [false, true, true] {
        let mut events = Vec::new();
        if !repeat {
            events.push(key(key_of(VALUE), true, false));
        }
        events.push(key(Key::ArrowUp, true, repeat));
        editor.frame(&params, &host, events);
        let sent = *host.sent().last().expect("each repeat sends");
        landed.push(params.tune.preview_plain(sent));
    }
    for (step, landed) in landed.iter().enumerate() {
        let expected = start + (step as f32 + 1.0);
        assert!(
            (landed - expected).abs() < 1e-3,
            "repeat {step} landed on {landed}, expected {expected}"
        );
    }
    editor.frame(
        &params,
        &host,
        vec![
            key(Key::ArrowUp, false, false),
            key(key_of(VALUE), false, false),
        ],
    );
    host.catch_up();
    assert!((params.tune.unmodulated_plain_value() - (start + 3.0)).abs() < 1e-3);
}
