//! Low-frequency oscillator.
//!
//! Five shapes, all bipolar in `[-1, 1]`. The LFO **free-runs**: its phase is not
//! reset by note-on, matching the hardware. That is the narrow meaning of the term
//! here — it does not imply the LFO keeps advancing while the plugin is suspended.
//!
//! No band limiting. At a 30 Hz ceiling the square and ramp shapes do alias, but an
//! LFO is a modulation source rather than something you hear directly, and band
//! limiting it would round off the very edges that make a square LFO useful.

use crate::Rng;
use crate::oscillator::Phasor;

/// Shapes, in the order they appear in the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfoShape {
    /// Starts at 0 and rises. The default, and the one that sounds like vibrato.
    #[default]
    Triangle,
    /// 50% duty, `+1` at phase zero.
    Square,
    /// Ramps -1 up to +1 across the cycle.
    SawUp,
    /// Ramps +1 down to -1 across the cycle.
    SawDown,
    /// Sample and hold: a new uniform value at each cycle wrap, held in between.
    Random,
}

#[derive(Debug, Clone)]
pub struct Lfo {
    phasor: Phasor,
    /// Current sample-and-hold value, only used by [`LfoShape::Random`].
    held: f32,
    rng: Rng,
}

impl Default for Lfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Seed for the sample-and-hold source. Fixed and restored on `reset()` so a
/// rendered take is bit-identical when replayed.
const SEED: u32 = 0x10F0_0101;

impl Lfo {
    #[must_use]
    pub fn new() -> Self {
        let mut lfo = Self {
            phasor: Phasor::new(),
            held: 0.0,
            rng: Rng::new(SEED),
        };
        lfo.reset();
        lfo
    }

    /// **The first held value is drawn here, not left at zero.**
    ///
    /// [`LfoShape::Random`] only draws on a phase wrap, so a held value of zero means the shape
    /// emits *nothing* until the first cycle completes — and this LFO goes down to 0.05 Hz, so
    /// that is twenty seconds of a route that looks connected doing nothing at all. Drawing one
    /// here makes the first cycle a real value like every cycle after it, and the fixed `SEED`
    /// keeps a replayed take bit-identical.
    ///
    /// The same omission was found in `mxm-drum-machine-dsp` on 2026-09-22, where it sat beside a
    /// worse one; `mxm-creative-sampler-dsp` had already got this right.
    pub fn reset(&mut self) {
        self.phasor.reset();
        self.rng = Rng::new(SEED);
        self.held = self.rng.next_bipolar();
    }

    /// Advance one sample and return the current value in `[-1, 1]`.
    #[inline]
    pub fn process(&mut self, rate_hz: f32, shape: LfoShape, sample_rate: f32) -> f32 {
        // Set the increment directly: `Phasor::set_freq` clamps to a musical
        // minimum of 8 Hz, which would be far too fast for an LFO.
        self.phasor
            .set_inc((rate_hz.max(0.0) / sample_rate).min(0.45));

        let p = self.phasor.phase();
        let value = match shape {
            LfoShape::Triangle => {
                if p < 0.25 {
                    4.0 * p
                } else if p < 0.75 {
                    2.0 - 4.0 * p
                } else {
                    4.0 * p - 4.0
                }
            }
            LfoShape::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoShape::SawUp => 2.0 * p - 1.0,
            LfoShape::SawDown => 1.0 - 2.0 * p,
            LfoShape::Random => self.held,
        };

        let before = self.phasor.phase();
        self.phasor.advance();
        // A wrap means the phase went down instead of up.
        if self.phasor.phase() < before {
            self.held = self.rng.next_bipolar();
        }

        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    fn collect(shape: LfoShape, rate: f32, n: usize) -> Vec<f32> {
        let mut lfo = Lfo::new();
        (0..n).map(|_| lfo.process(rate, shape, FS)).collect()
    }

    #[test]
    fn every_shape_stays_within_the_bipolar_range() {
        for shape in [
            LfoShape::Triangle,
            LfoShape::Square,
            LfoShape::SawUp,
            LfoShape::SawDown,
            LfoShape::Random,
        ] {
            for rate in [0.05f32, 1.0, 4.0, 30.0] {
                for v in collect(shape, rate, 20_000) {
                    assert!(
                        (-1.0..=1.0).contains(&v),
                        "{shape:?} at {rate} Hz produced {v}"
                    );
                    assert!(v.is_finite());
                }
            }
        }
    }

    #[test]
    fn triangle_starts_at_zero_and_rises() {
        let v = collect(LfoShape::Triangle, 1.0, 10);
        assert!(v[0].abs() < 1e-6, "should start at zero, got {}", v[0]);
        assert!(v[1] > v[0], "should be rising");
    }

    #[test]
    fn square_starts_high_and_is_symmetric() {
        let rate = 10.0;
        let n = (FS / rate) as usize;
        let v = collect(LfoShape::Square, rate, n);
        assert_eq!(v[0], 1.0, "square should start high");
        let high = v.iter().filter(|x| **x > 0.0).count();
        let ratio = high as f32 / n as f32;
        assert!((ratio - 0.5).abs() < 0.01, "duty cycle {ratio}");
    }

    #[test]
    fn ramps_run_in_opposite_directions() {
        let up = collect(LfoShape::SawUp, 1.0, 100);
        let down = collect(LfoShape::SawDown, 1.0, 100);
        assert!(up[0] < up[99], "SawUp should rise");
        assert!(down[0] > down[99], "SawDown should fall");
        for (a, b) in up.iter().zip(down.iter()) {
            assert!((a + b).abs() < 1e-6, "shapes should mirror: {a} vs {b}");
        }
    }

    #[test]
    fn random_holds_its_value_between_wraps() {
        let rate = 10.0;
        let period = (FS / rate) as usize;
        let v = collect(LfoShape::Random, rate, period * 3);

        // Within a cycle the value must not change.
        let mid = &v[period + 10..period * 2 - 10];
        assert!(
            mid.windows(2).all(|w| w[0] == w[1]),
            "sample-and-hold changed mid-cycle"
        );
        // Across cycles it must.
        assert_ne!(v[period + 10], v[period * 2 + 10], "value never changed");
    }

    #[test]
    fn random_holds_a_value_from_the_first_sample() {
        // It used to sit at exactly zero until the first wrap. At this LFO's 0.05 Hz floor that
        // is twenty seconds of a connected route doing nothing — the same omission found in
        // `mxm-drum-machine-dsp` on 2026-09-22.
        let mut lfo = Lfo::new();
        let first = lfo.process(0.05, LfoShape::Random, FS);
        assert!(
            first != 0.0,
            "Random began at exact zero, so a slow LFO modulates nothing until it wraps"
        );
        assert!(
            (-1.0..=1.0).contains(&first),
            "{first} left the bipolar range"
        );

        // And a reset puts it back to the same first value rather than to silence.
        lfo.reset();
        assert_eq!(lfo.process(0.05, LfoShape::Random, FS), first);
    }

    #[test]
    fn random_is_bit_repeatable_after_reset() {
        let mut lfo = Lfo::new();
        let first: Vec<f32> = (0..50_000)
            .map(|_| lfo.process(10.0, LfoShape::Random, FS))
            .collect();
        lfo.reset();
        let second: Vec<f32> = (0..50_000)
            .map(|_| lfo.process(10.0, LfoShape::Random, FS))
            .collect();
        assert_eq!(first, second, "sample-and-hold must be deterministic");
    }

    #[test]
    fn rate_is_accurate() {
        for rate in [0.5f32, 4.0, 30.0] {
            let secs = 4.0;
            let mut lfo = Lfo::new();
            let n = (FS * secs) as usize;
            let (mut crossings, mut prev) = (0usize, 0.0f32);
            for _ in 0..n {
                let v = lfo.process(rate, LfoShape::SawUp, FS);
                if prev <= 0.0 && v > 0.0 {
                    crossings += 1;
                }
                prev = v;
            }
            let measured = crossings as f32 / secs;
            assert!(
                (measured - rate).abs() / rate < 0.02,
                "rate {rate}: measured {measured}"
            );
        }
    }

    #[test]
    fn a_zero_rate_holds_still_rather_than_misbehaving() {
        let v = collect(LfoShape::Triangle, 0.0, 1_000);
        assert!(v.iter().all(|x| x.abs() < 1e-6), "zero rate should hold");
    }
}
