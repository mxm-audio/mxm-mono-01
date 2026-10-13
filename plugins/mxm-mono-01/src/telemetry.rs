//! DSP → editor telemetry, for the brief's §8 visualizations.
//!
//! # The contract, and why it is a struct rather than a convention
//!
//! mxm-kit's `docs/plugin-conventions.md` (linked from `plugins/AGENTS.md`) states it: *"DSP →
//! editor telemetry uses atomics or a triple buffer, never a mutex read by the audio thread, and
//! may drop frames."* A rule written only in prose is a rule each new visualization re-derives, so
//! it is expressed here as the only channel that exists.
//!
//! One [`Telemetry`], `Arc`-shared: the plugin owns it, the editor holds a clone. **The audio
//! thread writes; the UI thread reads.** No locks, no allocation, and nothing here is on a path
//! the audio callback can block on.
//!
//! # Why this lives in the plugin and not in `mxm-mono-01-dsp`
//!
//! The DSP crate is framework-free, zero-dependency and MSRV 1.87, and it stays that way. It
//! contributes the two read-only accessors — `Voice::env_level` and `Voice::env_stage` — and knows
//! nothing about who reads them or how they travel.
//!
//! # What each visualization actually needs
//!
//! The brief's three are not alike, and finding that out was worth the trouble:
//!
//! - **Envelope position** genuinely needs telemetry. The value exists for the length of one
//!   `process` call and is otherwise unreachable.
//! - **Output level and clip** need telemetry too, and this is easy to miss because the samples
//!   look available — they are in the buffer. But the buffer belongs to the host and is gone by
//!   the time the editor paints, so the peak has to be taken while it is there.
//! - **The filter response curve** needs none. It is a function of `cutoff`, `resonance` and the
//!   sample rate, all of which the editor can read for itself. The sample rate travels here only
//!   because the editor has no other way to learn it.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use mxm_mono_01_dsp::envelope::Stage;

/// Everything the editor may read from the audio thread.
#[derive(Debug)]
pub struct Telemetry {
    /// The output level and its clip latch: the collection's one meter policy
    /// (`mxm_telemetry::PeakMeter`).
    pub meter: mxm_telemetry::PeakMeter,
    /// The developer channel's requests of the editor, each taken once
    /// (`mxm_telemetry::DevRequests`).
    pub dev: mxm_telemetry::DevRequests,
    /// Envelope level, `0..=1`, as f32 bits.
    env_level: AtomicU32,
    /// Envelope segment, as [`Stage`] discriminant.
    env_stage: AtomicU8,
    /// Published once per `activate`, so the editor can plot the filter at the right rate.
    sample_rate: AtomicU32,
    /// The host tempo in force, so a synced LFO rate reads its division.
    pub tempo: mxm_tempo::TempoCell,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            meter: mxm_telemetry::PeakMeter::new(),
            dev: mxm_telemetry::DevRequests::new(),
            env_level: AtomicU32::new(0),
            env_stage: AtomicU8::new(stage_code(Stage::Idle)),
            // A plausible default rather than zero: an editor that opened before `activate` would
            // otherwise divide by it while drawing the filter curve.
            sample_rate: AtomicU32::new(48_000f32.to_bits()),
            tempo: mxm_tempo::TempoCell::new(),
        }
    }
}

impl Telemetry {
    #[must_use]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    // -------------------------------------------------------------------------------------
    // Audio thread
    // -------------------------------------------------------------------------------------

    /// Publishes the envelope's state. Called once per block from `process`.
    ///
    /// `Relaxed` throughout: these are independent scalars for a display that is allowed to drop
    /// frames, and nothing else is ordered against them. Paying for `Release` here would buy an
    /// ordering guarantee no reader needs.
    pub fn publish_envelope(&self, level: f32, stage: Stage) {
        self.env_level.store(level.to_bits(), Ordering::Relaxed);
        self.env_stage.store(stage_code(stage), Ordering::Relaxed);
    }

    /// Publishes the sample rate. Called from `activate`, not from the audio callback.
    pub fn publish_sample_rate(&self, sample_rate: f32) {
        self.sample_rate
            .store(sample_rate.to_bits(), Ordering::Relaxed);
    }

    // -------------------------------------------------------------------------------------
    // UI thread
    // -------------------------------------------------------------------------------------

    #[must_use]
    pub fn envelope(&self) -> (f32, Stage) {
        (
            f32::from_bits(self.env_level.load(Ordering::Relaxed)),
            stage_from_code(self.env_stage.load(Ordering::Relaxed)),
        )
    }

    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate.load(Ordering::Relaxed))
    }
}

/// [`Stage`] has no stable numeric representation of its own, so the mapping is written out.
/// A `as` cast would silently change meaning if a variant were ever inserted.
const fn stage_code(stage: Stage) -> u8 {
    match stage {
        Stage::Idle => 0,
        Stage::Attack => 1,
        Stage::Decay => 2,
        Stage::Sustain => 3,
        Stage::Release => 4,
    }
}

const fn stage_from_code(code: u8) -> Stage {
    match code {
        1 => Stage::Attack,
        2 => Stage::Decay,
        3 => Stage::Sustain,
        4 => Stage::Release,
        // Includes 0. An unknown code can only mean a mapping bug, and showing an idle envelope
        // is the harmless reading.
        _ => Stage::Idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_telemetry_is_safe_to_draw_from() {
        // The editor can open before `activate` runs. Every value has to be plottable, and the
        // sample rate in particular is a divisor in the filter curve.
        let telemetry = Telemetry::default();
        assert_eq!(telemetry.envelope(), (0.0, Stage::Idle));
        assert_eq!(telemetry.meter.take(), 0.0);
        assert!(!telemetry.meter.clipped());
        assert!(telemetry.sample_rate() > 0.0);
    }

    #[test]
    fn every_stage_survives_the_round_trip() {
        for stage in [
            Stage::Idle,
            Stage::Attack,
            Stage::Decay,
            Stage::Sustain,
            Stage::Release,
        ] {
            assert_eq!(stage_from_code(stage_code(stage)), stage);
        }
    }

    #[test]
    fn an_unknown_stage_code_reads_as_idle() {
        assert_eq!(stage_from_code(200), Stage::Idle);
    }

    #[test]
    fn the_envelope_round_trips_through_its_atomics() {
        let telemetry = Telemetry::default();
        telemetry.publish_envelope(0.625, Stage::Decay);
        assert_eq!(telemetry.envelope(), (0.625, Stage::Decay));
    }
}
