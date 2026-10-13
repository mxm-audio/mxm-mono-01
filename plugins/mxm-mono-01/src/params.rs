//! Parameter definitions.
//!
//! Every `#[id]` here is **permanent**. Changing one breaks every saved project
//! that used the plugin, so ids are treated as part of the public interface.
//!
//! Two conventions worth knowing before editing this file:
//!
//! * **Gain is stored as linear gain, never dB.** `SmoothingStyle::Logarithmic`
//!   over a dB range spanning zero is mathematically invalid and trips a debug
//!   assertion. dB is a display format only.
//! * **Values that are coefficients rather than signals are not smoothed.** The
//!   envelope times and the glide time set state-machine behaviour; smoothing them
//!   would make the timing impossible to reason about.

use mxm_mono_01_dsp::lfo::LfoShape;
use mxm_mono_01_dsp::oscillator::SubShape;
use mxm_mono_01_dsp::voice::{Retrigger, VcaSource};
use mxm_params::text;
use nice_plug::prelude::*;
use std::sync::RwLock;

/// Oscillator footage. The hardware's range switch, in organ-stop notation.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OscRange {
    #[id = "16"]
    #[name = "16'"]
    Sixteen,
    #[id = "8"]
    #[name = "8'"]
    Eight,
    #[id = "4"]
    #[name = "4'"]
    Four,
    #[id = "2"]
    #[name = "2'"]
    Two,
}

impl OscRange {
    /// Semitone offset relative to 8'.
    pub fn semitones(self) -> f32 {
        match self {
            OscRange::Sixteen => -12.0,
            OscRange::Eight => 0.0,
            OscRange::Four => 12.0,
            OscRange::Two => 24.0,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubKind {
    #[id = "oct1-square"]
    #[name = "1 oct square"]
    Oct1Square,
    #[id = "oct2-square"]
    #[name = "2 oct square"]
    Oct2Square,
    #[id = "oct2-pulse"]
    #[name = "2 oct pulse"]
    Oct2Pulse,
}

impl From<SubKind> for SubShape {
    fn from(k: SubKind) -> Self {
        match k {
            SubKind::Oct1Square => SubShape::Oct1Square,
            SubKind::Oct2Square => SubShape::Oct2Square,
            SubKind::Oct2Pulse => SubShape::Oct2Pulse,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoWave {
    #[id = "triangle"]
    #[name = "Triangle"]
    Triangle,
    #[id = "square"]
    #[name = "Square"]
    Square,
    #[id = "saw-up"]
    #[name = "Ramp up"]
    SawUp,
    #[id = "saw-down"]
    #[name = "Ramp down"]
    SawDown,
    #[id = "random"]
    #[name = "Random"]
    Random,
}

impl From<LfoWave> for LfoShape {
    fn from(w: LfoWave) -> Self {
        match w {
            LfoWave::Triangle => LfoShape::Triangle,
            LfoWave::Square => LfoShape::Square,
            LfoWave::SawUp => LfoShape::SawUp,
            LfoWave::SawDown => LfoShape::SawDown,
            LfoWave::Random => LfoShape::Random,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcaKind {
    #[id = "envelope"]
    #[name = "Envelope"]
    Envelope,
    #[id = "gate"]
    #[name = "Gate"]
    Gate,
}

impl From<VcaKind> for VcaSource {
    fn from(k: VcaKind) -> Self {
        match k {
            VcaKind::Envelope => VcaSource::Envelope,
            VcaKind::Gate => VcaSource::Gate,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetriggerKind {
    #[id = "legato"]
    #[name = "Legato"]
    Legato,
    #[id = "retrigger"]
    #[name = "Retrigger"]
    Always,
}

impl From<RetriggerKind> for Retrigger {
    fn from(k: RetriggerKind) -> Self {
        match k {
            RetriggerKind::Legato => Retrigger::Legato,
            RetriggerKind::Always => Retrigger::Always,
        }
    }
}

/// **The LFO rate's tempo sync** (`plans/plan-tempo-sync-controls.md`): every LFO's ladder, 1/32 to
/// four bars, the top the fastest.
pub const LFO_SYNC: mxm_tempo::Ladder =
    mxm_tempo::Ladder::new(mxm_tempo::Span::LFO, mxm_tempo::Direction::Rate);

#[derive(Params)]
pub struct MxmMono01Params {
    // ---- Oscillator ----
    #[id = "oscrange"]
    pub osc_range: EnumParam<OscRange>,
    #[id = "tune"]
    pub tune: FloatParam,
    #[id = "glide"]
    pub glide: FloatParam,
    #[id = "bendrange"]
    pub bend_range: FloatParam,

    // ---- Mixer ----
    #[id = "sawlevel"]
    pub saw_level: FloatParam,
    #[id = "pulselevel"]
    pub pulse_level: FloatParam,
    #[id = "sublevel"]
    pub sub_level: FloatParam,
    #[id = "noiselevel"]
    pub noise_level: FloatParam,

    // ---- Pulse width ----
    #[id = "pulsewidth"]
    pub pulse_width: FloatParam,
    #[id = "subtype"]
    pub sub_type: EnumParam<SubKind>,

    // ---- Filter ----
    #[id = "cutoff"]
    pub cutoff: FloatParam,
    #[id = "resonance"]
    pub resonance: FloatParam,

    // ---- Envelope ----
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "decay"]
    pub decay: FloatParam,
    #[id = "sustain"]
    pub sustain: FloatParam,
    #[id = "release"]
    pub release: FloatParam,

    // ---- LFO ----
    #[id = "lforate"]
    pub lfo_rate: FloatParam,
    /// The LFO rate's tempo sync: its position picks a division of the host's tempo.
    #[id = "lfosync"]
    pub lfo_sync: BoolParam,
    #[id = "lfowave"]
    pub lfo_wave: EnumParam<LfoWave>,

    // ---- Global ----
    #[id = "vcasource"]
    pub vca_source: EnumParam<VcaKind>,
    #[id = "retrigger"]
    pub retrigger: EnumParam<RetriggerKind>,
    #[id = "outgain"]
    pub output_gain: FloatParam,

    /// Which preset is loaded, and what it looked like when it was.
    ///
    /// **Persisted with the patch, not beside it.** nice-plug carries non-parameter state through
    /// the `Params` derive's `#[persist]`, so it belongs here rather than as a field on the plugin
    /// struct — and it has its own version number, because nice-plug's state version is
    /// `Plugin::VERSION`, which moves for unrelated reasons.
    // ---- Modulation routing ----
    /// One presence and one amount per *(target, source)* pair. Declared last so every existing
    /// parameter keeps its position in the preset file's declaration order.
    #[nested(group = "Modulation")]
    pub routes: crate::routes::Routes,

    #[persist = "preset"]
    pub preset: RwLock<mxm_preset::PresetIdentity>,
}

/// A `0..=1` control displayed as a percentage.
fn percent(name: &str, default: f32, smoothing_ms: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(smoothing_ms))
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

/// An envelope time. Skewed so the short end, where the useful resolution is, gets
/// most of the travel.
fn time_param(name: &str, default: f32, max: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: 0.001,
            max,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_value_to_string(text::milliseconds_then_seconds())
    .with_string_to_value(text::parse_time())
}

impl Default for MxmMono01Params {
    fn default() -> Self {
        Self {
            osc_range: EnumParam::new("Range", OscRange::Eight),
            tune: FloatParam::new(
                "Tune",
                0.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 100.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" cents")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            glide: FloatParam::new(
                "Glide",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 2.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            // Not smoothed: this is a time constant, not a signal.
            .with_value_to_string(text::milliseconds_then_seconds())
            .with_string_to_value(text::parse_time()),

            bend_range: FloatParam::new(
                "Bend range",
                2.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 12.0,
                },
            )
            .with_step_size(1.0)
            // Smoothed because it scales a continuous signal: stepping it while a
            // bend is held would click.
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" st"),

            saw_level: percent("Saw", 0.8, 10.0),
            pulse_level: percent("Pulse", 0.0, 10.0),
            sub_level: percent("Sub", 0.0, 10.0),
            noise_level: percent("Noise", 0.0, 10.0),

            pulse_width: FloatParam::new(
                "Pulse width",
                0.5,
                FloatRange::Linear {
                    min: 0.05,
                    max: 0.95,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            sub_type: EnumParam::new("Sub shape", SubKind::Oct1Square),

            cutoff: FloatParam::new(
                "Cutoff",
                // Init: effectively open. Not the 20 kHz range end — that is above the audio band
                // at 44.1 kHz, so the top tenth of the control would do nothing audible.
                16_000.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20_000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(15.0))
            .with_value_to_string(text::tenths_of_hertz_then_kilohertz())
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            resonance: percent("Resonance", 0.0, 15.0),
            // Init: a struck-string shape. Fast attack, long decay, low but non-zero sustain —
            // a real piano decays to silence while held, which reads as a broken instrument on the
            // first long note somebody plays after pressing Init.
            attack: time_param("Attack", 0.002, 8.0),
            decay: time_param("Decay", 1.2, 12.0),
            sustain: percent("Sustain", 0.2, 20.0),
            release: time_param("Release", 0.25, 12.0),

            lfo_rate: FloatParam::new(
                "LFO rate",
                // Init: a latent configuration. Every LFO depth is zero, so this is inaudible until
                // one is raised — and 5.5 Hz is what makes that moment *vibrato* rather than a
                // 0.05 Hz drift or a 30 Hz buzz.
                5.5,
                FloatRange::Skewed {
                    min: 0.05,
                    max: 30.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            lfo_sync: BoolParam::new("LFO sync", false),
            lfo_wave: EnumParam::new("LFO shape", LfoWave::Triangle),

            vca_source: EnumParam::new("VCA source", VcaKind::Envelope),
            routes: crate::routes::Routes::new(),
            preset: RwLock::new(mxm_preset::PresetIdentity::none()),

            retrigger: EnumParam::new("Note priority", RetriggerKind::Legato),

            output_gain: FloatParam::new(
                "Output",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-60.0),
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-60.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(15.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
        }
    }
}

impl MxmMono01Params {
    /// The LFO rate while its sync follows the host, or `None` for its free rate: the modulated
    /// position picks a division on [`LFO_SYNC`]. Resolved once a buffer by the plugin.
    pub fn synced_lfo_rate(&self, tempo: Option<f64>) -> Option<f32> {
        let rate = &self.lfo_rate;
        LFO_SYNC
            .resolve(
                self.lfo_sync.value(),
                tempo,
                rate.modulated_normalized_value(),
                f64::from(rate.preview_plain(0.0)),
                f64::from(rate.preview_plain(1.0)),
            )
            .map(|hz| hz as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::Param;

    /// The init-patch rule from `plugins/AGENTS.md`, as a test that survives a retune.
    ///
    /// **Amounts only.** `resonance` is a filter amount and the oscillator levels are a mix, not
    /// modulation depths — they are pinned by [`the_init_patch_is_the_agreed_table`] instead. Keeping
    /// the two apart is deliberate: this one must stay true for every future instrument's defaults,
    /// so it may only assert the part of the rule that is universal.
    ///
    /// **Every modulation depth on this instrument is now a route amount**, since the five knobs
    /// that used to be here — `filterenv`, `filterlfo`, `vcolfo`, `pwmdepth`, `keytrack` — became
    /// the machine's own routes in the init patch. So the rule is checked where the depths live,
    /// across all forty-four pairs rather than the five that had knobs.
    #[test]
    fn every_modulation_depth_starts_at_zero() {
        let p = MxmMono01Params::default();
        for (t, name) in mxm_mono_01_dsp::routing::TARGET_NAMES.iter().enumerate() {
            let target = p.routes.each()[t];
            for (route, source) in target
                .routes(t)
                .iter()
                .zip(mxm_mono_01_dsp::routing::SOURCE_NAMES)
            {
                assert_eq!(
                    route.amount.default_normalised(),
                    0.5,
                    "`{name} from {source}` is a modulation depth and must start at zero: after                      Init, turning one control has to produce the change that control is named                      after and no other. A signed amount is stored normalised, so zero is 0.5"
                );
            }
        }
    }

    /// **The init patch wires the machine's own five routes and nothing else**, at zero depth.
    ///
    /// The owner's requirement from the first sentence of the request: *the sources in the init
    /// patch should be the ones that follow the original signal flow*. Presence is configuration, so
    /// it starts where the machine itself wired it; the depth is an amount, so it starts at zero.
    #[test]
    fn the_init_patch_presets_exactly_the_machines_own_routes() {
        let p = MxmMono01Params::default();
        assert_eq!(
            p.routes.topology().present,
            mxm_mono_01_dsp::routing::Routing::init().present,
            "the parameter defaults and the DSP's own init wiring have drifted apart"
        );
    }

    /// Glide is a **time** and still an amount — the case the rule's wording exists to settle.
    #[test]
    fn glide_starts_at_zero_because_it_is_an_amount_not_a_configuration() {
        assert_eq!(MxmMono01Params::default().glide.default_plain_value(), 0.0);
    }

    /// Configurations start somewhere musical, so raising their amount lands on something chosen.
    ///
    /// The LFO is the worked example: at depth zero its rate is inaudible, so the only thing that
    /// makes raising the depth *vibrato* rather than a drift or a buzz is where the rate already is.
    /// **The LFO sync picks a division and is inert without a tempo**
    /// (`plans/plan-tempo-sync-controls.md`): off, or with no tempo, the knob's own hertz stand; on
    /// at 120 bpm the ends are the LFO ladder's ends, the top the fastest.
    #[test]
    fn lfo_sync_picks_a_division_and_is_inert_without_a_tempo() {
        use mxm_tempo::Division;
        use nice_plug::params::InternalParamMut;
        fn set<P: InternalParamMut>(param: &P, normalized: f32) {
            unsafe {
                let _ = param._internal_set_normalized_value(normalized);
            }
        }
        let p = MxmMono01Params::default();
        set(&p.lfo_rate, 1.0);
        assert_eq!(p.synced_lfo_rate(Some(120.0)), None, "off is the free rate");
        set(&p.lfo_sync, 1.0);
        assert_eq!(p.synced_lfo_rate(None), None, "no tempo is the free rate");

        let top = p.synced_lfo_rate(Some(120.0)).expect("synced at a tempo");
        set(&p.lfo_rate, 0.0);
        let bottom = p.synced_lfo_rate(Some(120.0)).expect("synced at a tempo");
        let (lo, hi) = (
            f64::from(p.lfo_rate.preview_plain(0.0)),
            f64::from(p.lfo_rate.preview_plain(1.0)),
        );
        assert!(
            top > bottom,
            "the top of a rate is the fastest: {bottom} to {top}"
        );
        let fastest = Division::ThirtySecond.hz(120.0).clamp(lo, hi) as f32;
        let slowest = Division::FourBars.hz(120.0).clamp(lo, hi) as f32;
        assert!((top - fastest).abs() < 1e-4, "{top} against {fastest}");
        assert!(
            (bottom - slowest).abs() < 1e-4,
            "{bottom} against {slowest}"
        );
    }

    #[test]
    fn a_latent_configuration_is_somewhere_useful_before_its_amount_is_raised() {
        let p = MxmMono01Params::default();

        let rate = p.lfo_rate.default_plain_value();
        assert!(
            (4.0..=7.0).contains(&rate),
            "LFO rate {rate} Hz is not a natural vibrato speed"
        );

        let width = p.pulse_width.default_plain_value();
        assert!(
            (0.45..=0.55).contains(&width),
            "pulse width {width} should be a square, so raising the pulse level is unsurprising"
        );
    }

    /// **Every parameter's text survives the host's round trip**, with the unit on, across the
    /// collection's grids and switch points (`mxm_plugin_test::text_checks`).
    #[test]
    fn every_parameter_text_survives_the_hosts_round_trip() {
        mxm_plugin_test::text_checks::every_text_round_trips(&MxmMono01Params::default(), &[]);
    }
}
