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
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use mxm_mono_01_dsp::envelope::Stage;

/// Everything the editor may read from the audio thread.
#[derive(Debug)]
pub struct Telemetry {
    /// Envelope level, `0..=1`, as f32 bits.
    env_level: AtomicU32,
    /// Envelope segment, as [`Stage`] discriminant.
    env_stage: AtomicU8,
    /// Peak `abs(sample)` since the UI last looked, as f32 bits.
    peak: AtomicU32,
    /// Latches on clip and stays latched until acknowledged.
    clipped: AtomicBool,
    /// Published once per `activate`, so the editor can plot the filter at the right rate.
    sample_rate: AtomicU32,
    /// The developer channel's requests of the editor: a view to show, and whether the expander
    /// is open. `NO_REQUEST` when nothing is asked. See `plugins/AGENTS.md`.
    dev_view: AtomicU8,
    dev_disclosure: AtomicU8,
    /// The developer channel's request to open or close the preset browser, or `NO_REQUEST`.
    dev_browser: AtomicU8,
    /// The developer channel's request to show a theme, by index, or `NO_REQUEST`. Theme is
    /// interface state, so this reaches the editor and nothing else; the DSP never sees it.
    dev_theme: AtomicU8,
    /// The host tempo in force, so a synced LFO rate reads its division.
    pub tempo: mxm_tempo::TempoCell,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            env_level: AtomicU32::new(0),
            env_stage: AtomicU8::new(stage_code(Stage::Idle)),
            peak: AtomicU32::new(0),
            clipped: AtomicBool::new(false),
            // A plausible default rather than zero: an editor that opened before `activate` would
            // otherwise divide by it while drawing the filter curve.
            sample_rate: AtomicU32::new(48_000f32.to_bits()),
            dev_view: AtomicU8::new(u8::MAX),
            dev_disclosure: AtomicU8::new(u8::MAX),
            dev_browser: AtomicU8::new(u8::MAX),
            dev_theme: AtomicU8::new(u8::MAX),
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

    /// Folds this block's peak in, and latches clip.
    ///
    /// # Max-combine, not overwrite
    ///
    /// The UI reads at frame rate and the audio thread writes at block rate, so most blocks are
    /// never seen. Overwriting would make a transient visible only if it happened to land in the
    /// block before a repaint — a meter that misses exactly the peaks it exists to show. Combining
    /// with `max` means the value the UI eventually reads is the loudest since it last looked.
    ///
    /// The loop is a compare-exchange rather than a `fetch_max` because the values are f32 bits,
    /// and integer `max` on the bit patterns is not float `max`. It is uncontended in practice —
    /// one writer, and the reader only ever stores zero — so it does not spin.
    pub fn publish_peak(&self, peak: f32) {
        if !peak.is_finite() || peak <= 0.0 {
            return;
        }

        if peak >= 1.0 {
            self.clipped.store(true, Ordering::Relaxed);
        }

        let mut current = self.peak.load(Ordering::Relaxed);
        loop {
            if f32::from_bits(current) >= peak {
                return;
            }
            match self.peak.compare_exchange_weak(
                current,
                peak.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(seen) => current = seen,
            }
        }
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

    /// The loudest sample since the last call, and resets the accumulator.
    ///
    /// Reset-on-read is what makes the meter mean *"loudest since you last looked"* rather than
    /// *"loudest ever"*, which would rise once and never fall.
    #[must_use]
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }

    /// Whether the output has clipped since the last acknowledgement.
    ///
    /// Design system §5.4: *"Clip indication remains visible until acknowledged."* So this does
    /// **not** reset on read — [`Telemetry::acknowledge_clip`] is the only way to clear it, and
    /// that is driven by the user.
    #[must_use]
    pub fn clipped(&self) -> bool {
        self.clipped.load(Ordering::Relaxed)
    }

    pub fn acknowledge_clip(&self) {
        self.clipped.store(false, Ordering::Relaxed);
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
        assert_eq!(telemetry.take_peak(), 0.0);
        assert!(!telemetry.clipped());
        assert!(telemetry.sample_rate() > 0.0);
    }

    #[test]
    fn the_peak_survives_frames_the_ui_never_read() {
        // The defect this prevents: a transient in a block that is not the last one before a
        // repaint. Overwriting loses it; max-combining does not.
        let telemetry = Telemetry::default();
        telemetry.publish_peak(0.2);
        telemetry.publish_peak(0.9);
        telemetry.publish_peak(0.3);
        assert!((telemetry.take_peak() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn reading_the_peak_resets_it() {
        let telemetry = Telemetry::default();
        telemetry.publish_peak(0.7);
        assert!((telemetry.take_peak() - 0.7).abs() < 1e-6);
        assert_eq!(telemetry.take_peak(), 0.0, "the meter would never fall");
    }

    #[test]
    fn clip_latches_until_a_person_clears_it() {
        // §5.4. A clip that clears itself tells you nothing a moment later, which is exactly when
        // you look up at the meter.
        let telemetry = Telemetry::default();
        telemetry.publish_peak(1.5);
        assert!(telemetry.clipped());

        let _ = telemetry.take_peak();
        telemetry.publish_peak(0.1);
        assert!(telemetry.clipped(), "quiet audio must not clear a clip");

        telemetry.acknowledge_clip();
        assert!(!telemetry.clipped());
    }

    #[test]
    fn a_non_finite_peak_is_ignored() {
        // A NaN reaching the meter would poison the max-combine forever, because every comparison
        // against NaN is false. DSP bugs happen; the display should not become the second victim.
        let telemetry = Telemetry::default();
        telemetry.publish_peak(0.5);
        telemetry.publish_peak(f32::NAN);
        telemetry.publish_peak(f32::INFINITY);
        assert!((telemetry.take_peak() - 0.5).abs() < 1e-6);
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

/// Nothing requested on a developer-channel slot.
const NO_REQUEST: u8 = u8::MAX;

/// The developer channel's requests of the editor — `plugins/AGENTS.md`, *A developer channel in
/// every editor*. Each is taken once; the DSP reads nothing.
impl Telemetry {
    /// Developer category address (0–5), or Parameters (127); never a derived tab index.
    pub fn request_view(&self, view: u8) {
        self.dev_view
            .store(view.min(NO_REQUEST - 1), Ordering::Relaxed);
    }

    /// The developer channel asks the editor to open or close the preset browser.
    pub fn request_browser(&self, open: bool) {
        self.dev_browser.store(u8::from(open), Ordering::Relaxed);
    }

    /// Whether the developer channel asked the browser open or closed since the editor last
    /// looked, if it did.
    pub fn take_browser_request(&self) -> Option<bool> {
        match self.dev_browser.swap(NO_REQUEST, Ordering::Relaxed) {
            NO_REQUEST => None,
            open => Some(open != 0),
        }
    }

    /// The developer channel asks the editor for a theme, by index — 0 light, 1 dark, 2 system,
    /// as `mxm_ui::theme::from_index` reads it.
    pub fn request_theme(&self, theme: u8) {
        self.dev_theme
            .store(theme.min(NO_REQUEST - 1), Ordering::Relaxed);
    }

    /// The theme the developer channel asked for since the editor last looked, if any.
    pub fn take_theme_request(&self) -> Option<u8> {
        match self.dev_theme.swap(NO_REQUEST, Ordering::Relaxed) {
            NO_REQUEST => None,
            theme => Some(theme),
        }
    }

    /// The developer channel asks the editor to open or close its expander.
    pub fn request_disclosure(&self, open: bool) {
        self.dev_disclosure.store(u8::from(open), Ordering::Relaxed);
    }

    /// The view the developer channel asked for since the editor last looked, if any.
    pub fn take_view_request(&self) -> Option<usize> {
        match self.dev_view.swap(NO_REQUEST, Ordering::Relaxed) {
            NO_REQUEST => None,
            view => Some(usize::from(view)),
        }
    }

    /// Whether the developer channel asked the expander open or closed since the editor last
    /// looked, if it did.
    pub fn take_disclosure_request(&self) -> Option<bool> {
        match self.dev_disclosure.swap(NO_REQUEST, Ordering::Relaxed) {
            NO_REQUEST => None,
            open => Some(open != 0),
        }
    }
}

#[cfg(test)]
mod developer_channel_tests {
    use super::*;

    #[test]
    fn a_developer_request_is_taken_once() {
        let t = Telemetry::shared();
        assert_eq!(
            t.take_view_request(),
            None,
            "nothing asked on a fresh instance"
        );
        t.request_view(1);
        assert_eq!(t.take_view_request(), Some(1));
        assert_eq!(t.take_view_request(), None, "and taking it clears it");
        t.request_disclosure(true);
        assert_eq!(t.take_disclosure_request(), Some(true));
        t.request_browser(true);
        assert_eq!(t.take_browser_request(), Some(true));
        assert_eq!(t.take_browser_request(), None, "taken once");
        assert_eq!(t.take_disclosure_request(), None);
    }
}
