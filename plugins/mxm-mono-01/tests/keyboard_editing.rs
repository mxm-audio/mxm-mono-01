//! The keyboard cursor, driven through mxm-mono-01's real editor.
//!
//! mxm-kit's `crates/ui` tests prove the cursor's arithmetic against a synthetic registry. This
//! proves the half that only exists once a real panel has drawn: that controls register themselves,
//! that the cursor reaches them, that a bare arrow moves the selected parameter by its own step, and
//! that a held key is one automation gesture rather than a hundred.
//!
//! Layout is not asserted here. Which card a knob lands on depends on the width the pack chose,
//! and `editor.rs`'s own paging test owns that. What is asserted is where the cursor **starts**:
//! on the output level in the app bar, which is drawn before any card (design system §3.1 item 6
//! puts it there), so a test about the cards walks into them first.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use mxm_mono_01::editor::sections::{ASSIGNMENT, binding_for};
use mxm_mono_01::editor::{OUTPUT_CARD, PresetUi, panel, set_advanced};
use mxm_mono_01::params::MxmMono01Params;
use mxm_mono_01::telemetry::Telemetry;

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
use nice_plug::params::{Param, internals::ParamPtr};
use nice_plug::prelude::{ParamSetter, PluginApi, PluginState};

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
        let modifiers = events
            .iter()
            .rev()
            .find_map(|event| match event {
                egui::Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let mut events = events;
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
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

    /// Walks the cursor from the app bar, where it lands, to the first card: LFO and envelope.
    ///
    /// `Shift`+Down leaves the bar for the nearest card in the first row, and `Shift`+Left walks
    /// that row to its start. Keys only, as a person would, because the cursor has no setter.
    fn walk_to_first_card(&mut self, params: &MxmMono01Params, host: &Recorder) {
        assert_eq!(
            self.nav.card(),
            Some(OUTPUT_CARD),
            "the cursor lands on the app bar's output level"
        );
        self.frame(
            params,
            host,
            press(egui::Key::ArrowDown, egui::Modifiers::SHIFT),
        );
        for _ in 0..SECTION_COUNT {
            let here = self.nav.card();
            if here == Some(0) {
                break;
            }
            self.frame(
                params,
                host,
                press(egui::Key::ArrowLeft, egui::Modifiers::SHIFT),
            );
            assert_ne!(
                self.nav.card(),
                here,
                "Shift+Left stopped short of LFO and envelope"
            );
        }
        self.frame(params, host, Vec::new());
        assert_eq!(
            self.nav.card(),
            Some(0),
            "the first row starts on LFO and envelope"
        );
        assert_eq!(self.nav.parameter(), Some("lforate"));
    }
}

/// The four cards: the most `Shift`+Left presses a walk along one row can take.
const SECTION_COUNT: usize = 4;

fn key(key: egui::Key, modifiers: egui::Modifiers, pressed: bool, repeat: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat,
        modifiers,
    }
}

fn press(k: egui::Key, modifiers: egui::Modifiers) -> Vec<egui::Event> {
    vec![
        key(k, modifiers, true, false),
        key(k, modifiers, false, false),
    ]
}

/// The cursor lands on something without being aimed, so the first keystroke is never spent
/// arriving — the property the whole feature is for.
///
/// It lands on the output level in the app bar: the bar is drawn before any card, and a cursor
/// that has never moved starts on the first control painted.
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
    assert_eq!(
        editor.nav.card(),
        Some(OUTPUT_CARD),
        "and it is on the app bar's card"
    );
    assert_eq!(editor.nav.parameter(), Some("outgain"));
}

/// Bare up is the M8's coarse axis, and it moves the parameter the cursor is on — not whichever
/// control egui's focus ring happened to be near.
#[test]
fn bare_up_moves_the_selected_parameter_by_a_coarse_step() {
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

    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowUp, egui::Modifiers::NONE),
    );

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

/// Fine is smaller than coarse, and it is left/right — the tracker's orientation, which is the
/// opposite of this feature's first sketch.
#[test]
fn left_and_right_are_the_fine_axis_and_up_and_down_the_coarse_one() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);

    let selected = editor.nav.parameter().expect("landed").to_owned();
    let bound = binding_for(permanent(&selected), &params);
    let here = f64::from(bound.param.normalised());
    let fine_up = bound.param.step_from(here, FINE_UP, bound.law) - here;
    let coarse_up = bound.param.step_from(here, COARSE_UP, bound.law) - here;

    assert!(
        fine_up <= coarse_up,
        "{selected}: fine ({fine_up}) must never exceed coarse ({coarse_up})"
    );

    let start = bound.param.normalised();
    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::NONE),
    );
    let after_fine = binding_for(permanent(&selected), &params)
        .param
        .normalised();
    assert!(
        (f64::from(after_fine - start) - fine_up).abs() < 1e-4,
        "right is the fine axis"
    );
}

/// **A held key is one gesture.** Otherwise a two-second hold writes a hundred begin/end pairs
/// into the host's automation lane, which is silent until somebody records over it.
#[test]
fn a_held_arrow_is_one_balanced_gesture_per_press() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);

    let selected = editor.nav.parameter().expect("landed").to_owned();
    let bound = binding_for(permanent(&selected), &params);
    let start = bound.param.normalised();
    let fine_up = bound.param.step_from(f64::from(start), FINE_UP, bound.law) - f64::from(start);
    let (before_begins, before_sets, before_ends) = (host.begins(), host.sets(), host.ends());
    editor.frame(
        &params,
        &host,
        vec![
            key(egui::Key::ArrowRight, egui::Modifiers::NONE, true, false),
            key(egui::Key::ArrowRight, egui::Modifiers::NONE, true, true),
        ],
    );
    assert_eq!(host.begins() - before_begins, 1, "the hold opens once");
    assert_eq!(host.ends() - before_ends, 0, "it remains open while held");
    let after_batched = binding_for(permanent(&selected), &params)
        .param
        .normalised();
    assert!(
        (f64::from(after_batched - start) - 2.0 * fine_up).abs() < 1e-4,
        "both same-frame presses contribute to the value"
    );

    editor.frame(
        &params,
        &host,
        vec![key(
            egui::Key::ArrowRight,
            egui::Modifiers::NONE,
            true,
            true,
        )],
    );
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

    editor.frame(
        &params,
        &host,
        vec![key(
            egui::Key::ArrowRight,
            egui::Modifiers::NONE,
            false,
            false,
        )],
    );
    assert_eq!(host.ends() - before_ends, 1, "release closes exactly once");
    assert_eq!(host.begins(), host.ends(), "the gesture is balanced");
}

#[test]
fn right_moves_to_the_card_painted_to_the_right_in_paging_order() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.walk_to_first_card(&params, &host);

    assert_eq!(editor.nav.card(), Some(0), "starts on LFO and envelope");
    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::SHIFT),
    );
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
    editor.walk_to_first_card(&params, &host);

    // Past the rate's tempo sync, which sits between the rate and its shapes.
    for expected in ["lfosync", "lfowave"] {
        editor.frame(
            &params,
            &host,
            press(egui::Key::ArrowRight, egui::Modifiers::COMMAND),
        );
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
    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::NONE),
    );
    assert!(
        params.lfo_wave.unmodulated_normalized_value() > before,
        "bare Right advances the selected waveform"
    );
    assert_eq!(host.sets() - sets, 1, "one host-visible parameter edit");
}

#[test]
fn a_text_segmented_parameter_can_be_edited_from_the_keyboard() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.walk_to_first_card(&params, &host);

    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::SHIFT),
    );
    editor.frame(&params, &host, Vec::new());
    assert_eq!(editor.nav.card(), Some(1));
    assert_eq!(editor.nav.parameter(), Some("oscrange"));

    let before = params.osc_range.unmodulated_normalized_value();
    let sets = host.sets();
    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::NONE),
    );
    assert!(
        params.osc_range.unmodulated_normalized_value() > before,
        "bare Right advances the selected text choice"
    );
    assert_eq!(host.sets() - sets, 1, "one host-visible parameter edit");
}

/// Shift+arrows are the module/card cursor's, and the cursor must actually move when pressed.
#[test]
fn shift_arrow_moves_the_card_cursor_rather_than_the_value() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.walk_to_first_card(&params, &host);

    let first_card = editor.nav.card().expect("landed");
    let sets_before = host.sets();

    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::SHIFT),
    );
    editor.frame(&params, &host, Vec::new());

    assert_ne!(
        editor.nav.card(),
        Some(first_card),
        "Shift+Right moves to the next card"
    );
    assert_eq!(
        host.sets(),
        sets_before,
        "and writes nothing to the host on the way"
    );
}

/// `Command` moves inside the card. The card must not change while it does. On a card of more
/// than one parameter, which the app bar's is not.
#[test]
fn command_moves_within_the_card_and_leaves_the_card_alone() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.walk_to_first_card(&params, &host);

    let card = editor.nav.card().expect("landed");
    let first = editor.nav.parameter().expect("landed").to_owned();

    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::COMMAND),
    );
    editor.frame(&params, &host, Vec::new());

    assert_eq!(editor.nav.card(), Some(card), "still the same card");
    assert_ne!(
        editor.nav.parameter(),
        Some(first.as_str()),
        "but a different parameter in it"
    );
}

#[test]
fn the_cardless_parameters_surface_does_not_run_the_musician_cursor() {
    let params = MxmMono01Params::default();
    let host = Recorder::default();
    let mut editor = Editor::new(&params);
    editor.settle(&params, &host);
    editor.view = mxm_ui::paging::PARAMETERS;

    // The app bar's output level is drawn on this surface too, and keeps the egui focus the cursor
    // gave it when it landed there; a focused control taking a bare arrow is the pre-cursor
    // editing a cardless surface keeps, not the cursor. Release it, so the only thing left that
    // could take the arrow is a cursor that should have stopped — whose stale target would name
    // that same bar card, which is still drawn here.
    if let Some(focused) = editor.ctx.memory(|memory| memory.focused()) {
        editor
            .ctx
            .memory_mut(|memory| memory.surrender_focus(focused));
    }
    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::NONE),
    );
    let survived = editor.ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::ArrowRight,
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

    editor.frame(
        &params,
        &host,
        press(egui::Key::Space, egui::Modifiers::NONE),
    );

    let survived = editor.ctx.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Key {
                    key: egui::Key::Space,
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

    let card = editor.nav.card().expect("landed");
    editor.presets.set_browser_open(true);
    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::NONE),
    );

    assert_eq!(
        editor.nav.card(),
        Some(card),
        "the cursor stayed put while another surface had the keyboard"
    );
}

// ---- The pointer picks the target, and pitch steps musically (the owner, 2026-09-23) ----

const FINE_UP: mxm_ui::control::Press = mxm_ui::control::Press {
    up: true,
    coarse: false,
    finer: false,
};
const COARSE_UP: mxm_ui::control::Press = mxm_ui::control::Press {
    up: true,
    coarse: true,
    finer: false,
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
        // Two frames, as a person's hand takes far longer than two to go from the mouse to an
        // arrow: the first hands the knob egui focus, and only the second can lock egui's own
        // arrow travel out of it — egui sets that lock on a widget that already had focus last
        // frame. An arrow in between would also walk egui's focus ring to the neighbour, exactly
        // as it would one frame after a `Command`+arrow move.
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
fn a_clicked_knob_takes_the_next_arrow_and_the_cutoff_steps_an_octave() {
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
    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowUp, egui::Modifiers::NONE),
    );
    let after = params.cutoff.unmodulated_plain_value();
    assert!(
        (after / before - 2.0).abs() < 1e-3,
        "Up is an octave: {before} Hz became {after} Hz"
    );
    assert_eq!(host.begins() - begins, 1, "one gesture opened");
    assert_eq!(host.ends() - ends, 1, "and closed");
}

/// A drag is the pointer's too, and the fine tune then moves by exactly one cent.
#[test]
fn a_dragged_knob_takes_the_next_arrow_and_the_tune_steps_a_cent() {
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
    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowRight, egui::Modifiers::NONE),
    );
    let after = params.tune.unmodulated_plain_value();
    assert!(
        (after - before - 1.0).abs() < 1e-3,
        "Right is one cent: {before} became {after}"
    );
}

/// Up on the bend range is an octave — twelve semitones — which on a 0–12 range is its end.
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
    editor.frame(
        &params,
        &host,
        press(egui::Key::ArrowUp, egui::Modifiers::NONE),
    );
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
        vec![
            key(egui::Key::ArrowUp, egui::Modifiers::NONE, true, false),
            key(egui::Key::ArrowUp, egui::Modifiers::NONE, true, true),
            key(egui::Key::ArrowUp, egui::Modifiers::NONE, false, false),
        ],
    );
    let after = params.cutoff.unmodulated_plain_value();
    assert!(
        (after / 500.0 - 4.0).abs() < 1e-3,
        "500 Hz became {after} Hz"
    );
}

/// **A held key chains from what it last sent.** The host here applies nothing until the key is
/// released, and every repeat still moves exactly one more cent.
#[test]
fn a_held_arrow_advances_one_cent_per_repeat_while_the_host_lags() {
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
        editor.frame(
            &params,
            &host,
            vec![key(
                egui::Key::ArrowRight,
                egui::Modifiers::NONE,
                true,
                repeat,
            )],
        );
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
        vec![key(
            egui::Key::ArrowRight,
            egui::Modifiers::NONE,
            false,
            false,
        )],
    );
    host.catch_up();
    assert!((params.tune.unmodulated_plain_value() - (start + 3.0)).abs() < 1e-3);
}
