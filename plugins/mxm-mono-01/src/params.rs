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
use nice_plug::prelude::*;
use std::sync::{Arc, RwLock};

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

/// Formats a parameter value for display.
type ValueToString = Arc<dyn Fn(f32) -> String + Send + Sync>;
/// Parses a typed-in value, returning `None` if it cannot be understood.
type StringToValue = Arc<dyn Fn(&str) -> Option<f32> + Send + Sync>;

/// Format seconds as milliseconds below a second, seconds above.
///
/// **The unit is chosen from what the millisecond text would round to, not from the raw value.**
/// A host parses the text and normalises it before printing it again, so the value it prints lands a
/// hair either side of where it started: switching at the raw second printed `0.9996 s` as
/// `1000 ms`, which parses to one second and prints `1.00 s`, and `clap-validator`'s
/// `param-conversions` fails whenever its values land there. Anything that would print `1000 ms`
/// prints seconds instead.
fn v2s_time() -> ValueToString {
    Arc::new(|s| {
        if (s * 1_000.0).round() >= 1_000.0 {
            format!("{s:.2} s")
        } else {
            format!("{:.0} ms", s * 1000.0)
        }
    })
}

/// Hertz to a tenth below a kilohertz, kilohertz to a tenth above — nice-plug's
/// `v2s_f32_hz_then_khz(1)` with **the unit chosen from what the hertz text would round to**.
///
/// That formatter switches at the raw 1000 Hz, so `999.96 Hz` printed `1000.0 Hz`, which parses to
/// 1000 and — normalised and back — prints `1.0 kHz`, and a `1.0 kHz` could come back as
/// `1000.0 Hz`. `mxm-mono-pr1` met it on its cutoff first. The parser is still nice-plug's.
fn v2s_hertz() -> ValueToString {
    Arc::new(|hz| {
        if (hz * 10.0).round() >= 10_000.0 {
            format!("{:.1} kHz", hz / 1_000.0)
        } else {
            format!("{hz:.1} Hz")
        }
    })
}

fn s2v_time() -> StringToValue {
    Arc::new(|text| {
        let t = text.trim().to_lowercase();
        let (number, scale) = if let Some(rest) = t.strip_suffix("ms") {
            (rest, 0.001)
        } else if let Some(rest) = t.strip_suffix('s') {
            (rest, 1.0)
        } else {
            (t.as_str(), 0.001)
        };
        number.trim().parse::<f32>().ok().map(|v| v * scale)
    })
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
    .with_value_to_string(v2s_time())
    .with_string_to_value(s2v_time())
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
            .with_value_to_string(v2s_time())
            .with_string_to_value(s2v_time()),

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
            .with_value_to_string(v2s_hertz())
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

    /// **Every parameter reads the same after the host's own round trip**: printed with its unit,
    /// parsed, and printed again, it is the same text (mxm-kit's `docs/code-review-notes.md` §6).
    ///
    /// The host never hands a formatter a plain value. The CLAP wrapper's `value_to_text` and
    /// `text_to_value` carry a normalised value in `f64`, scaled by the step count, so a parsed number
    /// goes through the range's normalisation and back before it is printed again — and a formatter
    /// that picks its unit or its precision from the *raw* value flips branch when that trip lands a
    /// hair the other side of the switch: `0.9996 s` printed `1000 ms`, which parses to one second
    /// and prints `1.00 s`. `clap-validator`'s `param-conversions` fails only when its values land in
    /// that sliver, so one clean run proves nothing.
    ///
    /// So this walks the whole parameter map, as the wrapper converts, at the validator's grids, at
    /// plain values either side of every branch point this file's and the routes' formatters have — a
    /// second and a kilohertz, zero where a range crosses it, 0 dB — and at every representable
    /// normalised value near each of those points.
    #[test]
    fn every_parameter_reads_the_same_after_the_hosts_round_trip() {
        let params = MxmMono01Params::default();
        let map = params.param_map();
        // `clap-validator` 0.4.1 spends 4000 conversions across the parameters, 5 to 100 each.
        let installed = 4000usize.div_ceil(map.len()).clamp(5, 100);

        let mut probes: Vec<f32> = vec![
            // Seconds, printed `{:.0} ms` below one second: the millisecond text reaches `1000` at
            // 0.9995 s.
            0.9994, 0.999_49, 0.9995, 0.999_51, 0.9996, 0.9999, 1.0, 1.000_01, 1.004, 1.005, 1.006,
            // Hertz, printed `{:.1} Hz` below one kilohertz: the text reaches `1000.0` at 999.95 Hz.
            999.4, 999.46, 999.49, 999.5, 999.9, 999.94, 999.95, 999.96, 1_000.0, 1_000.04, 1_000.1,
            1_049.9, 1_050.0, 1_050.1,
        ];
        // Either side of zero, where a plain `{:.N}` prints a negative zero.
        probes.push(0.0);
        for decade in [1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1] {
            for multiple in [1.0, 4.0, 5.0, 6.0] {
                probes.extend([decade * multiple, -decade * multiple]);
            }
        }
        // Either side of 0 dB, for a gain shown in decibels.
        for db in [1e-4, 1e-3, 0.04, 0.05, 0.06] {
            probes.extend([util::db_to_gain(db), util::db_to_gain(-db)]);
        }
        // The branch points themselves, where every nearby normalised value is tried too.
        let edges = [0.0, 0.9995, 1.0, 999.95, 1_000.0];

        let mut failures = Vec::new();
        for (id, param, _) in &map {
            // SAFETY: `params` owns every parameter these pointers name and outlives the loop; this
            // is the access the wrapper makes.
            unsafe {
                let steps = param.step_count();
                let scale = steps.unwrap_or(1) as f64;
                let mut values: Vec<f64> = (0..=19)
                    .map(|i| scale * f64::from(i) / 19.0)
                    .chain((0..installed).map(|i| scale * i as f64 / (installed - 1) as f64))
                    .collect();
                if steps.is_none() {
                    let (low, high) = (param.preview_plain(0.0), param.preview_plain(1.0));
                    values.extend(
                        probes
                            .iter()
                            .map(|&plain| f64::from(param.preview_normalized(plain))),
                    );
                    for &edge in edges.iter().filter(|&&edge| low <= edge && edge <= high) {
                        let mut up = param.preview_normalized(edge);
                        let mut down = up;
                        for _ in 0..=64 {
                            values.extend([f64::from(up), f64::from(down)]);
                            up = up.next_up().min(1.0);
                            down = down.next_down().max(0.0);
                        }
                    }
                }
                // `ext_params_value_to_text` and `ext_params_text_to_value`, as the wrapper has them.
                let text_of = |value: f64| {
                    param.normalized_value_to_string(value as f32 / scale as f32, true)
                };
                for value in values {
                    let first = text_of(value);
                    let Some(back) = param.string_to_normalized_value(&first) else {
                        failures.push(format!("{id}: {first:?} does not parse"));
                        continue;
                    };
                    let second = text_of(f64::from(back) * scale);
                    if second != first {
                        failures.push(format!("{id}: {first:?} parses and reads {second:?}"));
                    }
                }
            }
        }
        failures.sort();
        failures.dedup();
        assert!(
            failures.is_empty(),
            "{} texts changed through the host's conversion:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
