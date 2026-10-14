//! DSP for mxm-mono-01.
//!
//! Deliberately free of any plugin-framework types: everything here takes plain
//! values and a sample rate, so the whole voice is testable with `cargo test` and
//! no host involved.

#[cfg(any(test, feature = "conformance"))]
pub mod conformance;
pub mod envelope;
pub mod filter;
pub mod lfo;
pub mod oscillator;
pub mod routing;
pub mod voice;

/// The lowest host rate the plugin activates at; a non-finite rate is refused with it.
///
/// `f32::clamp` panics when its lower bound is above its upper one or either is NaN, and the
/// ladder's cutoff is clamped to `20 Hz ..= 0.45 × rate`, which crosses below 44.4 Hz. 1 kHz is far
/// clear of that, and no higher than the lowest rate clap-validator (1234.57 Hz) or the player's
/// robustness sweeps (1 kHz) ask for.
pub const MIN_SAMPLE_RATE: f32 = 1_000.0;
