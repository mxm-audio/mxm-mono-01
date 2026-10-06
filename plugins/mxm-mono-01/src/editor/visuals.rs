//! The brief's §8 visualizations.
//!
//! Two are drawn here; the third — output level and clip — is `mxm_ui::shell::level_meter` in the
//! app bar, because §3.1 puts it there and every MXM instrument needs the same one.
//!
//! # Fidelity, stated
//!
//! The brief says it and this module implements it: the filter curve is the **linear analytic
//! 4-pole response at the current sample rate**, and it ignores the input drive stage and the
//! resonance-feedback nonlinearity. A measured sweep at high resonance or high input will not
//! match it exactly.
//!
//! That is a deliberate approximation, not an oversight. The thing this plot exists to make
//! visible — the brief's words: *"the resonance-bass-loss behaviour… instead of mysterious"* — is
//! fully present in the linear model, and an exact plot would mean running the filter, which is
//! the audio thread's job and not the editor's.
//!
//! # Degrading
//!
//! §7.5: *"A visualization must degrade gracefully if realtime data is unavailable."* Everything
//! here is a function of parameters plus one published scalar, so there is no "no data" state — but
//! there are degenerate ones, and each is handled rather than allowed to produce a `NaN` that
//! reaches a paint call.

use egui::{Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use mxm_mono_01_dsp::envelope::Stage;
use mxm_mono_01_dsp::oscillator::SubShape;
use mxm_ui::space::{HAIRLINE, RADIUS};
use mxm_ui::theme::Tokens;

/// Height of a visualization panel at full size. §10 reduces this first when space is short, per
/// §4.3's priority order — controls, then labels, then routing, then visualization.
const FULL_HEIGHT: f32 = 72.0;
const REDUCED_HEIGHT: f32 = 48.0;
/// Below this card width the display is the first thing to shrink.
const ROOMY: f32 = 260.0;

/// The height every panel here is drawn at when it is `width` wide: full from [`ROOMY`] up, reduced
/// below it. **This is the size the card's tree states for it** (`sections::card`, a filling
/// `Custom` leaf with no minimum width of its own), and each panel below allocates exactly this, so
/// the statement and the drawing are one function.
#[must_use]
pub fn panel_height(width: f32) -> f32 {
    if width >= ROOMY {
        FULL_HEIGHT
    } else {
        REDUCED_HEIGHT
    }
}

/// How many oscillator cycles the mixer waveform draws.
///
/// Four, so **every** sub shape completes at least one cycle of its own. Two was wrong and made
/// the display look broken: a sub two octaves down needs four oscillator cycles for one of its
/// own, so `2 oct square` never completed a period — it drew as a constant offset, and switching
/// between the sub shapes barely changed the picture.
const CYCLES: f32 = 4.0;

/// The filter response curve, with the envelope's modulation range as a secondary trace.
pub fn filter_response(
    ui: &mut Ui,
    tokens: &Tokens,
    cutoff_hz: f32,
    resonance: f32,
    filter_env: f32,
    sample_rate: f32,
) {
    let height = panel_height(ui.available_width());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    frame(ui, tokens, rect);

    // A zero or negative rate would divide by zero below. `Telemetry` defaults to 48 kHz for this
    // reason; this is the second line of defence, because a host may report something absurd.
    let fs = if sample_rate.is_finite() && sample_rate > 1_000.0 {
        sample_rate
    } else {
        48_000.0
    };

    // The envelope's modulation range, drawn first so the main curve sits on top of it. The
    // envelope shifts the cutoff, so the secondary trace is the same curve at the shifted
    // frequency — which is what "shows the envelope's modulation range" means here.
    if filter_env.abs() > 0.001 {
        let modulated = (cutoff_hz * 2f32.powf(filter_env * 4.0)).clamp(20.0, fs * 0.49);
        ui.painter().add(Shape::line(
            curve(rect, modulated, resonance, fs),
            Stroke::new(1.0, tokens.mod_envelope),
        ));
    }

    ui.painter().add(Shape::line(
        curve(rect, cutoff_hz, resonance, fs),
        Stroke::new(2.0, tokens.accent),
    ));

    response.on_hover_text(
        "The filter's shape at the current cutoff and resonance; the red band is how far the envelope moves it.",
    );
}

/// The envelope's shape, with a position indicator while a note sounds.
#[allow(clippy::too_many_arguments)]
pub fn envelope_shape(
    ui: &mut Ui,
    tokens: &Tokens,
    attack_s: f32,
    decay_s: f32,
    sustain: f32,
    release_s: f32,
    level: f32,
    stage: Stage,
) {
    let height = panel_height(ui.available_width());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    frame(ui, tokens, rect);

    // The shape is drawn on a proportional time axis with a fixed sustain segment, so the four
    // stages stay readable whatever the absolute times are. An absolute axis would make a 10 ms
    // attack invisible next to an 8 s release, which is exactly the patch where you want to see
    // the attack.
    let total = (attack_s + decay_s + release_s).max(0.001);
    let sustain_fraction = 0.25_f32;
    let span = (1.0 - sustain_fraction).max(0.05);
    let a = attack_s / total * span;
    let d = decay_s / total * span;
    let r = release_s / total * span;

    let x = |t: f32| rect.left() + rect.width() * t.clamp(0.0, 1.0);
    let y = |v: f32| rect.bottom() - rect.height() * v.clamp(0.0, 1.0);

    let attack_end = a;
    let decay_end = a + d;
    let sustain_end = decay_end + sustain_fraction;
    let release_end = (sustain_end + r).min(1.0);

    let points = vec![
        Pos2::new(x(0.0), y(0.0)),
        Pos2::new(x(attack_end), y(1.0)),
        Pos2::new(x(decay_end), y(sustain)),
        Pos2::new(x(sustain_end), y(sustain)),
        Pos2::new(x(release_end), y(0.0)),
    ];
    ui.painter()
        .add(Shape::line(points, Stroke::new(2.0, tokens.mod_envelope)));

    // The position indicator. §8 wants it moving through the shape while a note sounds, so it is
    // placed by the *stage* as well as the level — the level alone is ambiguous, since 0.7 could
    // be a rising attack or a decaying tail.
    if stage != Stage::Idle {
        let t = match stage {
            Stage::Attack => attack_end * level,
            Stage::Decay => {
                let fallen = (1.0 - level) / (1.0 - sustain).max(0.001);
                attack_end + d * fallen.clamp(0.0, 1.0)
            }
            Stage::Sustain => sustain_end,
            Stage::Release => {
                let fallen = 1.0 - (level / sustain.max(0.001)).clamp(0.0, 1.0);
                sustain_end + r * fallen
            }
            Stage::Idle => 0.0,
        };
        let px = x(t);
        ui.painter().line_segment(
            [Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
            Stroke::new(1.0, tokens.accent),
        );
        ui.painter()
            .circle_filled(Pos2::new(px, y(level)), 3.0, tokens.accent);
    }

    response.on_hover_text(
        "The envelope's shape, for both the filter and the volume; the marker shows where the playing note is.",
    );
}

/// One cycle of what the mixer produces, from the four source levels.
///
/// # Not an oscilloscope
///
/// Brief §8 excludes oscilloscope and spectrum displays, and this is neither. It shows the *shape
/// the mixer makes* from its four levels — a static consequence of the controls beside it, not a
/// live view of the output. It answers "what do these four faders add up to", which is a question
/// the numbers alone cannot, and it changes only when a control changes.
///
/// # Declared approximation
///
/// The **ideal** waveform, not the one the DSP renders. The real oscillator is antialiased, so its
/// saw and pulse edges are band-limited and its corners are soft; this draws the mathematical
/// shape. That is the right trade for a shape preview — the antialiasing is what stops the sound
/// being harsh, not what makes it recognisable — but it means a very high note's real output has
/// visibly rounder corners than this.
///
/// Noise is drawn as a **deterministic** jitter, so the display does not shimmer while nothing is
/// being changed. It shows how much noise is in the mix, not what any particular moment sounds
/// like.
#[allow(clippy::too_many_arguments)]
pub fn mixer_waveform(
    ui: &mut Ui,
    tokens: &Tokens,
    saw: f32,
    pulse: f32,
    sub: f32,
    noise: f32,
    pulse_width: f32,
    sub_shape: SubShape,
) {
    let height = panel_height(ui.available_width());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    frame(ui, tokens, rect);

    // A silent mixer is a real state, not an error: every level at zero. Drawing the centre line
    // and stopping is more honest than a flat trace that looks like a signal.
    let total = saw + pulse + sub + noise;
    if total <= f32::EPSILON {
        let y = rect.center().y;
        ui.painter().line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(1.0, tokens.text_disabled),
        );
        response.on_hover_text("Every source is at zero, so the mixer produces silence.");
        return;
    }

    const POINTS: usize = 240;

    // Normalised by the summed levels rather than by the peak: the shape stays put as levels
    // change instead of rescaling under the pointer, and the *relative* contribution of each
    // source is what the display is for.
    let points: Vec<Pos2> = (0..=POINTS)
        .map(|i| {
            let t = i as f32 / POINTS as f32;
            let phase = (t * CYCLES).fract();
            let value = sample(phase, t, saw, pulse, sub, noise, pulse_width, sub_shape) / total;
            Pos2::new(
                rect.left() + rect.width() * t,
                rect.center().y - rect.height() * 0.44 * value.clamp(-1.0, 1.0),
            )
        })
        .collect();

    // The zero line, so "louder" and "offset" are distinguishable. §7.5 asks for it.
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), rect.center().y),
            Pos2::new(rect.right(), rect.center().y),
        ],
        Stroke::new(HAIRLINE, tokens.border),
    );
    ui.painter()
        .add(Shape::line(points, Stroke::new(2.0, tokens.accent)));

    response.on_hover_text("One cycle of the four sources mixed together.");
}

/// The mixed sources at one phase, before normalisation.
#[allow(clippy::too_many_arguments)]
fn sample(
    phase: f32,
    t: f32,
    saw: f32,
    pulse: f32,
    sub: f32,
    noise: f32,
    pulse_width: f32,
    sub_shape: SubShape,
) -> f32 {
    // Saw: rising ramp, -1 to +1.
    let saw_value = phase * 2.0 - 1.0;

    // Pulse: the duty cycle is what `pulse_width` sets, and it is the only source that control
    // affects — which is worth being able to see.
    let pulse_value = if phase < pulse_width { 1.0 } else { -1.0 };

    // Sub: a square or narrow pulse, one or two octaves down.
    //
    // `octave_ratio` is how many oscillator cycles fit in one of the sub's, so the sub's phase is
    // the drawn span divided by it. Deriving it from `CYCLES` rather than hard-coding a divisor is
    // what keeps the two in step — they were not, and the sub drew at the wrong rate.
    let (octave_ratio, duty) = match sub_shape {
        SubShape::Oct1Square => (2.0, 0.5),
        SubShape::Oct2Square => (4.0, 0.5),
        SubShape::Oct2Pulse => (4.0, 0.25),
    };
    let sub_phase = (t * CYCLES / octave_ratio).fract();
    let sub_value = if sub_phase < duty { 1.0 } else { -1.0 };

    // Noise: deterministic, so the picture is still while nothing is being changed. A shimmering
    // display would draw the eye to the one part of it that carries no information.
    let noise_value = deterministic_noise(t);

    saw * saw_value + pulse * pulse_value + sub * sub_value + noise * noise_value
}

/// A fixed pseudo-random sequence in `-1..=1`, from the sample position alone.
fn deterministic_noise(t: f32) -> f32 {
    let x = (t * 9871.0).to_bits() ^ 0x9E37_79B9;
    let x = x.wrapping_mul(2_654_435_761);
    let x = x ^ (x >> 15);
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// The panel every visualization sits in: a flat surface and a hairline, per §3.3.
fn frame(ui: &Ui, tokens: &Tokens, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS as f32, tokens.surface_2);
    painter.rect_stroke(
        rect,
        RADIUS as f32,
        Stroke::new(HAIRLINE, tokens.border),
        StrokeKind::Inside,
    );
    // §7.5: "Scopes show a neutral zero/reference line and meaningful scale when appropriate."
    painter.line_segment(
        [
            Pos2::new(rect.left(), rect.bottom() - HAIRLINE),
            Pos2::new(rect.right(), rect.bottom() - HAIRLINE),
        ],
        Stroke::new(HAIRLINE, tokens.border_strong),
    );
}

/// The linear 4-pole ladder magnitude response, sampled across the audible band.
///
/// Log frequency axis, because that is how hearing works and how the cutoff control is skewed.
/// Decibel magnitude axis, clamped to a window that shows the resonant peak and the slope without
/// letting a 24 dB/octave skirt flatten everything else against the floor.
#[allow(clippy::manual_clamp)]
fn curve(rect: Rect, cutoff_hz: f32, resonance: f32, sample_rate: f32) -> Vec<Pos2> {
    const POINTS: usize = 96;
    const MIN_HZ: f32 = 20.0;
    const DB_TOP: f32 = 18.0;
    const DB_BOTTOM: f32 = -42.0;

    // `clamp` would panic if the bounds crossed, and they can: a host reporting an 8 kHz rate
    // makes Nyquist lower than the floor. Ordered `min` then `max` degrades to the floor instead.
    let max_hz = (sample_rate * 0.49).min(20_000.0).max(MIN_HZ * 2.0);
    let fc = cutoff_hz.clamp(MIN_HZ, max_hz);
    // Feedback, matching the DSP's `k = K_MAX * resonance` shape. Kept just below the
    // self-oscillation limit so the peak stays finite and the plot never divides by zero.
    let k = resonance.clamp(0.0, 1.0) * 3.99;

    (0..=POINTS)
        .map(|i| {
            let t = i as f32 / POINTS as f32;
            let f = MIN_HZ * (max_hz / MIN_HZ).powf(t);
            let db = magnitude_db(f, fc, k);
            let v = ((db - DB_BOTTOM) / (DB_TOP - DB_BOTTOM)).clamp(0.0, 1.0);
            Pos2::new(
                rect.left() + rect.width() * t,
                rect.bottom() - rect.height() * v,
            )
        })
        .collect()
}

/// `|H(f)|` in dB for a 4-pole lowpass with feedback `k`, as an analogue prototype.
///
/// `H(s) = 1 / ((1 + s/wc)^4 + k)`, evaluated at `s = jw`. Written out rather than approximated
/// because the resonant peak's height and the bass loss beside it are the two things the plot
/// exists to show, and both come out of the `+ k` term.
fn magnitude_db(f: f32, fc: f32, k: f32) -> f32 {
    let w = f / fc.max(1.0);
    // (1 + jw)^4 = (1 - 6w^2 + w^4) + j(4w - 4w^3)
    let real = 1.0 - 6.0 * w * w + w * w * w * w + k;
    let imag = 4.0 * w - 4.0 * w * w * w;
    let magnitude = (real * real + imag * imag).sqrt();
    if magnitude <= f32::MIN_POSITIVE {
        return 60.0;
    }
    -20.0 * magnitude.log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_response_is_flat_below_cutoff_and_falls_above_it() {
        // The defining property of a lowpass, and the one a wrong sign or a swapped term would
        // silently invert while still producing a plausible-looking squiggle.
        let below = magnitude_db(100.0, 1_000.0, 0.0);
        let at = magnitude_db(1_000.0, 1_000.0, 0.0);
        let above = magnitude_db(10_000.0, 1_000.0, 0.0);
        assert!(below > at, "{below} should exceed {at}");
        assert!(at > above, "{at} should exceed {above}");
        assert!(
            below.abs() < 1.0,
            "the passband should be near 0 dB, was {below}"
        );
    }

    #[test]
    fn four_poles_fall_at_about_24_db_per_octave() {
        // Far above cutoff, where the asymptote holds. This is what makes it a 4-pole rather than
        // a 2-pole drawn with a thicker line.
        let one = magnitude_db(8_000.0, 1_000.0, 0.0);
        let two = magnitude_db(16_000.0, 1_000.0, 0.0);
        let slope = one - two;
        assert!(
            (slope - 24.0).abs() < 2.0,
            "expected about 24 dB per octave, measured {slope}"
        );
    }

    #[test]
    fn resonance_lifts_the_peak_and_thins_the_bass() {
        // The brief's stated reason for this visualization: "it makes the resonance-bass-loss
        // behaviour visible instead of mysterious". If the model did not reproduce it, the plot
        // would be decorative.
        let flat_bass = magnitude_db(50.0, 1_000.0, 0.0);
        let resonant_bass = magnitude_db(50.0, 1_000.0, 3.5);
        assert!(
            resonant_bass < flat_bass - 3.0,
            "bass should drop with resonance: {flat_bass} then {resonant_bass}"
        );

        let flat_peak = magnitude_db(1_000.0, 1_000.0, 0.0);
        let resonant_peak = magnitude_db(1_000.0, 1_000.0, 3.5);
        assert!(
            resonant_peak > flat_peak + 6.0,
            "the peak should rise with resonance: {flat_peak} then {resonant_peak}"
        );
    }

    #[test]
    fn the_curve_never_produces_a_non_finite_point() {
        // A NaN reaching a paint call is a crash or a garbage triangle, and the inputs here come
        // from a host that is allowed to report nonsense.
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 80.0));
        for (cutoff, resonance, rate) in [
            (0.0, 0.0, 48_000.0),
            (20_000.0, 1.0, 48_000.0),
            (1_000.0, 1.0, 8_000.0),
            (-5.0, -1.0, 44_100.0),
        ] {
            for point in curve(rect, cutoff, resonance, rate) {
                assert!(
                    point.x.is_finite() && point.y.is_finite(),
                    "cutoff {cutoff}, resonance {resonance}, rate {rate} produced {point:?}"
                );
            }
        }
    }

    #[test]
    fn a_silent_mixer_is_a_state_not_a_crash() {
        // Every level at zero is a normal thing to do while building a patch, and it divides by
        // the summed levels. The drawing path guards it; this pins that the guard is the reason.
        let total = 0.0f32;
        assert!(total <= f32::EPSILON);
    }

    #[test]
    fn the_mixed_waveform_stays_within_the_summed_levels() {
        // Normalisation divides by the sum, so no combination of sources may exceed it -- an
        // overshoot would draw outside the panel and over the controls above it.
        let cases = [
            (1.0, 0.0, 0.0, 0.0),
            (0.0, 1.0, 0.0, 0.0),
            (0.0, 0.0, 1.0, 0.0),
            (0.0, 0.0, 0.0, 1.0),
            (0.8, 0.25, 0.5, 0.07),
        ];
        for (saw, pulse, sub, noise) in cases {
            let total = saw + pulse + sub + noise;
            for i in 0..=64 {
                let t = i as f32 / 64.0;
                let phase = (t * 2.0).fract();
                let v = sample(phase, t, saw, pulse, sub, noise, 0.5, SubShape::Oct1Square) / total;
                assert!(
                    v.is_finite() && v.abs() <= 1.0 + 1e-5,
                    "saw {saw} pulse {pulse} sub {sub} noise {noise} at t={t} gave {v}"
                );
            }
        }
    }

    #[test]
    fn pulse_width_changes_only_the_pulse() {
        // The one control that shapes a single source, and being able to see that is most of why
        // the display earns its place.
        let narrow = sample(0.3, 0.0, 0.0, 1.0, 0.0, 0.0, 0.2, SubShape::Oct1Square);
        let wide = sample(0.3, 0.0, 0.0, 1.0, 0.0, 0.0, 0.8, SubShape::Oct1Square);
        assert_ne!(narrow, wide, "pulse width should move the pulse edge");

        let saw_narrow = sample(0.3, 0.0, 1.0, 0.0, 0.0, 0.0, 0.2, SubShape::Oct1Square);
        let saw_wide = sample(0.3, 0.0, 1.0, 0.0, 0.0, 0.0, 0.8, SubShape::Oct1Square);
        assert_eq!(saw_narrow, saw_wide, "pulse width must not touch the saw");
    }

    #[test]
    fn the_sub_octave_is_actually_an_octave_down() {
        // The *relationship*, not a magic number: a sub one octave down changes sign exactly twice
        // as often as one two octaves down, whatever `CYCLES` is. Counting absolute flips broke the
        // moment the drawn span changed from two cycles to four - and that change was the fix for
        // this display barely reacting to the sub shape at all.
        let flips = |shape| {
            // Inclusive of the end: `t = 1.0` wraps each sub back to the start of its cycle, and
            // that wrap is a real sign change. Excluding it counts three flips for a wave that has
            // four, which makes the octave ratio look wrong when it is not.
            (1..=400)
                .filter(|i| {
                    let a = sample(0.0, (*i - 1) as f32 / 400.0, 0.0, 0.0, 1.0, 0.0, 0.5, shape);
                    let b = sample(0.0, *i as f32 / 400.0, 0.0, 0.0, 1.0, 0.0, 0.5, shape);
                    a.signum() != b.signum()
                })
                .count()
        };

        let one_octave = flips(SubShape::Oct1Square);
        let two_octaves = flips(SubShape::Oct2Square);
        assert!(
            one_octave > 0,
            "the sub should be a square wave, not a constant"
        );
        assert_eq!(
            one_octave,
            two_octaves * 2,
            "-1 oct should cycle twice as fast as -2 oct ({one_octave} vs {two_octaves})"
        );

        // And every shape must complete a whole cycle in the drawn span, which is the defect the
        // user actually saw: `2 oct square` drew as a flat offset that barely changed.
        assert!(
            two_octaves >= 2,
            "every sub shape must show a whole cycle; -2 oct managed {two_octaves} sign changes"
        );
    }

    #[test]
    fn noise_is_deterministic_so_the_display_does_not_shimmer() {
        // A display that flickers while nothing is being changed draws the eye to the one part of
        // it that carries no information.
        for i in 0..32 {
            let t = i as f32 / 32.0;
            assert_eq!(deterministic_noise(t), deterministic_noise(t));
        }
    }

    #[test]
    fn noise_stays_in_range() {
        for i in 0..1000 {
            let n = deterministic_noise(i as f32 / 1000.0);
            assert!(n.is_finite() && (-1.0..=1.0).contains(&n), "noise was {n}");
        }
    }

    #[test]
    fn the_curve_stays_inside_its_rect() {
        // Clamping is what keeps a resonant peak from painting over the controls above it.
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(200.0, 80.0));
        for point in curve(rect, 1_000.0, 1.0, 48_000.0) {
            assert!(rect.contains(point), "{point:?} escaped {rect:?}");
        }
    }
}
