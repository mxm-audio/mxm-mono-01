//! Band-limited oscillators, sub-oscillator and noise.
//!
//! Waveforms are generated with PolyBLEP (polynomial band-limited step) residuals:
//! cheap, no wavetables, and quiet enough for an instrument whose character comes
//! from the filter rather than the oscillator.
//!
//! Two details that are easy to get wrong and are handled explicitly here:
//!
//! * **The sub-oscillator gets PolyBLEP too.** Running it from its own phasor
//!   avoids the aliasing a flip-flop divider would produce, but the sub's own
//!   square and pulse edges still alias without band limiting.
//! * **Phasors free-run.** They are never reset on note-on. Resetting mid-legato
//!   is a waveform discontinuity and an audible click, and an analog VCO does not
//!   do it either.

use crate::{Rng, flush};

/// Lowest frequency the oscillator will produce.
const FREQ_MIN_HZ: f32 = 8.0;

/// Highest frequency, as a fraction of the sample rate. MIDI 127 at the 2' setting
/// with pitch bend and LFO on top can otherwise exceed Nyquist, and PolyBLEP does
/// not make an invalid phase increment safe.
const NYQUIST_FRACTION: f32 = 0.45;

/// PolyBLEP residual for a step discontinuity.
///
/// `t` is the phase in `0..1` and `dt` the per-sample phase increment. The residual
/// corrects the two samples either side of a discontinuity, which removes most of
/// the aliasing energy for the cost of a few operations.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

/// A free-running phase accumulator.
#[derive(Debug, Clone, Copy, Default)]
pub struct Phasor {
    phase: f32,
    inc: f32,
}

impl Phasor {
    pub const fn new() -> Self {
        Self {
            phase: 0.0,
            inc: 0.0,
        }
    }

    /// Set the increment from a frequency, clamping so the increment stays valid.
    #[inline]
    pub fn set_freq(&mut self, hz: f32, sample_rate: f32) {
        let hz = hz.clamp(FREQ_MIN_HZ, NYQUIST_FRACTION * sample_rate);
        self.inc = hz / sample_rate;
    }

    /// Set the increment directly. Used for the sub-oscillator, whose increment is
    /// derived exactly from the main one rather than recomputed from a frequency.
    #[inline]
    pub fn set_inc(&mut self, inc: f32) {
        self.inc = inc.clamp(0.0, NYQUIST_FRACTION);
    }

    #[inline]
    pub fn phase(&self) -> f32 {
        self.phase
    }

    #[inline]
    pub fn inc(&self) -> f32 {
        self.inc
    }

    #[inline]
    pub fn advance(&mut self) {
        self.phase += self.inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
    }

    /// Zero the phase. Called from `reset()`, never from note-on.
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }
}

/// Band-limited sawtooth, falling edge corrected.
#[inline]
pub fn saw(p: &Phasor) -> f32 {
    2.0 * p.phase() - 1.0 - poly_blep(p.phase(), p.inc())
}

/// Band-limited pulse of the given width, both edges corrected.
#[inline]
pub fn pulse(p: &Phasor, width: f32) -> f32 {
    let (t, dt) = (p.phase(), p.inc());
    let w = clamp_pulse_width(width, dt);
    let mut y = if t < w { 1.0 } else { -1.0 };
    y += poly_blep(t, dt);
    let second = {
        let x = t - w;
        if x < 0.0 { x + 1.0 } else { x }
    };
    y -= poly_blep(second, dt);
    y
}

/// Keep the two PolyBLEP corrections from overlapping.
///
/// Each correction spans `dt` either side of an edge, so if the pulse is narrower
/// than `2*dt` the two would overlap and produce nonsense. At extreme frequencies
/// where no valid width exists, fall back to a square.
#[inline]
pub fn clamp_pulse_width(width: f32, dt: f32) -> f32 {
    let lo = 0.05f32.max(2.0 * dt);
    let hi = 0.95f32.min(1.0 - 2.0 * dt);
    if lo > hi { 0.5 } else { width.clamp(lo, hi) }
}

/// Sub-oscillator shape. The names are octaves below the main oscillator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubShape {
    /// One octave down, square.
    #[default]
    Oct1Square,
    /// Two octaves down, square.
    Oct2Square,
    /// Two octaves down, narrow pulse. Thinner and more harmonically dense.
    Oct2Pulse,
}

impl SubShape {
    /// Divisor applied to the main oscillator's phase increment.
    #[inline]
    fn divisor(self) -> f32 {
        match self {
            SubShape::Oct1Square => 2.0,
            SubShape::Oct2Square | SubShape::Oct2Pulse => 4.0,
        }
    }

    #[inline]
    fn width(self) -> f32 {
        match self {
            SubShape::Oct1Square | SubShape::Oct2Square => 0.5,
            SubShape::Oct2Pulse => 0.25,
        }
    }
}

/// One-pole DC blocker.
///
/// A pulse wave is not symmetric unless its width is exactly 50%, so pulse-width
/// modulation swings the DC offset around. This does the job the hardware's AC
/// coupling does. Cut at 15 Hz, deliberately below the fundamental of the lowest
/// note at the 16' setting, so it removes offset without thinning the bass.
#[derive(Debug, Clone, Copy, Default)]
pub struct DcBlocker {
    x1: f32,
    y1: f32,
    r: f32,
}

impl DcBlocker {
    pub const CUTOFF_HZ: f32 = 15.0;

    pub const fn new() -> Self {
        Self {
            x1: 0.0,
            y1: 0.0,
            r: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.r = 1.0 - (std::f32::consts::TAU * Self::CUTOFF_HZ / sample_rate);
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = x - self.x1 + self.r * self.y1;
        self.x1 = x;
        self.y1 = flush(y);
        self.y1
    }
}

/// The complete oscillator section: one main oscillator, a sub, and noise.
#[derive(Debug, Clone)]
pub struct Oscillator {
    main: Phasor,
    sub: Phasor,
    noise: Rng,
    last: Parts,
}

impl Default for Oscillator {
    fn default() -> Self {
        Self::new()
    }
}

/// Mixer levels for the four sources, each `0..=1`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MixLevels {
    pub saw: f32,
    pub pulse: f32,
    pub sub: f32,
    pub noise: f32,
}

impl Oscillator {
    pub const fn new() -> Self {
        Self {
            main: Phasor::new(),
            sub: Phasor::new(),
            noise: Rng::new(0x0517_0101),
            last: Parts::silent(),
        }
    }

    pub fn reset(&mut self) {
        self.main.reset();
        self.sub.reset();
        self.noise = Rng::new(0x0517_0101);
        // The published parts are state a backward route reads, so a reset that left them would let
        // one render leak a sample into the next.
        self.last = Parts::silent();
    }

    /// Render one sample of the mixed oscillator section.
    ///
    /// The `0.5` headroom keeps four sources at full level from slamming the
    /// filter's input saturator.
    #[inline]
    pub fn process(
        &mut self,
        freq_hz: f32,
        pulse_width: f32,
        sub_shape: SubShape,
        levels: &MixLevels,
        sample_rate: f32,
    ) -> f32 {
        self.main.set_freq(freq_hz, sample_rate);
        // Derived from the main increment rather than from a divided frequency, so
        // the sub stays exactly an octave (or two) down even as pitch moves.
        self.sub.set_inc(self.main.inc() / sub_shape.divisor());

        let parts = Parts {
            saw: saw(&self.main),
            pulse: pulse(&self.main, pulse_width),
            sub: pulse(&self.sub, sub_shape.width()),
            noise: self.noise.next_bipolar(),
        };

        self.main.advance();
        self.sub.advance();

        self.last = parts;
        parts.mixed(levels)
    }

    /// What each source produced on the **last** call to [`Oscillator::process`].
    ///
    /// These are the instrument's own audio, made routable so a target can be modulated by an
    /// oscillator or by noise — which is what `plans/plan-modulation-routing.md` §2.3 means by
    /// exposing what is already generating rather than inventing a new generator, and what makes FM
    /// reachable at all. They were already computed separately here and mixed at the end; nothing
    /// new is produced to publish them.
    ///
    /// **Last, not this**: they exist only after the oscillator has run, so a route from one of them
    /// into pitch or width is a backward route and is one sample late. That is the unit delay, and
    /// `routing`'s declared source order is where it is written down.
    #[must_use]
    pub fn parts(&self) -> Parts {
        self.last
    }
}

/// One sample of each thing the oscillator section makes, before the mixer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Parts {
    /// The main oscillator's sawtooth.
    pub saw: f32,
    /// The main oscillator's pulse, at the current width.
    pub pulse: f32,
    /// The sub oscillator.
    pub sub: f32,
    /// The noise generator.
    pub noise: f32,
}

impl Parts {
    /// Nothing produced yet — what a fresh or reset oscillator has published.
    #[must_use]
    pub const fn silent() -> Self {
        Self {
            saw: 0.0,
            pulse: 0.0,
            sub: 0.0,
            noise: 0.0,
        }
    }

    /// The mixer's own sum, which is what the voice hears.
    #[inline]
    #[must_use]
    pub fn mixed(&self, levels: &MixLevels) -> f32 {
        (self.saw * levels.saw
            + self.pulse * levels.pulse
            + self.sub * levels.sub
            + self.noise * levels.noise)
            * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mxm_measure::{convert, pitch};

    const RATES: [f32; 4] = [44_100.0, 48_000.0, 96_000.0, 192_000.0];

    fn only(saw: f32, pulse: f32, sub: f32, noise: f32) -> MixLevels {
        MixLevels {
            saw,
            pulse,
            sub,
            noise,
        }
    }

    /// Renders the oscillator and measures its frequency with `mxm-measure`'s interpolated ruler.
    ///
    /// **This used to count whole zero crossings over a fixed window**, which quantises to ±1 cycle
    /// — about ±9 cents at 55 Hz over two seconds, an order of magnitude worse than the one cent
    /// `saw_frequency_is_accurate_to_a_cent` claims to assert. `mxm-mono-00-dsp` found that and
    /// interpolated its crossings; the fix stayed where it was made until the ruler became shared.
    fn measure_freq(levels: MixLevels, freq: f32, fs: f32, secs: f32) -> f64 {
        let mut osc = Oscillator::new();
        let n = (fs * secs) as usize;
        let rendered: Vec<f32> = (0..n)
            .map(|_| osc.process(freq, 0.5, SubShape::Oct1Square, &levels, fs))
            .collect();
        pitch::frequency_by_crossings(&rendered, f64::from(fs)).expect("the oscillator sounds")
    }

    #[test]
    fn saw_frequency_is_accurate_to_a_tenth_of_a_cent() {
        // **The bound was re-derived when the ruler was corrected, not rescaled.** This asserted one
        // cent while being measured by a crossing *count*, which quantises to ±1 cycle — ±9 cents at
        // 55 Hz over two seconds. The assertion could not have failed for any tuning error smaller
        // than the ruler's own error, so it was not measuring the oscillator.
        //
        // With the interpolated ruler the worst case over these twenty combinations is **0.009
        // cents**, at 55 Hz and 192 kHz. A tenth of a cent leaves an order of magnitude of headroom
        // against that and is ninety times tighter than the claim it replaces — so a real detuning
        // now fails it, which is what the test was always for.
        for fs in RATES {
            for freq in [55.0f32, 110.0, 440.0, 1_000.0, 4_000.0] {
                let measured = measure_freq(only(1.0, 0.0, 0.0, 0.0), freq, fs, 2.0);
                let cents = convert::cents_error(measured, f64::from(freq));
                assert!(
                    cents.abs() < 0.1,
                    "{freq} Hz at {fs}: measured {measured}, off by {cents:.4} cents"
                );
            }
        }
    }

    #[test]
    fn sub_is_exactly_one_or_two_octaves_below() {
        let fs = 48_000.0;
        for (shape, ratio) in [
            (SubShape::Oct1Square, 0.5f32),
            (SubShape::Oct2Square, 0.25),
            (SubShape::Oct2Pulse, 0.25),
        ] {
            let mut osc = Oscillator::new();
            let freq = 440.0;
            let secs = 2.0;
            let n = (fs * secs) as usize;
            let rendered: Vec<f32> = (0..n)
                .map(|_| osc.process(freq, 0.5, shape, &only(0.0, 0.0, 1.0, 0.0), fs))
                .collect();
            let measured = pitch::frequency_by_crossings(&rendered, f64::from(fs))
                .expect("the sub oscillator sounds");
            let expected = f64::from(freq * ratio);
            let cents = convert::cents_error(measured, expected);
            assert!(
                cents.abs() < 1.0,
                "{shape:?}: expected {expected} Hz, measured {measured} ({cents:.2} cents off)"
            );
        }
    }

    #[test]
    fn dc_offset_stays_near_zero_across_the_pulse_width_range() {
        let fs = 48_000.0;
        for width in [0.05f32, 0.15, 0.3, 0.5, 0.7, 0.85, 0.95] {
            let mut osc = Oscillator::new();
            let mut dc = DcBlocker::new();
            dc.set_sample_rate(fs);
            let n = (fs * 0.5) as usize;
            // Discard the settling transient.
            for _ in 0..n / 2 {
                let y = osc.process(
                    220.0,
                    width,
                    SubShape::Oct1Square,
                    &only(0.0, 1.0, 0.0, 0.0),
                    fs,
                );
                dc.process(y);
            }
            let mut sum = 0.0f64;
            for _ in 0..n {
                let y = osc.process(
                    220.0,
                    width,
                    SubShape::Oct1Square,
                    &only(0.0, 1.0, 0.0, 0.0),
                    fs,
                );
                sum += dc.process(y) as f64;
            }
            let mean = (sum / n as f64).abs();
            assert!(mean < 1e-3, "width {width}: mean offset {mean}");
        }
    }

    #[test]
    fn dc_blocker_passes_the_low_bass_it_is_supposed_to() {
        // 16' low notes sit near 20-40 Hz; the blocker must not thin them out.
        let fs = 48_000.0;
        for freq in [40.0f32, 60.0, 100.0] {
            let mut dc = DcBlocker::new();
            dc.set_sample_rate(fs);
            let n = (fs * 1.0) as usize;
            let mut peak = 0.0f32;
            for i in 0..n {
                let t = i as f32 / fs;
                let y = dc.process((std::f32::consts::TAU * freq * t).sin());
                if i > n / 2 {
                    peak = peak.max(y.abs());
                }
            }
            let db = 20.0 * peak.log10();
            assert!(
                db > -1.0,
                "{freq} Hz attenuated by {db:.2} dB, expected < 1 dB"
            );
        }
    }

    #[test]
    fn output_is_bounded_and_finite_at_extreme_pitch() {
        // MIDI 127 at the 2' setting, plus bend and LFO, should still be safe.
        for fs in RATES {
            let mut osc = Oscillator::new();
            for freq in [0.0f32, 1.0, 20_000.0, 40_000.0, 1e9, -100.0] {
                for width in [0.0f32, 0.05, 0.5, 0.95, 1.0] {
                    for _ in 0..2_000 {
                        let y = osc.process(
                            freq,
                            width,
                            SubShape::Oct2Pulse,
                            &only(1.0, 1.0, 1.0, 1.0),
                            fs,
                        );
                        assert!(
                            y.is_finite(),
                            "non-finite at {freq} Hz, width {width}, {fs} Hz"
                        );
                        assert!(y.abs() <= 2.0, "unbounded: {y}");
                    }
                }
            }
        }
    }

    #[test]
    fn phase_increment_never_exceeds_the_nyquist_limit() {
        let mut p = Phasor::new();
        for fs in RATES {
            p.set_freq(1e9, fs);
            assert!(
                p.inc() <= NYQUIST_FRACTION,
                "increment {} too large",
                p.inc()
            );
            p.set_freq(-1000.0, fs);
            assert!(p.inc() > 0.0, "increment must stay positive");
        }
    }

    #[test]
    fn pulse_width_clamp_keeps_blep_corrections_apart() {
        for dt in [0.0001f32, 0.01, 0.1, 0.2, 0.3, 0.45] {
            for w in [0.0f32, 0.05, 0.5, 0.95, 1.0] {
                let c = clamp_pulse_width(w, dt);
                assert!((0.0..=1.0).contains(&c), "width {c} out of range");
                if 2.0 * dt < 0.5 {
                    assert!(c >= 2.0 * dt - 1e-6, "width {c} too narrow for dt {dt}");
                    assert!(c <= 1.0 - 2.0 * dt + 1e-6, "width {c} too wide for dt {dt}");
                }
            }
        }
    }

    #[test]
    fn silence_when_all_levels_are_zero() {
        for fs in RATES {
            let mut osc = Oscillator::new();
            for _ in 0..10_000 {
                let y = osc.process(
                    440.0,
                    0.5,
                    SubShape::Oct1Square,
                    &only(0.0, 0.0, 0.0, 0.0),
                    fs,
                );
                assert_eq!(y, 0.0);
            }
        }
    }

    #[test]
    fn changing_frequency_never_resets_the_phase() {
        // This is the property that keeps legato note changes click-free: the
        // oscillator must retune without discontinuity. Asserted on the phasor
        // directly rather than inferred from the output, because PolyBLEP spreads
        // the waveform's own wrap across two samples and those large-but-legitimate
        // sample deltas make an output-based check meaningless.
        let fs = 48_000.0;
        let mut p = Phasor::new();
        p.set_freq(220.0, fs);
        for _ in 0..100 {
            p.advance();
        }

        let before = p.phase();
        p.set_freq(880.0, fs);
        assert_eq!(p.phase(), before, "set_freq moved the phase");

        p.advance();
        let expected = (before + 880.0 / fs) % 1.0;
        assert!(
            (p.phase() - expected).abs() < 1e-6,
            "phase {} did not advance by exactly the new increment (expected {expected})",
            p.phase()
        );
    }

    #[test]
    fn sub_phase_is_locked_to_the_main_oscillator() {
        // The sub is derived from the main increment, so it must stay exactly an
        // octave down even while the pitch moves.
        let fs = 48_000.0;
        let mut osc = Oscillator::new();
        let levels = only(0.0, 0.0, 1.0, 0.0);
        for i in 0..10_000 {
            let freq = 110.0 * (1.0 + i as f32 / 5_000.0);
            osc.process(freq, 0.5, SubShape::Oct1Square, &levels, fs);
            let ratio = osc.main.inc() / osc.sub.inc();
            assert!(
                (ratio - 2.0).abs() < 1e-4,
                "sub drifted: increment ratio {ratio}"
            );
        }
    }

    /// Alias-to-signal ratio of an exactly-periodic buffer, in dB.
    ///
    /// The frequency is chosen as `f0 = periods * fs / N` with `periods` odd and
    /// `N` a power of two, so the waveform repeats exactly in the window. Every
    /// wanted harmonic then lands on bin `k*periods`, every aliased component on
    /// bin `k*periods mod N`, and because `gcd(periods, N) = 1` the two sets never
    /// collide. Alias energy is therefore separated exactly, with no window
    /// function, no leakage and no peak picking.
    ///
    /// A direct DFT rather than an FFT: `N` is small, this is a test, and the
    /// clarity is worth more than the microseconds.
    fn alias_to_signal_db(x: &[f64], periods: usize) -> f64 {
        let n = x.len();
        let half = n / 2;
        let (mut wanted, mut alias) = (0.0f64, 0.0f64);
        for bin in 1..half {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, &v) in x.iter().enumerate() {
                let ang = -2.0 * std::f64::consts::PI * bin as f64 * i as f64 / n as f64;
                re += v * ang.cos();
                im += v * ang.sin();
            }
            let power = re * re + im * im;
            if bin % periods == 0 {
                wanted += power;
            } else {
                alias += power;
            }
        }
        10.0 * (alias / wanted.max(1e-30)).max(1e-30).log10()
    }

    /// The sawtooth's aliasing, measured, with the measurement validated against an
    /// ideal additive sawtooth in the same test.
    ///
    /// This is a regression guard, not a quality target: the threshold is set well
    /// below what the shipped PolyBLEP measures (-35 dB at 44.1 kHz per
    /// `examples/osc_spike.rs`) and well above what a trivial modulo counter would
    /// reach (-19 dB), so it fails loudly if the band limiting is ever broken or
    /// silently bypassed, and does not fail for a 1 dB drift.
    #[test]
    fn sawtooth_aliasing_stays_far_below_the_trivial_waveform() {
        const N: usize = 2048;
        const PERIODS: usize = 21; // odd, so gcd(PERIODS, N) = 1
        let fs = 44_100.0f32;
        let inc = PERIODS as f32 / N as f32;

        // Validate the measurement itself: an additive sawtooth has no aliasing,
        // and anything the analysis reports for it is its own numerical floor.
        let harmonics = (N / 2 - 1) / PERIODS;
        let ideal: Vec<f64> = (0..N)
            .map(|i| {
                (1..=harmonics)
                    .map(|k| {
                        let ang =
                            2.0 * std::f64::consts::PI * (k * PERIODS) as f64 * i as f64 / N as f64;
                        -2.0 / (std::f64::consts::PI * k as f64) * ang.sin()
                    })
                    .sum()
            })
            .collect();
        let floor = alias_to_signal_db(&ideal, PERIODS);
        assert!(
            floor < -100.0,
            "the analysis is not trustworthy: an ideal saw measured {floor:.1} dB"
        );

        // The trivial waveform, for the same reason: it establishes the top of the
        // range the shipped code has to beat.
        let mut phase = 0.0f32;
        let trivial: Vec<f64> = (0..N)
            .map(|_| {
                let y = 2.0 * phase - 1.0;
                phase += inc;
                if phase >= 1.0 {
                    phase -= 1.0;
                }
                y as f64
            })
            .collect();
        let trivial_db = alias_to_signal_db(&trivial, PERIODS);

        let mut p = Phasor::new();
        p.set_inc(inc);
        let shipped: Vec<f64> = (0..N)
            .map(|_| {
                let y = saw(&p);
                p.advance();
                y as f64
            })
            .collect();
        let shipped_db = alias_to_signal_db(&shipped, PERIODS);

        assert!(
            shipped_db < -30.0,
            "sawtooth aliasing {shipped_db:.1} dB at {} Hz, expected below -30 dB",
            inc * fs
        );
        assert!(
            shipped_db < trivial_db - 10.0,
            "band limiting bought only {:.1} dB over the trivial waveform              (shipped {shipped_db:.1} dB, trivial {trivial_db:.1} dB)",
            trivial_db - shipped_db
        );
    }

    /// The pulse wave's worst case is a narrow width, where the two corrections are
    /// closest together and the spectrum is widest.
    #[test]
    fn pulse_aliasing_holds_up_at_narrow_widths() {
        const N: usize = 2048;
        const PERIODS: usize = 21;
        let inc = PERIODS as f32 / N as f32;

        for width in [0.05f32, 0.25, 0.5] {
            let mut p = Phasor::new();
            p.set_inc(inc);
            let x: Vec<f64> = (0..N)
                .map(|_| {
                    let y = pulse(&p, width);
                    p.advance();
                    y as f64
                })
                .collect();
            let db = alias_to_signal_db(&x, PERIODS);
            assert!(
                db < -25.0,
                "pulse width {width}: aliasing {db:.1} dB, expected below -25 dB"
            );
        }
    }
}
