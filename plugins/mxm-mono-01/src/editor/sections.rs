//! What each of the four cards contains — the brief's six sections — and at what surface.
//!
//! This module is `docs/briefs/mxm-mono-01.md` §5 and §10 expressed as code. Both tables are
//! reproduced here — the section a parameter belongs to, and whether it is primary or behind a
//! labelled disclosure — because a layout that agrees with the brief only by coincidence is a
//! layout that stops agreeing with it at the first refactor.
//!
//! `every_parameter_is_assigned_exactly_once` in the tests is the mechanical check: **every
//! parameter id of the instrument's own, each in exactly one section, each at exactly one surface**
//! — its card, the card's disclosure, or the app bar. Revision 1 of the plan miscounted the section
//! list by hand and the surface list survived unnoticed to revision 8, so neither is trusted to a
//! reading.
//!
//! # Each card is a tree
//!
//! A card's body is described once, as a `mxm_ui::tree` ([`card`]), and that one description is
//! both measured — the card's floor and its height, by the paging renderer — and drawn, leaf by
//! leaf, through the bindings ([`paint`]). Nothing is typed and nothing is drawn to learn a size
//! (`plans/plan-layout-tree.md`).

use std::collections::HashMap;

use egui::{Rect, Ui};
use mxm_ui::control::{Size, Wave};
use mxm_ui::space::{SPACE_2, SPACE_3};
use mxm_ui::theme::Tokens;
use mxm_ui::tree::{self, Height, Kind, Node, leaf, pad, pad_all, row_gap, stack};
use nice_plug::prelude::ParamSetter;

use super::binding::{Bound, segmented_named, segmented_waves_marked, segmented_waves_named};
use super::{Section, visuals};
use crate::params::MxmMono01Params;
use crate::telemetry::Telemetry;

/// §7.1's sizing, as used here.
///
/// The §2 priority parameters sit at **Standard**, which §7.1 permits alongside Primary, and the
/// amounts that act on them at **Compact**. `Primary` is deliberately unused: at 64 px it is the
/// difference between the filter's five controls sitting on one row and wrapping onto three, and
/// a row that reads as one group is worth more here than 16 px of knob.
const STANDARD: Size = Size::Standard;
const COMPACT: Size = Size::Compact;

/// The Oscillator card's disclosure: its label, which is also where its open state is kept —
/// [`mxm_ui::shell::disclosure_id`], in the editor's egui memory, where the developer channel's
/// CC 118 writes it too.
pub const ADVANCED: &str = "Advanced";

/// What the disclosure's switch says it is for.
const ADVANCED_DESCRIPTION: &str = "Controls you set once and leave.";

/// The LFO's shapes: all five, from `params::LfoWave`. Three were offered at first, which left
/// **Ramp up and Ramp down unreachable from the editor** — the parameter had them, the panel did
/// not. The debug assertion is what caught it; §7.3's limit is five, so this is exactly at it, and a
/// sixth shape would have to become a menu.
///
/// The names stay: they are the hover text and the accessible name, which a drawing cannot be. A
/// picture on screen and a word for everything that cannot see it.
const LFO_SHAPES: &[(Wave, &str)] = &[
    (Wave::Triangle, "Triangle"),
    (Wave::Square, "Square"),
    (Wave::RampUp, "Ramp up"),
    (Wave::RampDown, "Ramp down"),
    (Wave::Random, "Random"),
];

/// The sub-oscillator's shapes: pictures with the octave beside each (the owner, 2026-09-23), since
/// two of the three are the same square an octave apart, so a picture alone would draw two
/// identical cells. The names are the plugin's own, from `params::SubKind` — the hover text and the
/// accessible name, which §7.1 keeps equal to host automation. They were wrong once — "-1 oct
/// pulse" for what the DSP documents as two octaves down.
const SUB_SHAPES: &[(Wave, &str)] = &[
    (Wave::Square, "1 oct square"),
    (Wave::Square, "2 oct square"),
    (Wave::Pulse, "2 oct pulse"),
];
const SUB_MARKS: &[&str] = &["\u{2212}1", "\u{2212}2", "\u{2212}2"];

/// A control as its card draws it: the binding, with the sentence its tooltip carries on the card.
///
/// [`binding_for`] is the same parameter for the `Parameters` list and the tables, in one line
/// each; these are the card's own, longer descriptions, with the laws and polarity the card draws
/// with. Anything a card does not draw falls through to it.
fn on_card<'a>(id: &'static str, p: &'a MxmMono01Params) -> Bound<'a> {
    card_binding(id, p).details(details_of(id))
}

/// What each option of a stepped control does, one sentence per cell in the parameter's own order
/// (design system §7.3; the owner, 2026-09-27: the cells of a row do not share one sentence).
/// Empty for everything drawn as a knob, slider or toggle.
fn details_of(id: &str) -> &'static [&'static str] {
    match id {
        "oscrange" => &[
            "One octave below the note played.",
            "The note as played.",
            "One octave above the note played.",
            "Two octaves above the note played.",
        ],
        "subtype" => &[
            "A square one octave down: adds weight.",
            "A square two octaves down: deeper still.",
            "A thin pulse two octaves down: a reedier low end.",
        ],
        "lfowave" => &[
            "Rises and falls smoothly: vibrato and sweeps.",
            "Jumps between two values, like a trill.",
            "Rises, then snaps back down.",
            "Falls, then snaps back up.",
            "A new random value on every cycle.",
        ],
        "vcasource" => &[
            "The envelope shapes the volume of each note.",
            "Full volume while a key is held, silent once it is released.",
        ],
        "retrigger" => &[
            "Notes played over a held key glide in without a new attack.",
            "Every new note restarts the envelope.",
        ],
        _ => &[],
    }
}

fn card_binding<'a>(id: &'static str, p: &'a MxmMono01Params) -> Bound<'a> {
    match id {
        "lforate" => Bound::new(
            id,
            &p.lfo_rate,
            "How fast the LFO cycles. Slow rates sweep; fast rates buzz.",
        ),
        "lfowave" => Bound::new(
            id,
            &p.lfo_wave,
            "The LFO's shape. Triangle glides, square jumps, the ramps sweep one way and reset, \
             and random steps to a new value each cycle.",
        ),
        "oscrange" => Bound::new(
            id,
            &p.osc_range,
            "The oscillator's octave; 8' is the note as played.",
        ),
        "pulsewidth" => Bound::new(
            id,
            &p.pulse_width,
            "How narrow the pulse wave is: 50 % is hollow and square, the extremes thin and nasal.",
        ),
        "glide" => Bound::new(
            id,
            &p.glide,
            "Portamento: how long the pitch takes to slide from the last note to the new one.",
        ),
        "subtype" => Bound::new(
            id,
            &p.sub_type,
            "The sub-oscillator's shape and how far below the main oscillator it sits.",
        ),
        "tune" => Bound::new(
            id,
            &p.tune,
            "Fine pitch trim, for matching another instrument.",
        )
        .bipolar()
        .law(mxm_preset::StepLaw::Cents),
        // Not on the original: the SH-101's bender had its own depth sliders, and a
        // semitones-per-deflection value is a MIDI-era addition to that idea.
        "bendrange" => Bound::new(
            id,
            &p.bend_range,
            "How many semitones a full pitch-bend deflection covers.",
        )
        .law(mxm_preset::StepLaw::Semitones),
        "sawlevel" => Bound::new(
            id,
            &p.saw_level,
            "Sawtooth level. The bright, buzzy source most bass and lead patches start from.",
        ),
        "pulselevel" => Bound::new(
            id,
            &p.pulse_level,
            "Pulse level. Hollow and reedy, and the only source the pulse width affects.",
        ),
        "sublevel" => Bound::new(
            id,
            &p.sub_level,
            "Sub-oscillator level. This is what makes a bass line carry on a small speaker.",
        ),
        "noiselevel" => Bound::new(
            id,
            &p.noise_level,
            "Noise level. Adds breath and grit, and is what percussive patches are built from.",
        ),
        "cutoff" => Bound::new(
            id,
            &p.cutoff,
            "Where the filter starts removing high frequencies.",
        )
        .law(mxm_preset::StepLaw::Hertz),
        "resonance" => Bound::new(
            id,
            &p.resonance,
            "Emphasis at the cutoff. High settings whistle, and thin the bass as they do.",
        ),
        // The original's VCA ENV/GATE switch. On the panel.
        "vcasource" => Bound::new(
            id,
            &p.vca_source,
            "Whether the envelope shapes the volume, or the note simply switches it on and off.",
        ),
        "attack" => Bound::new(
            id,
            &p.attack,
            "How long the sound takes to reach full level after a key is pressed.",
        ),
        "decay" => Bound::new(
            id,
            &p.decay,
            "How long it takes to fall from full level to the sustain level.",
        ),
        "sustain" => Bound::new(id, &p.sustain, "The level held while a key stays down."),
        "release" => Bound::new(
            id,
            &p.release,
            "How long the sound takes to fade after a key is let go.",
        ),
        // On the panel, not behind Advanced. It was disclosed as an addition the original lacked;
        // the owner ruled otherwise — the SH-101's own GATE / GATE+TRIG trigger modes are this same
        // choice by another name, so a 101 player expects it on the envelope's face.
        "retrigger" => Bound::new(
            id,
            &p.retrigger,
            "Whether overlapping notes restart the envelope or glide into it.",
        ),
        other => binding_for(other, p),
    }
}

/// A switch's cells, **labelled by the parameter itself**: each is its option's own formatted value,
/// so a cell reads what the host's automation list reads.
fn switch_options(params: &MxmMono01Params, id: &'static str) -> Vec<String> {
    let param = on_card(id, params).param;
    let last = param
        .steps()
        .unwrap_or_else(|| panic!("`{id}` is not drawn as a switch"));
    (0..=last)
        .map(|option| param.format(option as f32 / last as f32))
        .collect()
}

// --------------------------------------------------------------------------------------------
// The cards, as trees (plans/plan-layout-tree.md). Each card is described once — `card` — and that
// one description is both measured (its floor and its height) and drawn, leaf by leaf, through the
// bindings (`paint`). The gaps are the hand layout's own: a card body's `SPACE_3` rhythm between
// siblings, and whatever `add_space` it put on top of that, as a pad.
// --------------------------------------------------------------------------------------------

/// What a leaf of this editor's cards draws. Hashed by what it names, which is also what keeps its
/// widget ids stable when a route appears above it.
#[derive(Clone, Debug, Hash)]
pub enum Leaf {
    /// A knob drawn across the column it is given, centred in it.
    Knob(&'static str, Size),
    Slider(&'static str),
    Switch(&'static str),
    /// The LFO's shapes, on the rate knob's grid beside it.
    LfoShapes,
    /// A control's tempo sync, the quarter note beside it.
    Picture(&'static str),
    /// The sub-oscillator's shapes, each marked with its octave.
    SubShapes,
    /// Nothing: the room beside a disclosure's body that makes the disclosure span its card, so
    /// its separator runs across the card as the hand-drawn one did.
    Span,
    /// A target's route stack, by target.
    Routes(usize),
    MixerWaveform,
    FilterResponse,
    EnvelopeShape,
}

fn knob_kind(params: &MxmMono01Params, id: &'static str, size: Size, column: f32) -> Kind {
    let bound = on_card(id, params);
    let param = bound.param;
    // A syncable control's column holds its free readings and its divisions.
    let widest = if id == "lforate" {
        super::binding::synced_widest(param, crate::params::LFO_SYNC.span)
    } else {
        mxm_ui::control::widest_value(|n| param.format(n as f32))
    };
    Kind::Knob {
        name: bound.painted().to_owned(),
        widest,
        size,
        column,
    }
}

/// Knobs in **equal columns** across the whole card, each centred in its own — `ui.columns` as
/// data.
///
/// A knob's cell is as wide as the wider of its knob and its label, so a row of them spaced by its
/// own widths spaces itself by label width: "Filter envelope" pushes its neighbours along and the
/// knobs end up at uneven intervals under evenly-read labels. Equal columns with centred contents
/// is what makes a row of controls look like a row. It is the collection's knob row
/// (`mxm_ui::tree::knob_row`): equal columns capped at the collection's knob column, where they
/// used to stretch across the whole card.
fn knobs(ui: &Ui, params: &MxmMono01Params, knobs: &[(&'static str, Size)]) -> Node<Leaf> {
    mxm_ui::tree::knob_row(
        ui,
        knobs
            .iter()
            .map(|(id, size)| {
                (
                    *size,
                    leaf(Leaf::Knob(id, *size), knob_kind(params, id, *size, 0.0)),
                )
            })
            .collect(),
    )
}

/// Levels that want comparing as the collection's fader row (`tree::fader_row`): the mixer's four
/// sources, the envelope's A, D, S and R.
fn faders(ui: &Ui, params: &MxmMono01Params, ids: &[&'static str]) -> Node<Leaf> {
    mxm_ui::tree::fader_row(
        ui,
        ids.iter()
            .map(|&id| {
                let bound = on_card(id, params);
                mxm_ui::tree::fader(
                    Leaf::Slider(id),
                    fader_label(id).unwrap_or(bound.painted()),
                    mxm_ui::control::widest_value(|n| bound.param.format(n as f32)),
                )
            })
            .collect(),
    )
}

/// What an envelope fader paints: its stage's letter, by the convention (the owner, 2026-09-25).
/// A host, the tooltip and a screen reader read *Attack*.
fn fader_label(id: &str) -> Option<&'static str> {
    match id {
        "attack" => Some("A"),
        "decay" => Some("D"),
        "sustain" => Some("S"),
        "release" => Some("R"),
        _ => None,
    }
}

fn switch(params: &MxmMono01Params, id: &'static str) -> Node<Leaf> {
    leaf(
        Leaf::Switch(id),
        Kind::Segmented {
            label: on_card(id, params).painted().to_owned(),
            options: switch_options(params, id),
            beside: None,
        },
    )
}

/// `child` at its own width at the left, in a row that takes the whole width offered.
///
/// A disclosure is as wide as its body, and its header's separator is drawn across that width; the
/// bend range alone would leave a hairline as short as the switch under it, where `shell::disclosure`
/// ran it across the card.
fn spanning(child: Node<Leaf>) -> Node<Leaf> {
    row_gap(
        0.0,
        vec![
            child,
            leaf(
                Leaf::Span,
                Kind::Custom {
                    min_width: 0.0,
                    height: Height::Fixed(0.0),
                    fills: true,
                },
            ),
        ],
    )
}

/// One of §8's panels: it fills the card's width, and its height steps with that width
/// ([`visuals::panel_height`]). It has no minimum width of its own.
fn display(key: Leaf) -> Node<Leaf> {
    leaf(
        key,
        Kind::Custom {
            min_width: 0.0,
            height: Height::Of(visuals::panel_height),
            fills: true,
        },
    )
}

/// One target's routes, `SPACE_3` below what precedes it. A route stack is a composite with a rule
/// of its own — its floor is every route revealed — so it states its size (`stack_size`).
fn routes_leaf(ui: &Ui, params: &MxmMono01Params, target: usize) -> Node<Leaf> {
    let size = mxm_modulation_params::ui::stack_size(
        ui,
        mxm_mono_01_dsp::routing::TARGET_NAMES[target],
        &params.routes.each()[target].routes(target),
    );
    pad(
        SPACE_3,
        leaf(
            Leaf::Routes(target),
            Kind::Custom {
                min_width: size.x,
                height: Height::Fixed(size.y),
                fills: true,
            },
        ),
    )
}

/// One section's body, as a tree, from the parameters and the one editor fact that changes what a
/// card shows: whether the Oscillator's Advanced disclosure is open, read from egui's memory. The
/// card reserves it open, so opening it never grows the card; what is under it moves down.
pub fn card(ui: &Ui, section: Section, params: &MxmMono01Params) -> Node<Leaf> {
    use mxm_mono_01_dsp::routing::target;
    match section {
        // Tune and the two PWM controls are all original SH-101 controls, and hiding an original
        // control is what makes an instrument stop being recognisable, so they are on the card.
        // Only parameters the original did not have go in Advanced.
        Section::Oscillator => stack(vec![
            switch(params, "oscrange"),
            pad(
                SPACE_3,
                knobs(ui, params, &[("pulsewidth", STANDARD), ("glide", STANDARD)]),
            ),
            pad(
                SPACE_3,
                leaf(
                    Leaf::SubShapes,
                    Kind::Waves {
                        label: Some(on_card("subtype", params).param.name().to_owned()),
                        count: SUB_SHAPES.len(),
                        marks: SUB_MARKS.iter().map(|m| (*m).to_owned()).collect(),
                        beside: None,
                    },
                ),
            ),
            pad(SPACE_3, knobs(ui, params, &[("tune", COMPACT)])),
            // §5's labelled disclosure: an expander, never a hover-reveal. §4.3 requires essential
            // controls to stay reachable, and hidden-until-hover fails keyboard navigation outright
            // — there is no hover to give.
            tree::disclosure(
                ui.ctx(),
                ADVANCED,
                ADVANCED_DESCRIPTION,
                spanning(knobs(ui, params, &[("bendrange", COMPACT)])),
            ),
            routes_leaf(ui, params, target::PITCH),
            routes_leaf(ui, params, target::PULSE_WIDTH),
        ]),
        // Four sources, as sliders, because the point is comparing them: §7.1 prefers sliders
        // "where range and comparison matter more than compactness", and four levels read against
        // each other are one picture where four knobs are four readings. §3.3: visualization first
        // when it materially aids editing — four levels are a set of numbers until you see what
        // they add up to. Every slider is followed by `SPACE_2`, the last one too.
        Section::Mixer => stack(vec![
            display(Leaf::MixerWaveform),
            pad_all(
                SPACE_3,
                0.0,
                SPACE_3 + SPACE_2,
                faders(
                    ui,
                    params,
                    &["sawlevel", "pulselevel", "sublevel", "noiselevel"],
                ),
            ),
        ]),
        // The panel's centre of gravity, with §8's response curve — the brief calls it "the
        // clearest possible answer to what the filter is doing". Cutoff and resonance, the §2 pair;
        // the three modulation amounts that once shared their row are routes now, drawn by the
        // stack below: the envelope, the LFO and the key are three of the eleven sources that can
        // reach this cutoff rather than the only three.
        Section::Filter => stack(vec![
            display(Leaf::FilterResponse),
            pad(
                SPACE_3,
                knobs(ui, params, &[("cutoff", STANDARD), ("resonance", STANDARD)]),
            ),
            routes_leaf(ui, params, target::CUTOFF),
        ]),
        // **The LFO, the envelope and the amplifier the envelope drives, as one card** (the owner,
        // 2026-09-24): hugged, the LFO was one knob and a shape row and the Amplifier one
        // switch and a stack. What the LFO *is* comes first — its depths live with their
        // destinations, per §5 — then the one envelope, which the line says drives both the filter
        // and the amplifier, then what opens the amplifier and what modulates it. The card holds
        // three modules, so every name keeps its prefix. The output level after the amplifier is
        // not a module control: it is in the app bar beside the meter (design system §3.1 item 6).
        //
        // **Rate and shape are one question — how the LFO moves — so they sit on one row**, the
        // shapes on the rate knob's grid: label on its name line, shapes on its circle. The rate's
        // tempo sync is the quarter note beside it (`plans/plan-tempo-sync-controls.md`).
        Section::LfoAndEnvelope => stack(vec![
            row_gap(
                ui.spacing().item_spacing.x,
                vec![
                    knobs(ui, params, &[("lforate", STANDARD)]),
                    tree::switch_beside_knob(
                        STANDARD,
                        leaf(Leaf::Picture("lfosync"), Kind::SyncToggle),
                    ),
                    pad_all(
                        0.0,
                        SPACE_3,
                        0.0,
                        leaf(
                            Leaf::LfoShapes,
                            Kind::Waves {
                                label: Some(on_card("lfowave", params).painted().to_owned()),
                                count: LFO_SHAPES.len(),
                                marks: Vec::new(),
                                beside: Some(STANDARD),
                            },
                        ),
                    ),
                ],
            ),
            // The envelope reaching both the filter and the volume is the display's hover text,
            // not a line on the card (the owner, 2026-09-27: no help text on the panel).
            pad(SPACE_3, display(Leaf::EnvelopeShape)),
            pad(
                SPACE_3,
                faders(ui, params, &["attack", "decay", "sustain", "release"]),
            ),
            pad(SPACE_2, switch(params, "retrigger")),
            pad(SPACE_3, switch(params, "vcasource")),
            routes_leaf(ui, params, target::AMPLITUDE),
        ]),
    }
}

/// Everything a leaf draws with: the parameters and their host, the telemetry the displays read
/// (none of it destructive), and the text-entry buffers.
pub struct Live<'a, 'b> {
    pub params: &'a MxmMono01Params,
    pub telemetry: &'a Telemetry,
    pub setter: &'a ParamSetter<'b>,
    pub entries: &'a mut HashMap<&'static str, Option<String>>,
}

/// Draws one leaf, in the `Ui` the tree bounded to `rect`, through the bindings — so the controls,
/// their gestures and their names are exactly what they were.
pub fn paint(ui: &mut Ui, tokens: &Tokens, leaf: &Leaf, rect: Rect, live: &mut Live<'_, '_>) {
    let params = live.params;
    let setter = live.setter;
    match *leaf {
        // Inside its column this **is** the column's width, which is what stops a long name being
        // broken to fit a knob's diameter.
        // Synced to a tempo, the LFO rate reads its division; the host still reads its hertz.
        Leaf::Knob(id, size) => {
            let bound = on_card(id, params);
            let division = (id == "lforate" && params.lfo_sync.value())
                .then(|| {
                    use nice_plug::prelude::Param as _;
                    let rate = &params.lfo_rate;
                    crate::params::LFO_SYNC.shown(
                        rate.unmodulated_normalized_value(),
                        live.telemetry.tempo.get(),
                        f64::from(rate.preview_plain(0.0)),
                        f64::from(rate.preview_plain(1.0)),
                    )
                })
                .flatten();
            match division {
                Some(division) => bound.knob_with_reading(
                    ui,
                    tokens,
                    setter,
                    size,
                    rect.width(),
                    live.entries,
                    division.label(),
                ),
                None => bound.knob(ui, tokens, setter, size, rect.width(), live.entries),
            }
        }
        Leaf::Picture(id) => {
            super::binding::sync_picture(ui, tokens, id, on_card(id, params).param, setter);
        }
        Leaf::Slider(id) => {
            let bound = on_card(id, params);
            bound.slider_vertical(
                ui,
                tokens,
                setter,
                live.entries,
                fader_label(id).unwrap_or(bound.painted()),
                rect.width(),
                mxm_ui::control::FADER_HEIGHT,
            );
        }
        Leaf::Switch(id) => {
            let bound = on_card(id, params);
            let labels = switch_options(params, id);
            let options: Vec<&str> = labels.iter().map(String::as_str).collect();
            segmented_named(
                ui,
                tokens,
                id,
                bound.param,
                bound.panel.as_deref(),
                &options,
                bound.details,
                setter,
                0.0,
            );
        }
        Leaf::LfoShapes => {
            let bound = on_card("lfowave", params);
            segmented_waves_named(
                ui,
                tokens,
                bound.id,
                bound.param,
                bound.panel.as_deref(),
                LFO_SHAPES,
                // The knob it shares the row with: label on its name line, shapes on its circle.
                Some(STANDARD),
                bound.details,
                setter,
            );
        }
        Leaf::SubShapes => {
            let bound = on_card("subtype", params);
            segmented_waves_marked(
                ui,
                tokens,
                bound.id,
                bound.param,
                SUB_SHAPES,
                SUB_MARKS,
                bound.details,
                setter,
            );
        }
        Leaf::Routes(target) => routes(ui, tokens, target, params, setter, live.entries),
        Leaf::Span => {}
        Leaf::MixerWaveform => visuals::mixer_waveform(
            ui,
            tokens,
            params.saw_level.value(),
            params.pulse_level.value(),
            params.sub_level.value(),
            params.noise_level.value(),
            params.pulse_width.value(),
            // The plugin's own enum, converted at the boundary: the DSP's `SubShape` is what
            // describes the shape, and `From` already exists for it.
            params.sub_type.value().into(),
        ),
        Leaf::FilterResponse => visuals::filter_response(
            ui,
            tokens,
            params.cutoff.value(),
            params.resonance.value(),
            // The filter-envelope depth, which is a route now: the pair the machine itself wires
            // from the envelope into the cutoff. The curve shows where the envelope will take the
            // filter, so it reads the route rather than a knob that no longer exists.
            params.routes.cutoff.env.value(),
            live.telemetry.sample_rate(),
        ),
        Leaf::EnvelopeShape => {
            let (level, stage) = live.telemetry.envelope();
            visuals::envelope_shape(
                ui,
                tokens,
                params.attack.value(),
                params.decay.value(),
                params.sustain.value(),
                params.release.value(),
                level,
                stage,
            );
        }
    }
}

/// Draws one section's body: its tree, shown in `ui`.
///
/// The layout lab (`apps/mxm-layout-lab`, in the private archive since the split) draws these real
/// cards through this. `disclosed` is not read — the Advanced disclosure keeps its state in egui's
/// memory under [`mxm_ui::shell::disclosure_id`] — and stays in the signature so that caller is
/// unchanged.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut Ui,
    tokens: &Tokens,
    section: Section,
    params: &MxmMono01Params,
    telemetry: &Telemetry,
    setter: &ParamSetter<'_>,
    text_entry: &mut HashMap<&'static str, Option<String>>,
    _disclosed: &mut HashMap<&'static str, bool>,
) {
    let tree = card(ui, section, params);
    let mut live = Live {
        params,
        telemetry,
        setter,
        entries: text_entry,
    };
    tree::show(ui, tokens, &tree, |ui, leaf, rect| {
        paint(ui, tokens, leaf, rect, &mut live);
    });
}

/// One target's routes, drawn beneath the control they move.
///
/// **Routing belongs under the thing it affects**, never in a detached footer — the ruling
/// mxm-mono-00's `plugins/mxm-mono-00/AGENTS.md` records and design system §7.4 makes normative.
/// The rows and the `‹ modulate ›` menu come from `mxm_modulation_params`, so every editor in the
/// collection draws this the same way.
fn routes(
    ui: &mut Ui,
    tokens: &Tokens,
    target: usize,
    params: &MxmMono01Params,
    setter: &ParamSetter<'_>,
    text_entry: &mut HashMap<&'static str, Option<String>>,
) {
    let target_routes = params.routes.each()[target];
    let entry = text_entry.entry("routes").or_default();
    mxm_modulation_params::ui::stack(
        ui,
        tokens,
        mxm_mono_01_dsp::routing::TARGET_NAMES[target],
        // Nothing to drop: these names carry no module prefix the card already says.
        mxm_mono_01_dsp::routing::TARGET_NAMES[target],
        &target_routes.routes(target),
        entry,
        setter,
    );
}

// --------------------------------------------------------------------------------------------
// The brief's two tables, as data
// --------------------------------------------------------------------------------------------

/// Where a control sits: always visible on its section's card, behind §5's labelled disclosure,
/// or in the app bar.
///
/// **The rule is provenance, not frequency.** Everything the original SH-101 had on its panel is
/// on this panel — hiding an original control is what makes an instrument stop being recognisable,
/// which is the whole requirement the layout exists to meet. Advanced holds only what the original
/// did not have: additions that expand the classic architecture.
///
/// One qualifies today: `bendrange`, a MIDI-era value for what the original's bender did with its
/// own depth sliders. `retrigger` was disclosed on the same reasoning and reclassified by the
/// owner: the SH-101's GATE / GATE+TRIG trigger modes are the same choice by another name, so a
/// 101 player expects it on the envelope's face.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Surface {
    Primary,
    Disclosed,
    /// In the app bar beside the level meter, not on its section's card: design system §3.1 item
    /// 6 puts the master output there. Its section is still where it sits in the chain and in the
    /// `Parameters` list. `outgain` is the one.
    AppBar,
}

/// Brief §5 and §10, as one table: every parameter, its section, and its surface.
///
/// This is the single source the editor and its tests agree on. The drawing functions above are
/// written by hand for layout, so this table is what proves they did not drift — a control quietly
/// moved from Filter to Oscillator would still compile and still look reasonable.
pub const ASSIGNMENT: &[(&str, Section, Surface)] = &[
    // What the LFO *is*. Its depths live with their destinations.
    ("lforate", Section::LfoAndEnvelope, Surface::Primary),
    ("lfosync", Section::LfoAndEnvelope, Surface::Primary),
    ("lfowave", Section::LfoAndEnvelope, Surface::Primary),
    // Oscillator
    ("oscrange", Section::Oscillator, Surface::Primary),
    ("glide", Section::Oscillator, Surface::Primary),
    ("pulsewidth", Section::Oscillator, Surface::Primary),
    ("subtype", Section::Oscillator, Surface::Primary),
    ("tune", Section::Oscillator, Surface::Primary),
    ("bendrange", Section::Oscillator, Surface::Disclosed),
    // Mixer
    ("sawlevel", Section::Mixer, Surface::Primary),
    ("pulselevel", Section::Mixer, Surface::Primary),
    ("sublevel", Section::Mixer, Surface::Primary),
    ("noiselevel", Section::Mixer, Surface::Primary),
    // Filter
    ("cutoff", Section::Filter, Surface::Primary),
    ("resonance", Section::Filter, Surface::Primary),
    // The envelope, and the amplifier it drives. The output level ends the chain, and is drawn in
    // the app bar.
    ("attack", Section::LfoAndEnvelope, Surface::Primary),
    ("decay", Section::LfoAndEnvelope, Surface::Primary),
    ("sustain", Section::LfoAndEnvelope, Surface::Primary),
    ("release", Section::LfoAndEnvelope, Surface::Primary),
    ("retrigger", Section::LfoAndEnvelope, Surface::Primary),
    ("vcasource", Section::LfoAndEnvelope, Surface::Primary),
    ("outgain", Section::LfoAndEnvelope, Surface::AppBar),
];

/// Every parameter, in [`ASSIGNMENT`] order.
///
/// Init uses this rather than a list of its own, so "reset everything" cannot fall out of step with
/// the editor's own idea of what "everything" is — the totality test below covers both at once.
pub fn all_parameters(params: &MxmMono01Params) -> Vec<Bound<'_>> {
    ASSIGNMENT
        .iter()
        .map(|(id, _, _)| binding_for(id, params))
        .collect()
}

/// The bindings for one section, in the order the `Parameters` view lists them.
pub fn parameters_in<'a>(section: Section, params: &'a MxmMono01Params) -> Vec<Bound<'a>> {
    ASSIGNMENT
        .iter()
        .filter(|(_, s, _)| *s == section)
        .map(|(id, _, _)| binding_for(id, params))
        .collect()
}

/// Every id in [`ASSIGNMENT`] resolves here, and the test below proves the mapping is total.
pub fn binding_for<'a>(id: &'static str, p: &'a MxmMono01Params) -> Bound<'a> {
    match id {
        "lforate" => Bound::new(id, &p.lfo_rate, "How fast the LFO cycles."),
        "lfosync" => Bound::new(id, &p.lfo_sync, super::binding::SYNC_DESCRIPTION),
        "lfowave" => Bound::new(id, &p.lfo_wave, "The LFO's shape."),
        "oscrange" => Bound::new(id, &p.osc_range, "The oscillator's octave."),
        "glide" => Bound::new(id, &p.glide, "Portamento time between notes."),
        "pulsewidth" => Bound::new(id, &p.pulse_width, "How narrow the pulse wave is."),
        "subtype" => Bound::new(id, &p.sub_type, "The sub-oscillator's shape and octave."),
        "tune" => Bound::new(id, &p.tune, "Fine pitch trim.")
            .bipolar()
            .law(mxm_preset::StepLaw::Cents),
        "bendrange" => Bound::new(
            id,
            &p.bend_range,
            "Semitones per full pitch-bend deflection.",
        )
        .law(mxm_preset::StepLaw::Semitones),
        "sawlevel" => Bound::new(id, &p.saw_level, "Sawtooth level."),
        "pulselevel" => Bound::new(id, &p.pulse_level, "Pulse level."),
        "sublevel" => Bound::new(id, &p.sub_level, "Sub-oscillator level."),
        "noiselevel" => Bound::new(id, &p.noise_level, "Noise level."),
        "cutoff" => Bound::new(id, &p.cutoff, "Where the filter starts removing highs.")
            .law(mxm_preset::StepLaw::Hertz),
        "resonance" => Bound::new(id, &p.resonance, "Emphasis at the cutoff."),
        "outgain" => Bound::new(id, &p.output_gain, "The instrument's output level."),
        "vcasource" => Bound::new(id, &p.vca_source, "Envelope or gate shapes the volume."),
        "attack" => Bound::new(
            id,
            &p.attack,
            "Time to reach full level after a key is pressed.",
        ),
        "decay" => Bound::new(id, &p.decay, "Time to fall from full level to sustain."),
        "sustain" => Bound::new(id, &p.sustain, "The level held while a key stays down."),
        "release" => Bound::new(id, &p.release, "Time to fade after a key is let go."),
        "retrigger" => Bound::new(
            id,
            &p.retrigger,
            "Whether overlapping notes restart the envelope.",
        ),
        // Unreachable while the test below passes, and the test is what keeps it so.
        other => panic!("no binding for parameter id `{other}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::Params;
    use std::collections::HashSet;

    /// Every non-hidden parameter the plugin declares.
    fn declared() -> Vec<String> {
        MxmMono01Params::default()
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect()
    }

    #[test]
    fn every_parameter_is_assigned_exactly_once() {
        // The brief's M4b checklist, mechanically. Revision 1 of the plan counted this by hand and
        // got 26 of 27 - it missed `vcolfo`, because `lfo1.to_filter` is an alias of `filterlfo`
        // and made the role count look complete. Coverage is over ids for that reason.
        let declared: HashSet<String> = declared().into_iter().collect();
        let assigned: Vec<&str> = ASSIGNMENT.iter().map(|(id, _, _)| *id).collect();
        let unique: HashSet<&str> = assigned.iter().copied().collect();

        assert_eq!(
            assigned.len(),
            unique.len(),
            "a parameter is assigned to two places"
        );

        // **A routing parameter is assigned by its target's card, not by this table.** The pairs are
        // drawn by the shared stack beneath the control they move, so the card that owns a target
        // owns its routes — which is what the table's per-`Bound` shape cannot express. They are
        // still assigned exactly once, and `routing_parameters_belong_to_their_targets_card` below
        // is what says so.
        let missing: Vec<&String> = declared
            .iter()
            .filter(|id| !unique.contains(id.as_str()))
            .filter(|id| !id.starts_with("mod_"))
            .collect();
        assert!(missing.is_empty(), "not shown anywhere: {missing:?}");

        let invented: Vec<&&str> = unique
            .iter()
            .filter(|id| !declared.contains(**id))
            .collect();
        assert!(
            invented.is_empty(),
            "assigned but not declared: {invented:?}"
        );

        assert_eq!(
            declared.len(),
            22 + 4 * 11 * 2,
            "the instrument's parameter count changed: 22 of its own, plus one presence and one              amount for each of four targets by eleven sources. It was 27 until the five knobs the              machine wired its own modulation with became five of those pairs"
        );
    }

    /// Every routing pair belongs to the card that owns its target, and to exactly one.
    ///
    /// The companion to the exemption above: the pairs are not in `ASSIGNMENT` because they are
    /// drawn by the shared stack rather than by a `Bound`, so this is what keeps *assigned exactly
    /// once* true of them.
    #[test]
    fn routing_parameters_belong_to_their_targets_card() {
        let declared: Vec<String> = declared();
        let routing: Vec<&String> = declared
            .iter()
            .filter(|id| id.starts_with("mod_"))
            .collect();

        assert_eq!(
            routing.len(),
            4 * 11 * 2,
            "one presence and one amount per (target, source) pair"
        );

        // Each prefix is one target's, and each target is drawn on exactly one card.
        for (prefix, section) in [
            ("mod_pitch_", Section::Oscillator),
            ("mod_width_", Section::Oscillator),
            ("mod_cutoff_", Section::Filter),
            ("mod_amp_", Section::LfoAndEnvelope),
        ] {
            let n = routing.iter().filter(|id| id.starts_with(prefix)).count();
            assert_eq!(
                n,
                11 * 2,
                "{prefix} should carry eleven pairs, drawn on {section:?}"
            );
        }

        // And no routing id belongs to two prefixes, which is what "exactly once" means here.
        for id in &routing {
            let matched = ["mod_pitch_", "mod_width_", "mod_cutoff_", "mod_amp_"]
                .iter()
                .filter(|p| id.starts_with(**p))
                .count();
            assert_eq!(matched, 1, "{id} matches {matched} targets");
        }
    }

    #[test]
    fn the_surface_split_matches_the_brief() {
        // Brief §5: 25 primary, 1 disclosed — `retrigger` moved to primary by the owner's ruling —
        // and 1 in the app bar, the output level, which design system §3.1 puts there.
        let count = |surface: Surface| ASSIGNMENT.iter().filter(|(_, _, s)| *s == surface).count();
        let (primary, disclosed, app_bar) = (
            count(Surface::Primary),
            count(Surface::Disclosed),
            count(Surface::AppBar),
        );
        assert_eq!(primary + disclosed + app_bar, ASSIGNMENT.len());
        // Six of brief §5's 25 were the machine's own modulation depths and its PWM source switch;
        // they are routes now, drawn by the stack rather than assigned here.
        // The LFO sync (2026-09-25) is the one added since.
        assert_eq!(
            primary, 20,
            "brief §5, less the six that became routes, plus the LFO sync"
        );
        assert_eq!(disclosed, 1, "brief §5 says 1 disclosed");
        assert_eq!(app_bar, 1, "brief §5 puts the output level in the app bar");
    }

    #[test]
    fn only_controls_the_original_did_not_have_are_disclosed() {
        // The rule is provenance, not frequency: hiding a control the SH-101 had on its panel is
        // what makes the instrument stop being recognisable, which is the requirement the whole
        // layout exists to meet. This is that rule as an assertion rather than a convention.
        let disclosed: Vec<&str> = ASSIGNMENT
            .iter()
            .filter(|(_, _, s)| *s == Surface::Disclosed)
            .map(|(id, _, _)| *id)
            .collect();
        // `retrigger` is not in this list by the owner's ruling: the original's GATE / GATE+TRIG
        // trigger modes are the same choice by another name, so it counts as classic architecture.
        assert_eq!(
            disclosed,
            vec!["bendrange"],
            "only additions to the classic architecture belong in Advanced"
        );
    }

    #[test]
    fn modulation_depths_sit_with_their_destinations() {
        // A depth lives with what it modulates, not with the LFO. That used to be a statement about
        // three knobs; it is now a statement about **every** route, and
        // `routing_parameters_belong_to_their_targets_card` is where it is held — a pair is drawn by
        // the card that owns its target, which is the same rule with nothing left to forget.
        //
        // What survives here is the other half: the LFO's own controls are only what it *is*.
        let lfo: Vec<&str> = ASSIGNMENT
            .iter()
            .map(|(id, _, _)| *id)
            .filter(|id| id.starts_with("lfo"))
            .collect();
        assert_eq!(lfo, vec!["lforate", "lfosync", "lfowave"]);
    }

    #[test]
    fn every_assigned_id_has_a_binding() {
        // `binding_for` panics on an unknown id, which would be a panic inside a paint call. This
        // is what keeps that arm unreachable.
        let params = MxmMono01Params::default();
        for (id, _, _) in ASSIGNMENT {
            let bound = binding_for(id, &params);
            assert_eq!(bound.id, *id);
            assert!(
                !bound.description.is_empty(),
                "§7.1 requires a one-sentence description on every control; `{id}` has none"
            );
        }
    }

    #[test]
    fn user_visible_text_is_ascii_until_the_font_is_bundled() {
        // The defect this pins: the Envelope card once read "One envelope [] filter and
        // amplifier", because the arrow had no glyph in the fallback font.
        //
        // Design system §6 requires Inter Variable bundled for release and permits a platform
        // fallback during development. While the fallback is in use, what it can draw is not
        // knowable from here - so the rule is the conservative one, and it lifts by itself the day
        // the font lands.
        if mxm_ui::typography::FONT_IS_BUNDLED {
            return;
        }

        let params = MxmMono01Params::default();
        for (id, section, _) in ASSIGNMENT {
            let bound = binding_for(id, &params);
            assert!(
                bound.description.is_ascii(),
                "`{id}`'s description has a character the fallback font may not have: {:?}",
                bound.description
            );
            assert!(
                section.title().is_ascii(),
                "section title {:?} is not ASCII",
                section.title()
            );
            assert!(
                bound.param.name().is_ascii(),
                "`{id}`'s label is not ASCII: {:?}",
                bound.param.name()
            );
        }
    }

    #[test]
    fn the_sections_partition_the_instrument() {
        // Every section's card draws at least one control. An empty card is a §3.2 "nearly empty
        // page" at card scale, and it would mean the sequence promises something the panel does
        // not show. A control drawn in the app bar is not on its section's card, so it does not
        // count.
        for section in [
            Section::LfoAndEnvelope,
            Section::Oscillator,
            Section::Mixer,
            Section::Filter,
        ] {
            assert!(
                ASSIGNMENT
                    .iter()
                    .any(|(_, s, surface)| *s == section && *surface != Surface::AppBar),
                "{section:?} would render as an empty card"
            );
        }
    }
}
