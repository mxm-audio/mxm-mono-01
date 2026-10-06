//! mxm-mono-01's editor.
//!
//! Built to `docs/briefs/mxm-mono-01.md`, which is the gating document — this module implements it and
//! does not re-decide it. In particular the brief owns:
//!
//! - **§10's section sequence**, LFO · Oscillator · Mixer · Filter · Amplifier · Envelope.
//!   That sequence is the signal flow and is also where someone who has used an SH-101 looks, and
//!   [`SECTIONS`] is written in that order so a reordering is a visible diff rather than a drift.
//! - **§5's surface classification** for every parameter: on its card, behind a disclosure, or —
//!   the output level alone — in the app bar beside the meter, as design system §3.1 item 6 asks.
//! - **§6's category/card inventory**, with derived musician pages and developer-only Parameters.
//! - **§8's three visualizations** and their fidelity.
//!
//! # It is a panel, not a window
//!
//! [`panel`] takes a `Ui` and draws into it. It does not create a window, run an event loop, or
//! own a swapchain. That is what lets the same code be the plugin's CLAP editor here and the
//! standalone harness's contents in `apps/mxm-mono-01-standalone`, and it is what keeps native GUI
//! hosting — deferred, not abandoned — from being foreclosed.
//!
//! # Gestures
//!
//! Every edit is bracketed: `begin_set_parameter`, `set_parameter_normalized`, `end_set_parameter`.
//! A drag opens on press and closes on release; a double-click, a keystroke and a text entry are
//! instantaneous and bracket themselves in one frame. An unclosed gesture leaves a host's
//! automation lane latched, which is silent until someone tries to record over it.

pub mod binding;
pub mod sections;
mod visuals;

use std::collections::HashMap;
use std::sync::Arc;

use binding::Bound;
use egui::Ui;
use mxm_ui::space::SPACE_5;
use mxm_ui::theme::Tokens;
use nice_plug::context::gui::GuiContext;
use nice_plug::prelude::*;
use nice_plug_egui::{EguiEditorState, NiceEguiApp, create_egui_editor};

use crate::params::MxmMono01Params;
use crate::telemetry::Telemetry;

/// The size the editor **opens** at; it resizes, and the cards reflow into rows (§3.4, §4.3).
///
/// **Derived, not chosen**: the quarter-4K budget hugged around every page, which
/// `tests::the_opening_size_is_the_budget_hugged` holds.
const REFERENCE: (u32, u32) = (1163, 840);

/// The four cards — brief §10's six sections, the LFO, envelope and amplifier as one — **in sequence**.
///
/// This is the instrument's information architecture, and the order is the contract. Changing it
/// changes what an SH-101 user finds where, which is the requirement §11's trial exists to check.
const SECTIONS: &[Section] = &[
    Section::LfoAndEnvelope,
    Section::Oscillator,
    Section::Mixer,
    Section::Filter,
];

/// The narrowest the window may be: **one card wide**, at the widest card's floor, plus the
/// panel's two gutters.
///
/// Grouping and wrapping are preferences the layout applies while it can, never floors on the
/// window — a single column of cards is a good layout in a narrow tile (§4.3).
const MINIMUM: (u32, u32) = (448, 320);

/// The keyboard cursor's card for the app bar's output level, outside the paging keys 0…3.
pub const OUTPUT_CARD: u64 = 64;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Section {
    /// The brief's LFO, Envelope and Amplifier as one card (the owner, 2026-09-24): the two
    /// modulation sources and the amplifier the envelope drives. Hugged, the LFO and the
    /// Amplifier were a small cluster each.
    LfoAndEnvelope,
    Oscillator,
    Mixer,
    Filter,
}

impl Section {
    pub const fn title(self) -> &'static str {
        match self {
            Self::LfoAndEnvelope => "LFO and envelope",
            Self::Oscillator => "Oscillator",
            Self::Mixer => "Mixer",
            Self::Filter => "Filter",
        }
    }
}

/// Builds the editor. Called from `Plugin::editor`.
pub fn create(params: Arc<MxmMono01Params>, telemetry: Arc<Telemetry>) -> Option<MxmMono01Editor> {
    let state = EguiEditorState::from_size(
        nice_plug::editor::dpi::LogicalSize::new(REFERENCE.0, REFERENCE.1),
        1.0,
    );

    create_egui_editor(
        state,
        nice_plug_egui::RepaintNotifier::new(),
        nice_plug_egui::EguiNiceSettings {
            title: "mxm-mono-01".to_owned(),
            // **Resizable, because the layout reflows.** The cards wrap into rows at whatever
            // width the window is given, which is what a tiling window manager needs and what the
            // old fixed frame could not do: a window of any other size could only add empty space
            // or clip, and both happened.
            //
            // `preserve_aspect_ratio` stays off. It is a CLAP hint for *embedded* editors, where
            // the host owns the window and does the constraining, and a floating window ignored it;
            // a reflowing layout has no ratio to preserve anyway.
            //
            // Zoom is still **chosen, not derived** — the app bar's control. Deriving it from the
            // window feeds back: zoom sets size, size sets zoom, and the window jitters between two
            // writers of one value. That trap is unchanged by reflow.
            resize_hint: ResizeHint {
                // The floor is one card wide plus the panel's gutters: below it a card would be
                // drawn narrower than its own controls, which no arrangement can fix.
                size_constraints: nice_plug::editor::SizeConstraints::min_logical_size(
                    nice_plug::editor::dpi::LogicalSize::new(MINIMUM.0 as f32, MINIMUM.1 as f32),
                ),
                ..ResizeHint::RESIZABLE
            },
            ..Default::default()
        },
        MxmMono01App::new(params, telemetry),
    )
}

/// The editor type the plugin exposes.
pub type MxmMono01Editor = nice_plug_egui::EguiEditor<MxmMono01App>;

/// The editor's own state: what the plugin does not own, and the host does not need.
pub struct MxmMono01App {
    params: Arc<MxmMono01Params>,
    telemetry: Arc<Telemetry>,
    /// Set in `build`, because that is where nice-plug hands it over.
    gui_context: Option<GuiContext>,
    /// Musician pages (0), or the developer-only Parameters surface (127).
    view: usize,
    /// Open text-entry buffers, keyed by parameter id. §7.1 requires direct text entry on every
    /// continuous control.
    text_entry: HashMap<&'static str, Option<String>>,
    /// The preset library and everything the browser needs across frames.
    presets: PresetUi,
    /// Where the keyboard is: a card, and a parameter inside it. Transient, like the text
    /// buffers — it is not a parameter and nothing durable reads it.
    nav: mxm_ui::navigation::State,
}

/// The app bar's preset controls and what they need between frames — `mxm-preset`'s, one for
/// every instrument.
pub use mxm_preset::PresetUi;

impl MxmMono01App {
    fn new(params: Arc<MxmMono01Params>, telemetry: Arc<Telemetry>) -> Self {
        let params_for_presets = Arc::clone(&params);
        Self {
            params,
            telemetry,
            gui_context: None,
            view: 0,
            text_entry: HashMap::new(),
            presets: PresetUi::new(params_for_presets.as_ref()),
            nav: mxm_ui::navigation::State::default(),
        }
    }
}

impl NiceEguiApp for MxmMono01App {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        // Once, on open. Both are idempotent, and doing it here rather than per frame means the
        // first frame is already styled — a `Ui` reads a clone of the style it was built with, so
        // applying mid-frame would show as an unstyled flash.
        mxm_ui::theme::apply(&egui_ctx);
        mxm_ui::typography::apply(&egui_ctx);

        // Light by default, overridable with `MXM_EDITOR_THEME`. The reasoning, and why the
        // default is not `System`, lives on `mxm_ui::theme::preference`.
        egui_ctx.set_theme(mxm_ui::theme::preference());
        self.gui_context = Some(nice_gui_ctx);
        Ok(())
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut nice_plug_egui::Frame) {
        let Some(gui_context) = self.gui_context.clone() else {
            // No context means `build` did not run, which should be impossible. Draw nothing
            // rather than panicking: a panic in a plugin's paint call takes the host with it.
            return;
        };

        panel(
            ui,
            &self.params,
            &self.telemetry,
            &gui_context.param_setter(),
            &mut self.view,
            &mut self.text_entry,
            &mut self.presets,
            &mut self.nav,
        );
    }

    fn editor_closed(&mut self) {
        // §8: the visualizations stop when the editor is closed. Dropping the context is also what
        // releases the host's callbacks, which nice-plug's docs ask for explicitly.
        self.gui_context = None;
    }
}

/// Draws the whole editor into `ui`.
///
/// Free function rather than a method, and taking only borrowed state, so the standalone harness
/// can call it with its own storage. Nothing here knows whether it is inside a DAW.
#[allow(clippy::too_many_arguments)]
pub fn panel(
    ui: &mut Ui,
    params: &MxmMono01Params,
    telemetry: &Telemetry,
    setter: &ParamSetter<'_>,
    view: &mut usize,
    text_entry: &mut HashMap<&'static str, Option<String>>,
    presets: &mut PresetUi,
    nav: &mut mxm_ui::navigation::State,
) {
    let tokens = tokens_for(ui);

    // A frame every 50 ms while the editor is open: the level meter changes between input events,
    // and so does a developer-channel request, which a frame that waited for the pointer would
    // strand on a view where nothing animates.
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(50));

    // The collection's developer channel (`plugins/AGENTS.md`): resolve a surface request before
    // deciding who owns this frame's keyboard.
    mxm_ui::paging::editor::developer_request(ui.ctx(), view, telemetry.take_view_request());
    // One question, and both layers suspend on it: the paging renderer's `hold` and the cursor's
    // `inert` both ask whether another surface owns this frame's keyboard.
    let busy = presets.holds_the_keyboard() || text_entry.values().any(Option::is_some);
    mxm_ui::paging::editor::hold(ui.ctx(), busy);

    // **The keyboard cursor moves before anything is drawn**, so a bare arrow is consumed here
    // rather than also walking egui's own focus ring — the failure `mxm_ui::browser` records.
    // It reads the registry and exact card rectangles the previous frame built, and navigates the
    // *plan's* order rather than the authored one: the renderer category-sorts, which since the
    // four cards took the categories' own order agrees with it.
    if *view == mxm_ui::paging::PARAMETERS {
        // This surface has no cards. Stop rather than merely hiding the outline, or its controls
        // lose their legacy bare-arrow editing to an invisible stale musician cursor.
        mxm_ui::navigation::stop(ui.ctx());
    } else {
        mxm_ui::navigation::paged_with_bar(ui.ctx(), nav, busy, &[OUTPUT_CARD]);
    }
    if let Some(open) = telemetry.take_browser_request() {
        presets.set_browser_open(open);
    }
    // A theme, by index. Applied and not stored: this channel is how a screenshot run and a test
    // reach a state, and neither should overwrite the choice made in the control.
    if let Some(index) = telemetry.take_theme_request()
        && let Some(preference) = mxm_ui::theme::from_index(index)
    {
        ui.ctx().set_theme(preference);
    }
    // The Oscillator's Advanced disclosure keeps its state where its header reads and writes it:
    // egui's memory, under the shared disclosure id.
    if let Some(open) = telemetry.take_disclosure_request() {
        set_advanced(ui.ctx(), open);
    }

    mxm_ui::AppBar::new("mxm-mono-01").show_with(
        ui,
        &tokens,
        // §3.1 slots 2-4.
        |ui| mxm_preset::ui::preset_row(ui, &tokens, params, setter, presets),
        |ui| {
            // §3.1 slot 6. Peak is reset by the read, so this is "loudest since the last frame".
            if mxm_ui::shell::level_meter(ui, &tokens, telemetry.take_peak(), telemetry.clipped()) {
                telemetry.acknowledge_clip();
            }

            // §3.1 slot 6: the output control sits beside its meter, not on a card.
            // A bar card, so the keyboard cursor reaches it although the paging report cannot see
            // the bar.
            mxm_ui::navigation::bar_card(ui, OUTPUT_CARD, |ui| {
                ui.scope(|ui| {
                    sections::binding_for("outgain", params)
                        .slider_inline(ui, &tokens, setter, text_entry, 96.0);
                })
                .response
                .rect
            });

            mxm_ui::shell::zoom_control(ui);

            // §3.1 slot 5, and the same place the player keeps it: at the left end of the bar's
            // right-hand group. What the person picks is remembered for every MXM editor, so the
            // next one to open agrees with this one.
            mxm_ui::shell::editor_theme_control(ui);
        },
    );

    // A name being typed, or the last thing that went wrong. Below the bar rather than in it: §3.1
    // says *do not turn the app bar into a second parameter panel*, and a text field that appears
    // in a bar moves everything beside it.
    mxm_preset::ui::overlays(ui, &tokens, params, setter, presets);

    // **The Parameters view has no tab.** It is the complete generated list, and an editor whose
    // own interface reaches every control does not need a second way to the same parameters in
    // front of a musician every day. It stays reachable: the developer channel still requests it
    // by index, which is what the CLI and a host's automation list use it for.
    // The paging renderer owns navigation; Parameters stays developer-only.

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(tokens.canvas)
                .inner_margin(egui::Margin::same(SPACE_5 as i8)),
        )
        .show(ui, |ui| {
            if *view == mxm_ui::paging::PARAMETERS {
                parameters_view(ui, &tokens, params, setter, text_entry);
            } else {
                paged_view(ui, &tokens, params, telemetry, setter, text_entry);
            }
        });
}

/// Opens or closes the Oscillator card's Advanced disclosure, where its header keeps that state.
pub fn set_advanced(ctx: &egui::Context, open: bool) {
    ctx.data_mut(|d| d.insert_temp(mxm_ui::shell::disclosure_id(sections::ADVANCED), open));
}

/// Every paging item, each floor computed from its card's tree in `ui`'s fonts every frame, and
/// each card exactly as wide as that floor: its ceiling is its floor, so everything packs tight
/// (`plans/plan-editor-standard.md` A1). No card declares a usability minimum (A2).
pub fn page_items(ui: &Ui, params: &MxmMono01Params) -> Vec<mxm_ui::paging::Item<'static>> {
    use mxm_ui::{
        flow::Card,
        paging::{Category as C, Item, Key},
    };
    SECTIONS
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let tree = sections::card(ui, *s, params);
            let floor = mxm_ui::tree::card_floor(ui, s.title(), &tree);
            Item {
                key: Key(i as u64),
                card: Card::new(s.title(), floor).capped(floor),
                category: match s {
                    Section::LfoAndEnvelope => C::Modulators,
                    Section::Oscillator => C::Generators,
                    _ => C::Tone,
                },
                kind: s.title(),
            }
        })
        .collect()
}

fn paged_view(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmMono01Params,
    telemetry: &Telemetry,
    setter: &ParamSetter<'_>,
    entries: &mut HashMap<&'static str, Option<String>>,
) {
    use mxm_ui::paging::Key;
    let items = page_items(ui, params);
    let text_editing = entries.values().any(Option::is_some);
    // Nothing a card draws reads telemetry destructively: the peak is the app bar's, read above.
    let mut live = sections::Live {
        params,
        telemetry,
        setter,
        entries,
    };
    mxm_ui::paging::editor::show(
        ui,
        tokens,
        &items,
        &[&[Key(2), Key(3)]],
        text_editing,
        &mut |ui, i| sections::card(ui, SECTIONS[i], params),
        &mut |ui, _, leaf, rect| sections::paint(ui, tokens, leaf, rect, &mut live),
    );
}

/// The paging items as the editor computes them, from a context set up as an editor's is — three
/// passes in, so the weighted font cuts are bound — for tests, which have no editor `Ui` to hand.
#[cfg(test)]
pub(crate) fn test_items() -> Vec<mxm_ui::paging::Item<'static>> {
    let ctx = egui::Context::default();
    mxm_ui::typography::apply(&ctx);
    mxm_ui::theme::apply(&ctx);
    let params = MxmMono01Params::default();
    let mut items = Vec::new();
    for _ in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            items = page_items(ui, &params);
        });
        output.textures_delta.clear();
    }
    items
}

/// The cards' floors in paging order, as [`test_items`] computes them.
#[cfg(test)]
pub(crate) fn test_floors() -> Vec<f32> {
    test_items().iter().map(|item| item.card.floor).collect()
}

/// §6's `Parameters` view: the flat list, which is the testing surface.
///
/// This is the interface the player already offers for any plugin. Having it here too is the
/// user's requested switch, and it is better placed here than in a host: the plugin knows its own
/// units, formatting and descriptions, and a host does not.
fn parameters_view(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmMono01Params,
    setter: &ParamSetter<'_>,
    text_entry: &mut HashMap<&'static str, Option<String>>,
) {
    // **The one view that reflows.** It is a list of 27 things with no layout to remember, so
    // filling whatever width it is given is the whole point of it.
    mxm_ui::shell::scroll_list(ui).show(ui, |ui| {
        let columns = if ui.available_width() >= 1000.0 { 3 } else { 2 };
        let entries = all_parameters(params);
        let per_column = entries.len().div_ceil(columns);

        ui.columns(columns, |uis| {
            for (index, chunk) in entries.chunks(per_column).enumerate() {
                let Some(column) = uis.get_mut(index) else {
                    continue;
                };
                for entry in chunk {
                    entry.slider(column, tokens, setter, text_entry);
                }
            }
        });
    });
}

fn tokens_for(ui: &Ui) -> Tokens {
    if ui.visuals().dark_mode {
        mxm_ui::DARK
    } else {
        mxm_ui::LIGHT
    }
}

/// Every non-hidden parameter, in `Parameters`-view order.
///
/// Grouped by section so the testing view reads in the same order as the instrument. A flat list
/// in declaration order would be a second, contradictory information architecture.
fn all_parameters(params: &MxmMono01Params) -> Vec<Bound<'_>> {
    let mut out = Vec::with_capacity(27);
    for section in SECTIONS {
        out.extend(sections::parameters_in(*section, params));
    }
    out
}

#[cfg(test)]
mod modulation_tests {
    #[test]
    fn a_double_click_aims_at_the_patch_while_something_else_sounds_the_parameter() {
        // The player's rule, seen from the editor: "put this back" means the patch, never the
        // factory default. Shipped wrong once - Range's base sat at 16', a lock sounded 2', and a
        // double-click went to 8', the factory default. The widget test verified the mechanism
        // without asking whether the target matched the model; this asks.
        let params = MxmMono01Params::default();
        use nice_plug::prelude::Params;
        let (_, ptr, _) = params
            .param_map()
            .into_iter()
            .find(|(id, _, _)| id == "oscrange")
            .expect("mxm-mono-01 has a range switch");

        let range = crate::editor::sections::all_parameters(&params)
            .into_iter()
            .find(|bound| bound.id == "oscrange")
            .expect("the range switch is on the panel")
            .param;

        // The patch is 16' (0.0); the factory default is 8' (1/3).
        unsafe { ptr._internal_set_normalized_value(0.0) };
        let factory = range.default_normalised();
        assert!(factory > 0.0, "the premise: default and patch differ");

        assert_eq!(
            crate::editor::binding::reset_target(range),
            factory,
            "at rest, double-click is 'init this control'"
        );

        // A lock sounds 2': modulation of two whole switch positions.
        unsafe { ptr._internal_modulate_value(2.0 / 3.0) };
        assert_eq!(
            crate::editor::binding::reset_target(range),
            0.0,
            "while sounding elsewhere, double-click puts back the patch - the base"
        );
    }

    use crate::editor::binding::ErasedParam;
    use crate::params::MxmMono01Params;
    #[allow(unused_imports)]
    use nice_plug::params::InternalParamMut;
    use nice_plug::prelude::Param;

    /// **The oracle for the whole modulation route.**
    ///
    /// The MXM player sends a step's deviation as `CLAP_EVENT_PARAM_MOD` rather than as a parameter
    /// value. Nothing on the *host* side can prove that arrived — `clap_params.get_value` reports the
    /// modulated value, so a host reading it back cannot tell an offset from a value. Only the
    /// instrument holds the two numbers apart, which is why this test lives here.
    #[test]
    fn a_modulated_parameter_keeps_its_own_value_and_reports_the_offset() {
        let params = MxmMono01Params::default();
        let before = params.cutoff.unmodulated_normalized_value();

        // What the wrapper does when a `PARAM_MOD` event arrives.
        unsafe {
            use nice_plug::params::internals::ParamPtr;
            let ptr = ParamPtr::FloatParam(std::ptr::from_ref(&params.cutoff).cast_mut());
            // **Downwards**, because cutoff's init value is 0.946 and adding 0.25 to it would
            // clamp at 1.0 - which would make this test about the clamp rather than about the
            // offset. The clamp has a test of its own below.
            ptr._internal_modulate_value(-0.25);
        }

        assert_eq!(
            params.cutoff.unmodulated_normalized_value(),
            before,
            "modulation must not disturb the parameter's own value - that is the whole point"
        );
        assert!(
            (params.cutoff.modulated_normalized_value() - (before - 0.25)).abs() < 1e-6,
            "and it must be laid over it"
        );
        assert!(
            (ErasedParam::modulation(&params.cutoff) + 0.25).abs() < 1e-6,
            "which is what the editor draws its mark from"
        );
    }

    #[test]
    fn modulation_is_clamped_into_range_and_that_bounds_what_a_step_can_ask_for() {
        // nice-plug applies it as `(unmodulated + offset).clamp(0, 1)`. The player never meets this,
        // because it sends `lock - patch` and both are in range, so the sum is the lock. Pinned so
        // that anyone tempted to send an absolute value as an offset finds out here rather than by
        // wondering why a filter is stuck open.
        let params = MxmMono01Params::default();
        unsafe {
            use nice_plug::params::internals::ParamPtr;
            let ptr = ParamPtr::FloatParam(std::ptr::from_ref(&params.cutoff).cast_mut());
            ptr._internal_modulate_value(5.0);
        }
        assert_eq!(params.cutoff.modulated_normalized_value(), 1.0);
    }

    #[test]
    fn an_unmodulated_parameter_is_not_marked() {
        // The other half: a mark that was always on would say nothing.
        let params = MxmMono01Params::default();
        assert_eq!(ErasedParam::modulation(&params.cutoff), 0.0);
    }

    #[test]
    fn a_knob_shows_the_value_and_not_the_value_plus_the_modulation() {
        // A control drawn at `value + modulation` would jump under your hand while the thing you are
        // setting sat still. The mark says something else is moving it; the position stays yours.
        let params = MxmMono01Params::default();
        let before = ErasedParam::normalised(&params.cutoff);

        unsafe {
            use nice_plug::params::internals::ParamPtr;
            let ptr = ParamPtr::FloatParam(std::ptr::from_ref(&params.cutoff).cast_mut());
            ptr._internal_modulate_value(-0.3);
        }

        assert_eq!(
            ErasedParam::normalised(&params.cutoff),
            before,
            "the knob does not move because something else is modulating it"
        );
    }
}

#[cfg(test)]
mod tests {
    use mxm_plugin_test::{opening_size, paging_checks};

    /// **The editor opens at the quarter-4K budget, hugged** — the owner's rule, 2026-09-09. The budget
    /// is the most room an editor may ask for, so laying the panel out there shows as many modules as
    /// it ever will; taking the slack away is the whole of the size.
    #[test]
    fn the_opening_size_is_the_budget_hugged() {
        let params = MxmMono01Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut view = 0usize;
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        // Every disclosure open: §4.2 measures the default from the rendered contents with them open.
        opening_size::is_the_budget_hugged(
            egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            &REVEAL,
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut view,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }

    /// **The app bar holds in the narrowest window**: its `…` menu whole and nothing drawn over
    /// anything else, from `MINIMUM` up (`opening_size::bar_holds_from_the_minimum`).
    #[test]
    fn the_app_bar_holds_in_the_minimum_window() {
        let params = MxmMono01Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut view = 0usize;
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        // Every disclosure open: §4.2 measures the default from the rendered contents with them open.
        opening_size::bar_holds_from_the_minimum(
            egui::vec2(MINIMUM.0 as f32, MINIMUM.1 as f32),
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut view,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }

    /// What this editor keeps behind a disclosure, opened: the Oscillator card's Advanced.
    const REVEAL: fn(&egui::Context) = |ctx| set_advanced(ctx, true);

    #[test]
    fn every_dynamic_page_fits_and_every_card_is_reachable() {
        let params = MxmMono01Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        // Advanced open, through the developer channel as a capture run would open it.
        telemetry.request_disclosure(true);
        let mut view = 0;
        let mut entries = HashMap::new();
        let mut nav = mxm_ui::navigation::State::default();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        paging_checks::verify(
            &test_items(),
            &[
                egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
                egui::vec2(1880.0, 1040.0),
                egui::vec2(MINIMUM.0 as f32, MINIMUM.1 as f32),
            ],
            |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut view,
                    &mut entries,
                    &mut presets,
                    &mut nav,
                )
            },
        );
    }
    use mxm_plugin_test::keyboard_checks;

    /// **The collection's coverage check, owed since the output level moved above the cards**
    /// (`plugins/AGENTS.md`: an editor with any surface above the cards owes it). Every parameter
    /// of the instrument's own and every routing pair registers with the keyboard cursor — the
    /// output level from the app bar's bar card — and the cursor lands and edits as one balanced
    /// gesture. `tests/keyboard_editing.rs` remains the deeper, table-backed proof.
    #[test]
    fn the_keyboard_cursor_reaches_and_operates_every_parameter() {
        let params = MxmMono01Params::default();
        // Every route present, because an absent one draws nothing at all.
        reveal_every_route(&params);
        let telemetry = Telemetry::default();
        let host = keyboard_checks::Recorder::default();
        let setter = ParamSetter::new(&host);
        let mut ids: Vec<&str> = sections::ASSIGNMENT.iter().map(|(id, _, _)| *id).collect();
        ids.extend(
            crate::routes::ROUTE_IDS
                .iter()
                .flatten()
                .flat_map(|(amount, present)| [*amount, *present]),
        );
        let mut view = 0usize;
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        // Advanced open, so `bendrange` is painted.
        keyboard_checks::the_cursor_reaches_and_operates(
            egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            &test_items(),
            keyboard_checks::Coverage::Exactly(&ids),
            &REVEAL,
            &host,
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut view,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }

    use super::*;
    use nice_plug::params::InternalParamMut;
    use nice_plug::params::internals::ParamPtr;
    use nice_plug::prelude::{PluginApi, PluginState};

    /// A `GuiContext` that records nothing, so the panel can be laid out with no host.
    ///
    /// The editor reports edits through a `ParamSetter`; laying it out does not make any, so every
    /// method here is unreachable in these tests and exists only to satisfy the trait.
    struct NoHost;

    impl nice_plug::context::gui::GuiContextInner for NoHost {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _param: ParamPtr, _normalized: f32) {}
        unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            PluginState {
                version: String::new(),
                params: Default::default(),
                fields: Default::default(),
            }
        }
        fn set_state(&self, _state: PluginState) {}
    }

    /// A host that **applies** what a `ParamSetter` reports, which `NoHost` deliberately does not.
    ///
    /// `NoHost` swallows every write, and that is right for a layout test: a setter *reports* to a
    /// host, and the host is what moves the parameter. A test about whether a preset reaches the
    /// sound therefore needs the other half, or it is testing that nothing happened.
    struct WritingHost;

    impl nice_plug::context::gui::GuiContextInner for WritingHost {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
            // SAFETY: the parameter outlives the setter, which borrows this host for the call.
            unsafe { param._internal_set_normalized_value(normalized) };
        }
        unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            PluginState {
                version: String::new(),
                params: Default::default(),
                fields: Default::default(),
            }
        }
        fn set_state(&self, _state: PluginState) {}
    }

    /// Applies a preset through the same path the browser does.
    ///
    /// **Through `apply_preset`, not by writing values directly.** What is being tested is the
    /// round trip a person actually gets — resolve, bracket, write — and a shortcut around it would
    /// test the shortcut.
    fn apply(params: &MxmMono01Params, preset: &crate::preset::Preset) {
        let host = WritingHost;
        let setter = ParamSetter::new(&host);
        mxm_preset::ui::apply_preset(params, &setter, preset);
    }

    #[test]
    fn a_factory_preset_actually_changes_the_sound() {
        // The end-to-end claim, and the one every layer below is in service of. `Noise sweep` is
        // the furthest from the defaults, so nothing here can pass by accident.
        let params = MxmMono01Params::default();
        let before = crate::preset::snapshot(&params);

        let noise = crate::preset::factory(&params)
            .into_iter()
            .find(|p| p.name == "Noise sweep")
            .expect("a factory preset");
        apply(&params, &noise);

        let after = crate::preset::snapshot(&params);
        assert_ne!(before, after, "the preset must reach the parameters");
        for (id, value) in &noise.params {
            let got = after.get(id).unwrap_or_else(|| panic!("`{id}` is missing"));
            assert!(
                (got - value.v).abs() < 1e-4,
                "`{id}` came out at {got}, not {}",
                value.v
            );
        }
    }

    #[test]
    fn loading_a_preset_leaves_the_patch_clean_and_editing_it_does_not() {
        // The whole dirty contract, through the real path. The baseline is captured **after**
        // applying, so a stepped parameter canonicalising its value cannot mark a fresh load dirty.
        let params = MxmMono01Params::default();
        let pad = crate::preset::factory(&params)
            .into_iter()
            .find(|p| p.name == "Soft pad")
            .expect("a factory preset");

        apply(&params, &pad);
        crate::preset::mark_loaded(&params, &pad.name, crate::preset::Origin::Factory);
        assert_eq!(
            crate::preset::loaded(&params),
            crate::preset::Loaded::Clean {
                name: "Soft pad".to_owned(),
                origin: crate::preset::Origin::Factory
            },
            "a preset is clean the instant it loads"
        );

        // Now move one parameter, exactly as a knob would.
        let host = WritingHost;
        let setter = ParamSetter::new(&host);
        for bound in sections::all_parameters(&params) {
            if bound.id == "cutoff" {
                bound.param.begin(&setter);
                bound.param.set(&setter, 0.1);
                bound.param.end(&setter);
            }
        }
        assert!(crate::preset::loaded(&params).is_modified());
    }

    #[test]
    fn init_from_the_browser_and_init_from_the_button_are_the_same_sound() {
        // They are one mechanism now rather than two that agree today: `Init` in the browser is
        // generated from the same defaults `init_patch` writes. If that ever stops being true this
        // is what says so.
        let generated = MxmMono01Params::default();
        let via_button = MxmMono01Params::default();

        // Move the second one somewhere else first, so `init_patch` has something to undo.
        let host = WritingHost;
        let setter = ParamSetter::new(&host);
        let sweep = crate::preset::factory(&via_button)
            .into_iter()
            .find(|p| p.name == "Noise sweep")
            .expect("a factory preset");
        mxm_preset::ui::apply_preset(&via_button, &setter, &sweep);
        assert_ne!(
            crate::preset::snapshot(&generated),
            crate::preset::snapshot(&via_button),
            "the premise: it has moved"
        );

        mxm_preset::ui::init_patch(&via_button, &setter);
        assert_eq!(
            crate::preset::snapshot(&via_button),
            crate::preset::snapshot(&generated),
            "Init must return exactly to the defaults the browser's Init carries"
        );
    }

    #[test]
    fn every_row_of_cards_shares_one_bottom_edge() {
        // The cards' bottoms line up across the three columns — the shorter columns pad the inside
        // of their last card until all three end where the tallest does. Measured from what is
        // actually painted, because the padding is invisible to everything else.
        let ctx = egui::Context::default();
        mxm_ui::theme::apply(&ctx);
        mxm_ui::typography::apply(&ctx);

        let params = MxmMono01Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut view = 0;
        let mut text_entry = HashMap::new();
        let mut nav = mxm_ui::navigation::State::default();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);

        let input = egui::RawInput {
            // Tall enough to paint every card. The Synth view scrolls now, and a card scrolled
            // out of sight is not in the paint output at all — which read as "four cards" when the
            // window was 700 points.
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 1600.0),
            )),
            ..Default::default()
        };

        // Three frames: the first measures the natural heights, the second draws the padding, the
        // third is what a settled panel paints.
        let mut cards: Vec<egui::Rect> = Vec::new();
        for _ in 0..3 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut view,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            });
            // A card is the one thing painted as a surface_1 rect of column width: the app bar is
            // full-width, and every smaller control surface is nowhere near 120 px tall.
            fn collect(shape: &egui::Shape, into: &mut Vec<egui::Rect>) {
                match shape {
                    egui::Shape::Rect(rect) => {
                        if rect.fill == mxm_ui::theme::DARK.surface_1
                            && rect.rect.width() < 520.0
                            && rect.rect.height() > 120.0
                        {
                            into.push(rect.rect);
                        }
                    }
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            collect(shape, into);
                        }
                    }
                    _ => {}
                }
            }
            cards.clear();
            for clipped in &output.shapes {
                collect(&clipped.shape, &mut cards);
            }
            output.textures_delta.clear();
        }

        assert!(!cards.is_empty(), "visible section cards: {cards:?}");

        // **Each row shares one bottom edge; rows do not share one with each other.** The old form
        // of this assertion held every *column* to the tallest one's foot — which a reflowing panel
        // has none of, and which reads as a demand that three rows of different content end at the
        // same place. §3.3's rule is per row.
        let rows = rows(&cards);
        assert!(!rows.is_empty(), "visible cards form rows: {cards:?}");
        for row in &rows {
            let low = row
                .iter()
                .map(egui::Rect::bottom)
                .fold(f32::INFINITY, f32::min);
            let high = row
                .iter()
                .map(egui::Rect::bottom)
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                high - low < 1.0,
                "a row of {} cards ends {:.1} points ragged: {row:?}",
                row.len(),
                high - low
            );
        }
    }

    #[test]
    fn opening_the_naming_row_does_not_move_the_layout() {
        // The window is a fixed size and the layout was measured without a naming row, so a row
        // injected into the flow has nowhere to come from - it squeezed itself and the Save and
        // Cancel buttons drew cropped. The row floats instead, so the panel underneath must paint
        // exactly the same with the row open as closed.
        let ctx = egui::Context::default();
        mxm_ui::theme::apply(&ctx);
        mxm_ui::typography::apply(&ctx);

        let params = MxmMono01Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut view = 0;
        let mut text_entry = HashMap::new();
        let mut nav = mxm_ui::navigation::State::default();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);

        let input = egui::RawInput {
            // Tall enough to paint every card. The Synth view scrolls now, and a card scrolled
            // out of sight is not in the paint output at all — which read as "four cards" when the
            // window was 700 points.
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 1600.0),
            )),
            ..Default::default()
        };

        fn cards(output: &egui::FullOutput) -> Vec<egui::Rect> {
            fn collect(shape: &egui::Shape, into: &mut Vec<egui::Rect>) {
                match shape {
                    egui::Shape::Rect(rect) => {
                        if rect.fill == mxm_ui::theme::DARK.surface_1
                            && rect.rect.width() < 520.0
                            && rect.rect.height() > 120.0
                        {
                            into.push(rect.rect);
                        }
                    }
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            collect(shape, into);
                        }
                    }
                    _ => {}
                }
            }
            let mut found = Vec::new();
            for clipped in &output.shapes {
                collect(&clipped.shape, &mut found);
            }
            found
        }

        // `mut`, because the closure now also carries the keyboard cursor across its frames.
        let mut run =
            |presets: &mut PresetUi,
             view: &mut usize,
             text_entry: &mut HashMap<&'static str, Option<String>>| {
                let mut last = None;
                for _ in 0..3 {
                    let mut output = ctx.run_ui(input.clone(), |ui| {
                        panel(
                            ui, &params, &telemetry, &setter, view, text_entry, presets, &mut nav,
                        );
                    });
                    output.textures_delta.clear();
                    last = Some(cards(&output));
                }
                last.expect("three frames ran")
            };

        let closed = run(&mut presets, &mut view, &mut text_entry);
        assert!(!closed.is_empty(), "a visible page to compare");

        presets.open_save_as("join in the chant");
        let open = run(&mut presets, &mut view, &mut text_entry);
        assert!(presets.is_naming(), "the row is still open");

        assert_eq!(
            closed, open,
            "the naming row must float: opening it moved the cards"
        );
    }

    #[test]
    fn a_modulated_button_row_shows_which_cell_is_sounding() {
        // "When I lock a range button, I cannot see it on the step, but I can hear it." A knob
        // under modulation draws an arc; a segmented control drew nothing - the selection is the
        // *base*, so the step's choice was audible and invisible. The sounding cell now gets a
        // ring in the reduced-alpha accent this system reserves for modulation.
        let params = MxmMono01Params::default();

        let count_rings = |params: &MxmMono01Params| {
            let ctx = egui::Context::default();
            mxm_ui::paging::editor::request_category(&ctx, mxm_ui::paging::Category::Generators);
            mxm_ui::theme::apply(&ctx);
            mxm_ui::typography::apply(&ctx);

            let telemetry = Telemetry::default();
            let host = NoHost;
            let setter = ParamSetter::new(&host);
            let mut view = 0;
            let mut text_entry = HashMap::new();
            let mut nav = mxm_ui::navigation::State::default();
            let mut presets = PresetUi::at(crate::preset::Library::at(None), params);

            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                ..Default::default()
            };

            let ring = mxm_ui::theme::DARK.accent.gamma_multiply(0.6);
            fn collect(shape: &egui::Shape, ring: egui::Color32, hits: &mut usize) {
                match shape {
                    egui::Shape::Rect(rect) if rect.stroke.color == ring => *hits += 1,
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            collect(shape, ring, hits);
                        }
                    }
                    _ => {}
                }
            }

            let mut hits = 0;
            for _ in 0..2 {
                let mut output = ctx.run_ui(input.clone(), |ui| {
                    panel(
                        ui,
                        params,
                        &telemetry,
                        &setter,
                        &mut view,
                        &mut text_entry,
                        &mut presets,
                        &mut nav,
                    );
                });
                hits = 0;
                for clipped in &output.shapes {
                    collect(&clipped.shape, ring, &mut hits);
                }
                output.textures_delta.clear();
            }
            hits
        };

        assert_eq!(count_rings(&params), 0, "nothing modulated, nothing ringed");
        let circles_before = painted_circles(&params, egui::ThemePreference::Dark);

        // What the wrapper does when the sequencer's PARAM_MOD for a locked Range arrives: one
        // whole step of the four-position switch, up.
        use nice_plug::prelude::Params;
        let (_, ptr, _) = params
            .param_map()
            .into_iter()
            .find(|(id, _, _)| id == "oscrange")
            .expect("mxm-mono-01 has a range switch");
        unsafe {
            ptr._internal_modulate_value(1.0 / 3.0);
        }

        assert_eq!(
            count_rings(&params),
            1,
            "exactly the sounding Range cell is ringed"
        );
        assert_eq!(
            painted_circles(&params, egui::ThemePreference::Dark),
            circles_before + 1,
            "and the row carries exactly one new mark dot"
        );
    }

    /// Every filled circle the panel paints, for a params set the caller has already modulated.
    ///
    /// The mark is painted rather than being a widget, so there is no node to find it by — the same
    /// reason the player's own panel grew a `painted_circles` oracle. A mark nothing can see is a
    /// mark that can silently stop being drawn.
    fn painted_circles(params: &MxmMono01Params, theme: egui::ThemePreference) -> usize {
        let ctx = egui::Context::default();
        mxm_ui::theme::apply(&ctx);
        mxm_ui::typography::apply(&ctx);
        ctx.set_theme(theme);
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut view = 0;
        let mut text_entry = HashMap::new();
        let mut nav = mxm_ui::navigation::State::default();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), params);
        // Count marks across the whole surface, independently of physical page fit.
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 20000.0),
            )),
            ..Default::default()
        };

        let mut count = 0;
        for _ in 0..2 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                panel(
                    ui,
                    params,
                    &telemetry,
                    &setter,
                    &mut view,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            });
            count = output
                .shapes
                .iter()
                .map(|clipped| count_circles(&clipped.shape))
                .sum();
            output.textures_delta.clear();
        }
        count
    }

    /// Every line segment painted in the accent at the delta line's alpha — the delta lines.
    ///
    /// Counted by colour because the panel paints many ordinary lines (markers, detents, borders);
    /// the reduced-alpha accent is used for nothing else.
    fn count_delta_lines(shape: &egui::Shape, accent: egui::Color32) -> usize {
        let delta = accent.gamma_multiply(0.6);
        match shape {
            egui::Shape::LineSegment { stroke, .. } => usize::from(stroke.color == delta),
            egui::Shape::Path(path) => {
                usize::from(path.stroke.color == egui::epaint::ColorMode::Solid(delta))
            }
            egui::Shape::Vec(shapes) => shapes.iter().map(|s| count_delta_lines(s, accent)).sum(),
            _ => 0,
        }
    }

    fn painted_delta_lines(params: &MxmMono01Params, theme: egui::ThemePreference) -> usize {
        painted_delta_lines_with(params, theme, |_| {})
    }

    fn painted_delta_lines_with(
        params: &MxmMono01Params,
        theme: egui::ThemePreference,
        prepare: impl FnOnce(&egui::Context),
    ) -> usize {
        let ctx = egui::Context::default();
        mxm_ui::theme::apply(&ctx);
        mxm_ui::typography::apply(&ctx);
        ctx.set_theme(theme);
        prepare(&ctx);
        let accent = if matches!(theme, egui::ThemePreference::Dark) {
            mxm_ui::DARK.accent
        } else {
            mxm_ui::LIGHT.accent
        };

        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut view = 0;
        let mut text_entry = HashMap::new();
        let mut nav = mxm_ui::navigation::State::default();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), params);

        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };

        let mut count = 0;
        for _ in 0..2 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                panel(
                    ui,
                    params,
                    &telemetry,
                    &setter,
                    &mut view,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            });
            count = output
                .shapes
                .iter()
                .map(|clipped| count_delta_lines(&clipped.shape, accent))
                .sum();
            output.textures_delta.clear();
        }
        count
    }

    #[test]
    fn a_drag_in_progress_draws_its_delta_before_anything_has_played() {
        // **Reported: "It only shows the delta line after having played the sequence once. It
        // should show while dragging it."** The line was drawn only from host modulation, which
        // does not exist until the sequencer has actually applied an offset — so authoring was
        // blind. While a gesture is open the editor now draws the distance back to where the drag
        // began, which in the common case is the patch: the deviation being authored, live.
        //
        // Driven the way a drag actually happens: the origin is remembered in egui's own memory at
        // gesture start, so the panel is laid out with a stored origin and a moved parameter, with
        // **no modulation anywhere** — exactly the state mid-drag, before any sequence has played.
        let params = MxmMono01Params::default();
        let before = painted_delta_lines(&params, egui::ThemePreference::Light);

        // A drag on Attack: origin remembered, parameter moved, no host modulation.
        let origin = params.attack.unmodulated_normalized_value();
        unsafe {
            params
                .attack
                ._internal_set_normalized_value((origin + 0.3).min(1.0));
        }

        let after = painted_delta_lines_with(&params, egui::ThemePreference::Light, |ctx| {
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new(("mxm-drag-origin", "attack")), origin);
            });
        });

        assert_eq!(
            after,
            before + 1,
            "a drag in progress draws its own delta: {before} then {after}"
        );
    }

    #[test]
    fn a_modulated_slider_draws_its_delta_line() {
        // **Reported: "It does not show a delta line on sliders."** The knob had its arc and the
        // slider had only the dot — but mxm-mono-01's mixer and envelope are sliders, so a sequenced
        // level or attack moved audibly with nothing on screen saying so.
        //
        // Attack is a slider in the envelope card; modulating it must add exactly one delta line.
        let params = MxmMono01Params::default();
        let before = painted_delta_lines(&params, egui::ThemePreference::Light);

        unsafe {
            use nice_plug::params::internals::ParamPtr;
            let ptr = ParamPtr::FloatParam(std::ptr::from_ref(&params.attack).cast_mut());
            ptr._internal_modulate_value(0.3);
        }
        let after = painted_delta_lines(&params, egui::ThemePreference::Light);

        assert_eq!(
            after,
            before + 1,
            "one modulated slider, one delta line: {before} then {after}"
        );
    }

    fn count_circles(shape: &egui::Shape) -> usize {
        match shape {
            egui::Shape::Circle(circle) => usize::from(circle.fill != egui::Color32::TRANSPARENT),
            egui::Shape::Vec(shapes) => shapes.iter().map(count_circles).sum(),
            _ => 0,
        }
    }

    #[test]
    fn a_modulated_parameter_is_marked_in_both_themes() {
        // The visible half of the feature: something else is moving this knob, and the panel says so.
        // **Both themes**, because §15's QA gate asks for both and a mark drawn in one only is a
        // mark that is missing for half the people using it.
        for theme in [egui::ThemePreference::Light, egui::ThemePreference::Dark] {
            let params = MxmMono01Params::default();
            let before = painted_circles(&params, theme);

            unsafe {
                use nice_plug::params::internals::ParamPtr;
                let ptr = ParamPtr::FloatParam(std::ptr::from_ref(&params.cutoff).cast_mut());
                ptr._internal_modulate_value(-0.25);
            }
            let after = painted_circles(&params, theme);

            assert_eq!(
                after,
                before + 1,
                "one modulated parameter, one mark in {theme:?}: {before} then {after}"
            );
        }
    }

    /// Lays the whole editor out at a given size — with the Oscillator card's Advanced expander
    /// **open** when `expanded` says so, the state the frame has to fit — and hands back the
    /// context and the height it actually needed.
    fn lay_out(view: usize, width: f32, height: f32, expanded: bool) -> (egui::Context, f32) {
        let ctx = egui::Context::default();
        mxm_ui::theme::apply(&ctx);
        mxm_ui::typography::apply(&ctx);
        ctx.set_theme(egui::ThemePreference::Light);

        let params = MxmMono01Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut view = view;
        let mut text_entry = HashMap::new();
        if expanded {
            set_advanced(&ctx, true);
        }
        // Rooted nowhere: a layout test must not read, and certainly must not write, the config
        // directory of whoever is running it.
        let mut nav = mxm_ui::navigation::State::default();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);

        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            ..Default::default()
        };

        // Three passes: the first sizes text galleys and lays panels out against defaults, the
        // columns are levelled against what the frame before measured, and the third is stable.
        let mut used = 0.0;
        for _ in 0..3 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut view,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            });
            // epaint asserts on a dropped `TexturesDelta` that was never applied, and there is no
            // renderer here to apply one. Clearing it is what that assertion documents as the
            // intentional way out.
            output.textures_delta.clear();
            used = ctx.globally_used_rect().height();
        }
        (ctx, used)
    }

    /// What the editor drew at a given size, with everything that expands open.
    fn measure(view: usize, width: f32, height: f32) -> f32 {
        lay_out(view, width, height, true).1
    }

    /// Where every card landed on the Synth view, at a given width.
    fn placed(width: f32, expanded: bool) -> Vec<egui::Rect> {
        // Tall component canvas: real physical fit is checked on every derived page above.
        let (ctx, _) = lay_out(0, width, 20000.0, expanded);
        paging_checks::all_rects(&ctx, SECTIONS.len())
    }

    /// Cards whose vertical extents overlap are on the same row, read off the geometry rather than
    /// off the packing — it is what a person sees, and it is what catches a layout that is right in
    /// taffy and wrong on screen.
    fn rows(placed: &[egui::Rect]) -> Vec<Vec<egui::Rect>> {
        let mut sorted: Vec<egui::Rect> = placed.to_vec();
        sorted.sort_by(|a, b| {
            a.top()
                .partial_cmp(&b.top())
                .unwrap()
                .then(a.left().partial_cmp(&b.left()).unwrap())
        });
        let mut rows: Vec<Vec<egui::Rect>> = Vec::new();
        for rect in sorted {
            match rows.last_mut() {
                Some(row) if row.iter().any(|r| r.bottom() > rect.top() + 1.0) => row.push(rect),
                _ => rows.push(vec![rect]),
            }
        }
        rows
    }

    /// The widths a window plausibly gets: a third, a half and a whole of a 1920 screen, and the
    /// collection's own reference.
    const WIDTHS: [f32; 5] = [MINIMUM.0 as f32, 640.0, 900.0, 1200.0, 1600.0];

    /// **The layout is rows.** Every card in a row starts on one line and ends on one line, and a
    /// row is no taller than the tallest card in it.
    ///
    /// This replaced *the three columns share one bottom edge*. The editor has no columns now, and
    /// a column test would have gone on passing while nothing lined up across the panel — which is
    /// exactly how the first attempt at this layout shipped looking like scattered boxes.
    #[test]
    fn the_cards_are_laid_out_in_rows() {
        for expanded in [false, true] {
            for width in WIDTHS {
                for row in rows(&placed(width, expanded)) {
                    if row.len() < 2 {
                        continue;
                    }
                    let top = row
                        .iter()
                        .map(egui::Rect::top)
                        .fold(f32::NEG_INFINITY, f32::max)
                        - row
                            .iter()
                            .map(egui::Rect::top)
                            .fold(f32::INFINITY, f32::min);
                    let bottom = row
                        .iter()
                        .map(egui::Rect::bottom)
                        .fold(f32::NEG_INFINITY, f32::max)
                        - row
                            .iter()
                            .map(egui::Rect::bottom)
                            .fold(f32::INFINITY, f32::min);
                    assert!(
                        top < 1.0 && bottom < 1.0,
                        "expanded {expanded} at {width}: a row of {} cards is {top:.1} ragged at \
                         the top and {bottom:.1} at the foot",
                        row.len()
                    );
                }
            }
        }
    }

    /// Nothing is clipped, nothing is squeezed under its floor, and nothing overlaps — at any width.
    #[test]
    fn the_layout_reflows_without_clipping_or_overlap() {
        let floors = test_floors();
        for width in WIDTHS {
            let placed = placed(width, true);
            for (index, rect) in placed.iter().enumerate() {
                assert!(
                    rect.left() >= -0.5 && rect.right() <= width + 0.5,
                    "{} runs from {:.1} to {:.1} in a {width} point window",
                    SECTIONS[index].title(),
                    rect.left(),
                    rect.right()
                );
                assert!(
                    (rect.width() - floors[index]).abs() <= 0.5,
                    "{} is {:.1} wide, not its floor {:.1}",
                    SECTIONS[index].title(),
                    rect.width(),
                    floors[index]
                );
            }
            for (i, a) in placed.iter().enumerate() {
                for b in placed.iter().skip(i + 1) {
                    let overlap = a.intersect(*b);
                    assert!(
                        overlap.width() <= 0.5 || overlap.height() <= 0.5,
                        "two cards overlap at {width} points"
                    );
                }
            }
        }
    }

    /// Reveals every route on every target, as the `‹ modulate ›` menu would one at a time.
    ///
    /// Through [`WritingHost`], because `add` reports the write to a host and `NoHost` swallows it:
    /// with the setter a layout test would otherwise hand it, this would reveal nothing at all and
    /// the test would quietly measure the init patch twice.
    fn reveal_every_route(params: &MxmMono01Params) {
        let host = WritingHost;
        let setter = ParamSetter::new(&host);
        for (index, group) in params.routes.each().into_iter().enumerate() {
            for route in &group.routes(index) {
                mxm_modulation_params::add(route, &setter);
            }
        }
    }

    /// **A route's remove sits on its slider's track, not above it.**
    ///
    /// The owner's report was that the cross floated. It did: a row centres what it holds, and a
    /// slider is a name/value line stacked over its track, so the cross — which is only as tall as
    /// the track — came to rest half a name line high.
    ///
    /// The oracle is the keyboard cursor's own registry rather than the painted shapes, because
    /// both halves of a route register there under the parameter they set, and what a slider
    /// registers is its **track** rect specifically. So the two Spots keyed by one route's amount
    /// and presence are exactly the two boxes that have to line up, named rather than guessed at.
    /// Both are `MIN_TARGET` tall, so equal centres and equal bottoms are the same claim.
    ///
    /// Verified against the defect: with the cross placed by a plain `ui.horizontal`, the presence
    /// centre sits above the amount centre by half the name line and this fails.
    #[test]
    fn a_routes_remove_is_vertically_centred_on_its_sliders_track() {
        // The whole editor, through the real paging renderer, because the registry is only written
        // inside a card scope and that scope belongs to the renderer. A tall canvas so every card
        // is on the page and every route present in the init patch is actually drawn.
        let (ctx, _) = lay_out(0, 1400.0, 20_000.0, true);
        let params = MxmMono01Params::default();

        let spots = mxm_ui::navigation::spots(&ctx);
        let at = |key: &str| {
            spots
                .iter()
                .find(|spot| spot.key == key)
                .map(|spot| spot.rect)
        };

        let mut checked = 0;
        for (index, group) in params.routes.each().into_iter().enumerate() {
            for route in &group.routes(index) {
                let (Some(track), Some(cross)) = (at(route.amount_id), at(route.present_id)) else {
                    continue;
                };
                assert!(
                    (track.center().y - cross.center().y).abs() <= 0.5,
                    "`{}`'s remove is {:.1} points off its track: track centre {:.1}, cross \
                     centre {:.1}",
                    route.present_id,
                    (track.center().y - cross.center().y).abs(),
                    track.center().y,
                    cross.center().y
                );
                checked += 1;
            }
        }
        assert!(
            checked > 0,
            "no route drew both a track and a cross, so this asserted nothing"
        );
    }

    /// **The output level is painted once, in the app bar** (design system §3.1 item 6), and not
    /// on the card that holds the amplifier. Read off the keyboard cursor's registry, which is
    /// built by painting, on a canvas tall enough to draw every card.
    #[test]
    fn the_output_level_is_drawn_once_in_the_app_bar() {
        let (ctx, _) = lay_out(0, 1400.0, 20_000.0, true);
        let spots = mxm_ui::navigation::spots(&ctx);
        let output: Vec<_> = spots.iter().filter(|spot| spot.key == "outgain").collect();
        assert_eq!(output.len(), 1, "Output is painted {} times", output.len());
        assert_eq!(output[0].card, OUTPUT_CARD, "Output is the app bar's");
        // The amplifier's card was painted too, so the output level's absence from it is a finding
        // rather than a card that never drew.
        assert!(
            spots
                .iter()
                .any(|spot| spot.key == "vcasource" && spot.card != OUTPUT_CARD),
            "the amplifier's card was not painted"
        );
        assert!(
            output[0].rect.bottom()
                <= spots
                    .iter()
                    .filter(|spot| spot.card != OUTPUT_CARD)
                    .map(|spot| spot.rect.top())
                    .fold(f32::INFINITY, f32::min),
            "Output sits above every card, in the bar"
        );
    }

    /// The window opens at a size the layout asks for, and may shrink to one card.
    ///
    /// A reflowing editor has no fixed frame, but it still has a size worth opening at and a floor
    /// below which a card would be drawn narrower than its own controls (§4.3).
    #[test]
    fn the_frame_is_the_layouts_own() {
        let widest = test_floors().into_iter().fold(0.0_f32, f32::max);
        assert!(
            MINIMUM.0 as f32 >= widest,
            "the minimum {} is under the widest card's floor of {widest:.0}",
            MINIMUM.0
        );
        assert!(
            (MINIMUM.0 as f32) < REFERENCE.0 as f32,
            "the minimum is not below the size it opens at"
        );

        // Total unpaged height no longer sizes the window; every page is checked above.
        every_dynamic_page_fits_and_every_card_is_reachable();
    }

    /// `Parameters` is allowed to be taller: it is the one view that scrolls.
    #[test]
    fn the_parameters_view_lays_out_without_panicking() {
        let used = measure(
            mxm_ui::paging::PARAMETERS,
            REFERENCE.0 as f32,
            REFERENCE.1 as f32,
        );
        assert!(used > 0.0);
    }

    use mxm_plugin_test::tree_checks;

    /// **The layout lab's entry draws each card as its tree.** `apps/mxm-layout-lab` calls
    /// `sections::draw` with its own state, disclosure map included; the wrapper builds the card's
    /// tree and shows it, so the card the lab draws is as tall as that tree says, at the floor the
    /// paging renderer is given.
    #[test]
    fn the_layout_labs_entry_draws_each_card_as_its_tree() {
        let floors = test_floors();
        for (index, section) in SECTIONS.iter().enumerate() {
            let params = MxmMono01Params::default();
            let telemetry = Telemetry::default();
            let host = NoHost;
            let setter = ParamSetter::new(&host);
            let mut text_entry = HashMap::new();
            let mut disclosed: HashMap<&'static str, bool> = HashMap::new();
            let ctx = tree_checks::context(&|_| {});
            let mut result = (0.0, 0.0);
            for _ in 0..3 {
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let mut column =
                        ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(floors[index], 4000.0),
                        )));
                    result = mxm_ui::ModuleCard::new(section.title()).show(
                        &mut column,
                        &mxm_ui::LIGHT,
                        |ui| {
                            let tree = sections::card(ui, *section, &params);
                            let stated =
                                tree.height(ui, tree.drawn_width(ui, ui.available_width()));
                            let top = ui.min_rect().top();
                            sections::draw(
                                ui,
                                &mxm_ui::LIGHT,
                                *section,
                                &params,
                                &telemetry,
                                &setter,
                                &mut text_entry,
                                &mut disclosed,
                            );
                            (stated, ui.min_rect().bottom() - top)
                        },
                    );
                });
                output.textures_delta.clear();
            }
            let (stated, drawn) = result;
            assert!(
                (stated - drawn).abs() < 0.5,
                "{}: the tree says {stated:.1}, the lab's entry drew {drawn:.1}",
                section.title()
            );
        }
    }

    /// Every card, in every state that changes what it holds, passes the layout tree's checks
    /// (plans/plan-layout-tree.md §4.3, `tree_checks::card`): its floor holds its content with
    /// nothing painted outside the card, the content floor is exact, the height its tree states is
    /// the height it draws, and every leaf stays in the room it was given.
    ///
    /// The states are this editor's structural-state matrix: the init patch, where the machine's
    /// own five routes are the only rows; every route revealed at full negative depth — where a
    /// reading carries its sign and every digit, the widest text a row can show; each of those with
    /// the Oscillator's Advanced disclosure closed and open; and a note sounding, the one thing
    /// telemetry adds to a card — the envelope's position marker; and the LFO synced, with no tempo
    /// and with one, where its rate reads a division. Nothing else changes a card's structure: the
    /// panels' heights follow only the width.
    #[test]
    fn every_card_passes_the_tree_checks_in_every_state() {
        let floors = test_floors();
        for state in [
            "init",
            "Advanced open",
            "every route revealed",
            "every route revealed, Advanced open",
            "a note sounding",
            "LFO synced, no tempo",
            "LFO synced to a tempo",
        ] {
            let params = MxmMono01Params::default();
            let host = WritingHost;
            let setter = ParamSetter::new(&host);
            if state.starts_with("every route revealed") {
                reveal_every_route(&params);
                for (index, group) in params.routes.each().into_iter().enumerate() {
                    for route in &group.routes(index) {
                        route.amount.set(&setter, 0.0);
                    }
                }
            }
            let telemetry = Telemetry::default();
            if state == "a note sounding" {
                telemetry.publish_envelope(1.0, mxm_mono_01_dsp::envelope::Stage::Attack);
            }
            if state.starts_with("LFO synced") {
                // SAFETY: the parameters are this test's own and nothing else reads them.
                unsafe {
                    use nice_plug::params::InternalParamMut;
                    let _ = params.lfo_sync._internal_set_normalized_value(1.0);
                }
            }
            if state == "LFO synced to a tempo" {
                telemetry.tempo.publish(Some(120.0));
            }
            let open = state.ends_with("Advanced open");
            let setup = move |ctx: &egui::Context| set_advanced(ctx, open);
            for (index, section) in SECTIONS.iter().enumerate() {
                let mut entries = HashMap::new();
                let mut live = sections::Live {
                    params: &params,
                    telemetry: &telemetry,
                    setter: &setter,
                    entries: &mut entries,
                };
                tree_checks::card(
                    &setup,
                    state,
                    section.title(),
                    floors[index],
                    &|ui| sections::card(ui, *section, &params),
                    &mut |ui, leaf, rect| {
                        sections::paint(ui, &mxm_ui::LIGHT, leaf, rect, &mut live);
                    },
                );
            }
        }
    }

    /// Every page at the opening size, light and dark, for the owner's review of the layout-tree
    /// conversion (plans/plan-layout-tree.md §4.3): `target/layout-tree/mxm-mono-01/<tag>/`, where
    /// `MXM_PICTURES` names the tag — `before` on the unconverted editor, `after` on the tree.
    ///
    /// `MXM_PICTURES=after cargo test -p mxm-mono-01 --lib tree_pictures -- --ignored`
    #[test]
    #[ignore = "renders through wgpu; run by hand"]
    fn tree_pictures() {
        let tag = std::env::var("MXM_PICTURES").unwrap_or_else(|_| "after".to_owned());
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/layout-tree/mxm-mono-01")
            .join(tag);
        let params = MxmMono01Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut view = 0usize;
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        tree_checks::pictures(
            &|_| {},
            egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            &dir,
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut view,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }
}
