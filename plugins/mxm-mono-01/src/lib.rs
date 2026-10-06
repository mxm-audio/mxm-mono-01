//! mxm-mono-01 — monophonic subtractive synthesizer.
//!
//! Architecture inspired by the Roland SH-101: one oscillator with sub and noise,
//! a four-source mixer, a 4-pole resonant lowpass, one envelope shared by filter
//! and amplifier, and one LFO. Not affiliated with or endorsed by Roland.
//!
//! This file is the plugin shell: identity, parameter plumbing, and MIDI. All the
//! signal processing lives in `mxm-mono-01-dsp`, which knows nothing about nice-plug.

/// The plugin's name, and the **only** place it is written in this crate.
///
/// Everything else that names the instrument derives from here: [`NAME`], which the host shows, and
/// [`CLAP_ID`], which it remembers. A rename is this line.
///
/// A macro rather than a `const` because [`CLAP_ID`] is built with `concat!`, which takes literals.
macro_rules! plugin_name {
    () => {
        "mxm-mono-01"
    };
}

/// What the host displays.
pub const NAME: &str = plugin_name!();

/// The permanent CLAP identifier.
///
/// Named here rather than only inside the `Plugin` impl because `preset` has to write it into every
/// file and refuse a preset that carries another one — and a second literal is a second thing that
/// can be edited alone.
///
/// **Deliberately assembled from [`plugin_name!`] and not from `CARGO_PKG_NAME`, and that is not an
/// oversight.** Deriving it from the package name would mean a future `git mv` of this directory
/// silently changed the plugin's permanent identity — no compile error, no failing test, and every
/// preset and saved project written under the old id orphaned. `plugins/AGENTS.md` calls this
/// identifier permanent; renaming a *directory* must not be able to move it. Renaming the plugin is
/// a deliberate act that edits `plugin_name!` above, which is exactly one line and exactly one
/// decision.
pub const CLAP_ID: &str = concat!("dk.mxm.", plugin_name!());

// Public for `apps/mxm-layout-lab` (in the private archive since the split) on the
// `dynamic-layout` branch: the lab draws these real cards outside a host. Nothing else about them
// changes, and the shipped cdylib is unaffected.
pub mod editor;
pub mod params;
pub mod preset;
pub mod routes;
pub mod telemetry;

use mxm_mono_01_dsp::oscillator::MixLevels;
use mxm_mono_01_dsp::voice::{NoteId, Patch, Retrigger, Voice};
use nice_plug::midi::{Channel, Key, VoiceID};
use nice_plug::prelude::*;
use params::MxmMono01Params;
use std::sync::Arc;

/// A note's identity in the shape the voice logic was written for. nice-plug 0.4 types it
/// (`VoiceID`, `Channel`, `Key`, each with a wildcard); 0.3 handed over a host's wildcard (-1) as
/// 255 and a missing voice id as `None`. Converting here keeps every note decision, and every
/// recorded render, exactly what it was before the upgrade.
fn legacy_note(voice_id: VoiceID, channel: Channel, key: Key) -> (Option<i32>, u8, u8) {
    (
        voice_id.id(),
        channel.number().unwrap_or(u8::MAX),
        key.number().unwrap_or(u8::MAX),
    )
}

/// Upper bound on how many samples are rendered between event checks.
///
/// Splitting only on events is not enough: a buffer containing no MIDI at all
/// would otherwise become one arbitrarily long block, and per-sample modulation
/// would be the only thing keeping it honest.
const MAX_BLOCK_SIZE: usize = 64;

/// MIDI channels, for the per-channel bend and mod wheel state.
const NUM_CHANNELS: usize = 16;

/// **The collection's developer channel, off unless asked for** (`plugins/AGENTS.md`). With
/// `MXM_DEV_CC` set in the plugin's process environment when an instance is made, CC 119 selects the
/// category (0–5) or Parameters (127); CC 118 opens (≥ 64) or closes its expander, through telemetry
/// atomics; the DSP reads nothing. It exists so a script, a screenshot run or an AI can put the
/// editor in a state CLAP gives a host no way to ask for — through the player, `mxm-cli cc 119 1`.
const DEV_VIEW_CC: u8 = 119;
const DEV_DISCLOSURE_CC: u8 = 118;
const DEV_BROWSER_CC: u8 = 117;
const DEV_THEME_CC: u8 = 116;
const DEV_CC_ENV: &str = "MXM_DEV_CC";

pub struct MxmMono01 {
    params: Arc<MxmMono01Params>,
    voice: Voice,
    /// Which routes are live, resolved **once per processing interval** rather than per sample:
    /// topology is discrete and changes only on a parameter event.
    routing: mxm_mono_01_dsp::routing::Routing,
    /// The velocity of the press that last triggered the envelope, as a routable source — full
    /// before any press, so the standard Velocity rests at zero.
    velocity: f32,
    /// Channel pressure, as a routable source. Per channel, like bend and the wheel.
    pressure: [f32; NUM_CHANNELS],

    /// Pitch bend per channel, in `-1..=1`. CLAP delivers `0..=1` with 0.5 centred.
    bend: [f32; NUM_CHANNELS],
    /// Mod wheel (CC 1) per channel, `0..=1`.
    mod_wheel: [f32; NUM_CHANNELS],
    /// Channel of the note currently sounding, so the right bend applies.
    active_channel: u8,
    /// Per-note pitch expression for the sounding note, in semitones.
    ///
    /// One voice, so one value — and the note it belongs to, because **the sounding note can
    /// change without a note-on**: releasing the newest key under a chord returns the voice to an
    /// older held one, and the bend drawn against the key that left must not follow the voice onto
    /// the key that stays. `Voice::sounding` is what says which note that is; this pair is what
    /// notices it moved.
    expression_semitones: f32,
    expression_note: Option<NoteId>,

    sample_rate: f32,

    /// DSP -> editor, atomics only. See [`telemetry`] for the contract; the short version is that
    /// the audio thread writes it once per block and the editor may drop as many frames as it
    /// likes. Held here even with no editor open, because `activate` publishes the sample rate
    /// and the cost of a few atomic stores per block is not worth branching on.
    telemetry: Arc<telemetry::Telemetry>,
    /// Whether the developer channel is on: `DEV_CC_ENV` was set when this instance was made.
    dev_cc: bool,
    /// The LFO rate as its sync resolved it for this buffer, or `None` for the free rate.
    synced_lfo_hz: Option<f32>,
}

impl Default for MxmMono01 {
    fn default() -> Self {
        Self {
            params: Arc::new(MxmMono01Params::default()),
            voice: Voice::new(),
            bend: [0.0; NUM_CHANNELS],
            mod_wheel: [0.0; NUM_CHANNELS],
            routing: mxm_mono_01_dsp::routing::Routing::new(),
            velocity: 1.0,
            pressure: [0.0; NUM_CHANNELS],
            active_channel: 0,
            expression_semitones: 0.0,
            expression_note: None,
            sample_rate: 48_000.0,
            telemetry: telemetry::Telemetry::shared(),
            dev_cc: std::env::var_os(DEV_CC_ENV).is_some(),
            synced_lfo_hz: None,
        }
    }
}

impl MxmMono01 {
    /// Build one sample's worth of plain values from the parameter smoothers.
    ///
    /// Called per sample, which is what makes envelope and LFO modulation of the
    /// cutoff actually sample-accurate. Every smoother must be advanced exactly
    /// once per sample, so this is the only place they are read.
    #[inline]
    /// Drops the pitch expression when the voice has moved to a **different** note.
    ///
    /// Called after every event that can change which key is sounding without a note-on. A bend
    /// drawn against one note is that note's; when priority hands the voice to another held key,
    /// the offset does not come along.
    ///
    /// **Going silent is not moving.** With the last key released `sounding` is `None`, and the
    /// offset is kept: the note is releasing at the pitch it was bent to, and zeroing it here
    /// would snap the pitch back in the middle of the tail.
    fn drop_expression_if_the_voice_moved(&mut self) {
        if let Some(now) = self.voice.sounding()
            && self.expression_note != Some(now)
        {
            self.expression_semitones = 0.0;
            self.expression_note = None;
        }
    }

    /// Renders one block through the plugin's own per-sample path, for measurement.
    ///
    /// **A measurement seam, not a second `process()`.** It runs exactly the loop `process()` runs —
    /// `next_patch()` per sample, which advances every smoother and rebuilds the `Patch`, then
    /// `Voice::process` — and it deliberately omits the wrapper's per-block event handling and
    /// buffer plumbing, which are not what the modulation work lands on. `mxm-shimmer`,
    /// `mxm-grain-fx` and `mxm-bucket-delay` carry the same `_for_test` shape for the same reason.
    ///
    /// It exists because `plans/plan-modulation-routing.md`'s cost gate must measure the **plugin**
    /// path rather than the voice's arithmetic: the smoothers and the per-sample `Patch` rebuild are
    /// where the routing work will land, and a framework-free DSP bench cannot reach them.
    pub fn render_block_for_test(&mut self, out: &mut [f32]) {
        // Topology once per block, exactly as `process()` does it.
        self.routing = self.params.routes.topology_from(&self.routing);
        self.voice.set_topology(&self.routing);
        let routed = self.routing.present.iter().flatten().any(|&p| p);
        for sample in out.iter_mut() {
            let patch = self.next_patch();
            if routed {
                self.params.routes.advance(
                    &mut self.routing,
                    self.mod_wheel[self.active_channel as usize],
                );
            }
            *sample = self.voice.process(&patch, &self.routing);
        }
    }

    fn next_patch(&self) -> Patch {
        let p = &self.params;
        let channel = self.active_channel as usize;

        Patch {
            velocity: self.velocity,
            wheel: self.mod_wheel[channel],
            pressure: self.pressure[channel],
            range_offset: p.osc_range.value().semitones(),
            tune_cents: p.tune.smoothed.next(),
            glide_time_s: p.glide.value(),
            bend_semitones: self.bend[channel] * p.bend_range.smoothed.next(),
            bend: self.bend[channel],
            expression_semitones: self.expression_semitones,

            levels: MixLevels {
                saw: p.saw_level.smoothed.next(),
                pulse: p.pulse_level.smoothed.next(),
                sub: p.sub_level.smoothed.next(),
                noise: p.noise_level.smoothed.next(),
            },
            pulse_width: p.pulse_width.smoothed.next(),
            sub_shape: p.sub_type.value().into(),

            cutoff_hz: p.cutoff.smoothed.next(),
            resonance: p.resonance.smoothed.next(),

            attack_s: p.attack.value(),
            decay_s: p.decay.value(),
            sustain: p.sustain.smoothed.next(),
            release_s: p.release.value(),

            // Synced, the LFO runs at its division; the free smoother is advanced as it is with sync off,
            // so turning sync off lands on the knob rather than an old rate.
            lfo_rate_hz: {
                let free = p.lfo_rate.smoothed.next();
                self.synced_lfo_hz.unwrap_or(free)
            },
            lfo_shape: p.lfo_wave.value().into(),

            vca_source: p.vca_source.value().into(),
            output_gain: p.output_gain.smoothed.next(),
        }
    }

    fn handle_event(&mut self, event: NoteEvent<()>) {
        let retrigger = self.params.retrigger.value().into();

        match event {
            NoteEvent::NoteOn {
                voice_id,
                channel,
                key,
                velocity,
                ..
            } => {
                let (voice_id, channel, note) = legacy_note(voice_id, channel, key);
                // Velocity zero is a note-off by convention, and the SH-101 is not
                // velocity sensitive, so velocity is otherwise ignored.
                if velocity <= 0.0 {
                    self.voice.note_off(voice_id, channel, note, retrigger);
                } else {
                    // A host's wildcard channel (CLAP's -1) arrives as 255 (`legacy_note`), and the bend,
                    // wheel and pressure tables have 16 entries: wrap it, as the pitch-bend path does, so a
                    // wildcard reads channel 15's instead of panicking. Channels 0 to 15 are unchanged.
                    self.active_channel = (usize::from(channel) % NUM_CHANNELS) as u8;
                    // **Velocity becomes a source here and nowhere else.** The machine itself is
                    // not velocity sensitive and stays that way: nothing reads this unless a player
                    // routes it, which is decision 1.7's *expand the original* rather than a change
                    // to what the original does. It is the velocity of **the press that last
                    // triggered the envelope** (the modulation standard): a legato joint, which
                    // retriggers nothing, keeps the phrase's.
                    let triggers =
                        retrigger == Retrigger::Always || self.voice.sounding().is_none();
                    if triggers {
                        self.velocity = velocity;
                    }
                    // A new note carries no expression until the host sends one.
                    self.expression_semitones = 0.0;
                    self.expression_note = None;
                    self.voice.note_on(
                        NoteId {
                            voice_id,
                            channel,
                            note,
                        },
                        retrigger,
                    );
                }
            }
            NoteEvent::NoteOff {
                voice_id,
                channel,
                key,
                ..
            } => {
                let (voice_id, channel, note) = legacy_note(voice_id, channel, key);
                self.voice.note_off(voice_id, channel, note, retrigger);
                self.drop_expression_if_the_voice_moved();
            }

            NoteEvent::Choke {
                voice_id,
                channel,
                key,
                ..
            } => {
                let (voice_id, channel, note) = legacy_note(voice_id, channel, key);
                self.voice.choke(voice_id, channel, note);
                self.drop_expression_if_the_voice_moved();
            }

            // **Per-note pitch, from the host's piano roll.** CLAP's tuning expression, already
            // in semitones, applied only while it names the note the voice is actually sounding —
            // which the note stack decides, since a note-off can hand the voice back to an older
            // held key. An expression for any other note belongs to no gate here.
            NoteEvent::PolyTuning {
                voice_id,
                channel,
                key,
                tuning,
                ..
            } => {
                let (voice_id, channel, note) = legacy_note(voice_id, channel, key);
                // A non-finite tuning is dropped, and the note keeps the offset it had: a NaN in the
                // pitch sum would reach the oscillator's phase and never leave.
                if let Some(id) = self.voice.sounding()
                    && id.matches(voice_id, channel, note)
                    && tuning.is_finite()
                {
                    self.expression_semitones = tuning;
                    self.expression_note = Some(id);
                }
            }

            NoteEvent::MidiPitchBend { channel, value, .. } => {
                self.bend[channel as usize % NUM_CHANNELS] = 2.0 * (value - 0.5);
            }

            // **Pressure becomes a source here and nowhere else**, on the same footing as velocity:
            // the machine has no aftertouch of its own and gains none, but a player may route what
            // their keyboard sends.
            NoteEvent::MidiChannelPressure {
                channel, pressure, ..
            } => {
                self.pressure[channel as usize % NUM_CHANNELS] = pressure;
            }

            NoteEvent::MidiCC {
                channel, cc, value, ..
            } => match cc {
                // The collection's developer channel, only when this instance was started with it.
                DEV_VIEW_CC if self.dev_cc => self
                    .telemetry
                    .request_view((value.clamp(0.0, 1.0) * 127.0).round() as u8),
                DEV_DISCLOSURE_CC if self.dev_cc => {
                    self.telemetry.request_disclosure(value >= 0.5);
                }
                DEV_BROWSER_CC if self.dev_cc => {
                    self.telemetry.request_browser(value >= 0.5);
                }
                // A theme by index, 0 light / 1 dark / 2 system, as the app bar's control lists
                // them. Applied to the editor and never saved: a capture run must not rewrite the
                // choice the person at the machine made.
                DEV_THEME_CC if self.dev_cc => {
                    self.telemetry
                        .request_theme((value.clamp(0.0, 1.0) * 127.0).round() as u8);
                }
                control_change::MODULATION_MSB => {
                    self.mod_wheel[channel as usize % NUM_CHANNELS] = value;
                }
                // All sound off: immediate, no release.
                control_change::ALL_SOUND_OFF => self.voice.all_sound_off(),
                // All notes off: deliberately different, notes release normally.
                control_change::ALL_NOTES_OFF => self.voice.all_notes_off(),
                // Sustain pedal is intentionally unsupported: the SH-101 has none.
                _ => {}
            },

            _ => {}
        }
    }
}

impl Plugin for MxmMono01 {
    const NAME: &'static str = crate::NAME;
    const VENDOR: &'static str = "mxm";
    const URL: &'static str = "https://mxm.dk";
    const EMAIL: &'static str = "plugins@mxm.dk";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    /// An instrument: no main input. Stereo output is dual mono — the voice is
    /// monophonic and there is no widening.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    /// `MidiCCs` rather than `Basic`: pitch bend, CC 1, CC 120 and CC 123 are all
    /// needed.
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;

    /// Smoothers advance per sample inside each event-delimited block, which
    /// already removes zipper noise; splitting a second time buys little here.
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type Editor = editor::MxmMono01Editor;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    /// The editor is transient: the host opens and closes it at will, and audio renders normally
    /// with none open. Both halves it needs are already `Arc`s, so this hands out clones rather
    /// than borrowing anything the audio thread owns.
    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::create(self.params.clone(), self.telemetry.clone())
    }

    fn activate(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        // A new activation starts with no tempo and nothing resolved: the first callback reports
        // the tempo, so neither the audio nor an editor frame before it shows the last session's
        // divisions (`plans/plan-tempo-sync-controls.md`).
        self.telemetry.tempo.publish(None);
        self.synced_lfo_hz = None;
        // A rate the DSP's clamps cannot hold is refused before anything changes: a NaN, or one
        // below `MIN_SAMPLE_RATE`, crosses a `clamp` bound and panics on the audio thread.
        if !buffer_config.sample_rate.is_finite()
            || buffer_config.sample_rate < mxm_mono_01_dsp::MIN_SAMPLE_RATE
        {
            return false;
        }
        self.sample_rate = buffer_config.sample_rate;
        self.voice.set_sample_rate(self.sample_rate);
        self.voice.reset();
        // Once, here, rather than per block: it changes only when the host reconfigures, and the
        // editor needs it to plot the filter response at the right rate.
        self.telemetry.publish_sample_rate(self.sample_rate);
        true
    }

    fn reset(&mut self) {
        self.voice.reset();
        self.bend = [0.0; NUM_CHANNELS];
        self.mod_wheel = [0.0; NUM_CHANNELS];
        // A reset forgets the phrase, so the Velocity source rests again.
        self.velocity = 1.0;
        self.active_channel = 0;
        self.expression_semitones = 0.0;
        self.expression_note = None;
    }

    /// **A project saved before the tempo syncs** restores each Off rather than keeping this
    /// instance's, and a loaded preset's baseline gains it, so the preset stays clean
    /// (`mxm_preset::add_switches_off`).
    fn filter_state(state: &mut PluginState) {
        mxm_preset::add_switches_off(state, crate::preset::TEMPO_SYNC_IDS);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let num_samples = buffer.samples();
        let mut next_event = context.next_event();
        let mut block_start = 0usize;

        // The LFO's tempo sync, once per buffer (`plans/plan-tempo-sync-controls.md`), and the tempo
        // in force for the editor's reading.
        let tempo = context.transport().tempo;
        self.synced_lfo_hz = self.params.synced_lfo_rate(tempo);
        self.telemetry.tempo.publish(tempo);

        // **Topology, once per buffer.** Which routes are live changes only on a parameter event,
        // and decision 1.9 puts a route's arrival at the processing interval rather than at an exact
        // sample — *moving a modulator can probably never be sample accurate; the modulation itself
        // is what must be*. So this is resolved here rather than inside the event-split loop, where
        // it would be redone for every sub-block to no audible end. A route that has just become
        // present has its smoother snapped to the depth it holds (`Routes::topology_from`), because
        // nothing advanced it while it was absent.
        self.routing = self.params.routes.topology_from(&self.routing);
        self.voice.set_topology(&self.routing);
        let routed = self.routing.present.iter().flatten().any(|&p| p);

        while block_start < num_samples {
            let mut block_end = (block_start + MAX_BLOCK_SIZE).min(num_samples);

            // Apply everything scheduled at or before this point, then shorten the
            // block so the next event lands exactly where it should.
            loop {
                match next_event {
                    Some(event) if (event.timing() as usize) <= block_start => {
                        self.handle_event(event);
                        next_event = context.next_event();
                    }
                    Some(event) if (event.timing() as usize) < block_end => {
                        block_end = event.timing() as usize;
                        break;
                    }
                    _ => break,
                }
            }

            // `block_end == block_start` can happen when several events share a
            // sample; the loop above will consume them on the next pass, so this
            // cannot spin forever.
            {
                let output = buffer.as_slice();
                // Accumulated across the block and published once, rather than an atomic store per
                // sample. `assert_process_allocs` would not catch that, but it is still the wrong
                // amount of traffic for a value a display reads at frame rate.
                let mut block_peak = 0.0f32;
                for i in block_start..block_end {
                    let patch = self.next_patch();
                    // The amounts for this sample, into the topology the interval already resolved.
                    // **In place and only where a route is live**: an absent pair's smoother is
                    // never advanced, so it costs nothing, and its stored depth is left exactly
                    // where the player put it.
                    if routed {
                        self.params.routes.advance(
                            &mut self.routing,
                            self.mod_wheel[self.active_channel as usize],
                        );
                    }
                    let sample = self.voice.process(&patch, &self.routing);
                    block_peak = block_peak.max(sample.abs());
                    for channel in output.iter_mut() {
                        channel[i] = sample;
                    }
                }
                self.telemetry.publish_peak(block_peak);
                self.telemetry
                    .publish_envelope(self.voice.env_level(), self.voice.env_stage());
            }

            block_start = block_end;
        }

        if self.voice.is_active() {
            ProcessStatus::Tail(self.voice.tail_samples(self.params.release.value()))
        } else {
            ProcessStatus::Normal
        }
    }
}

impl ClapPlugin for MxmMono01 {
    /// Permanent. Reverse DNS of a domain the project owns, so it survives moving
    /// between forges. Changing it breaks every saved project using the plugin.
    const CLAP_ID: &'static str = CLAP_ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some(
        "A one-oscillator monophonic synthesizer with a sub-oscillator and a 4-pole resonant filter",
    );
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
        ClapFeature::Mono,
    ];
}

nice_export_clap!(MxmMono01);

#[cfg(test)]
mod upstream_defects {
    use super::*;

    /// Pins the arithmetic behind the upstream nice-plug defect in mxm-kit's
    /// `docs/known-issues.md`.
    ///
    /// The recorded failure is nice-plug 0.3.0's: the wrapper's `input_events` was a
    /// `VecDeque::with_capacity(512)` that grew inside `process()`, and the allocation it failed
    /// on, 20480 bytes, is the deque doubling to 1024 events of the 20 bytes `NoteEvent<()>` then
    /// was. Since the fork's refresh onto 0.4.2 (2026-10-06) the event is 24 bytes, because a
    /// note's key, channel and voice id became typed; the fork's patch 1 still bounds the queue and
    /// reserves it at activation, so the record stays history. This pins today's size: if it ever
    /// fails, the event changed again and the record needs rechecking, not the plugin.
    #[test]
    fn note_event_size_explains_the_recorded_allocation_failure() {
        let size = size_of::<NoteEvent<()>>();
        assert_eq!(
            size, 24,
            "NoteEvent<()> is {size} bytes: recheck the record in docs/known-issues.md \
             (20 bytes under nice-plug 0.3.0, 24 since 0.4.2)"
        );
    }
}

/// The plugin's name, checked where it escapes this crate.
#[cfg(test)]
mod identity {
    use super::{CLAP_ID, NAME};

    /// The id is built from the name, so it cannot drift from it.
    #[test]
    fn the_id_is_the_name_under_the_project_domain() {
        assert_eq!(CLAP_ID, format!("dk.mxm.{NAME}"));
    }

    /// `bundler.toml` names the same instrument this crate does.
    ///
    /// **The one place the plugin's name is duplicated outside this crate**, and nothing else would
    /// catch a disagreement: `bundler.toml` is read by `xtask` at bundle time, never by the plugin,
    /// so a stale display name there produces a correctly-built bundle under the wrong filename.
    /// Cargo cannot check it — the file is hand-parsed and lives outside this crate — so this test
    /// is the check. See `plugin_name!` at the top of this file.
    #[test]
    fn the_bundle_is_named_after_this_plugin() {
        mxm_plugin_test::bundle::is_named(env!("CARGO_MANIFEST_DIR"), env!("CARGO_PKG_NAME"), NAME);
    }
}

/// The path from a host's note expression to the pitch.
///
/// **Velocity is the press that last triggered the envelope** — the modulation standard's rule, so a
/// route from it follows the phrase and not every key: a legato joint retriggers nothing and keeps
/// the phrase's velocity, and the next press from silence brings its own.
///
/// Falsified before trusted: storing every press's velocity reads the joint's 0.9.
#[cfg(test)]
mod velocity_source {
    use super::MxmMono01;
    use nice_plug::midi::{Channel, Key, VoiceID};
    use nice_plug::prelude::*;

    fn key(plugin: &mut MxmMono01, note: u8, velocity: f32, down: bool) {
        plugin.handle_event(if down {
            NoteEvent::NoteOn {
                timing: 0,
                voice_id: VoiceID::Wildcard,
                channel: Channel::Number(0),
                key: Key::Number(note),
                velocity,
            }
        } else {
            NoteEvent::NoteOff {
                timing: 0,
                voice_id: VoiceID::Wildcard,
                channel: Channel::Number(0),
                key: Key::Number(note),
                velocity: 0.0,
            }
        });
    }

    #[test]
    fn a_legato_joint_keeps_the_phrase_velocity() {
        let mut plugin = MxmMono01::default();
        key(&mut plugin, 60, 0.3, true);
        assert_eq!(plugin.next_patch().velocity, 0.3);
        key(&mut plugin, 64, 0.9, true);
        assert_eq!(
            plugin.next_patch().velocity,
            0.3,
            "a joint keeps the phrase's"
        );
        key(&mut plugin, 64, 0.0, false);
        key(&mut plugin, 60, 0.0, false);
        key(&mut plugin, 67, 0.7, true);
        assert_eq!(
            plugin.next_patch().velocity,
            0.7,
            "a press from silence brings its own"
        );
    }
}

/// **The link nothing else covers.** The DSP crate proves `expression_semitones` moves the pitch
/// and that `Voice::sounding` reports the right key; these prove the event reaches that field, and
/// that the note stack's fallback is honoured — the case with no note-on to notice it.
#[cfg(test)]
mod pitch_expression {
    use super::MxmMono01;
    use nice_plug::midi::{Channel, Key, VoiceID};
    use nice_plug::prelude::*;

    fn note_on(plugin: &mut MxmMono01, note: u8) {
        plugin.handle_event(NoteEvent::NoteOn {
            timing: 0,
            voice_id: VoiceID::Wildcard,
            channel: Channel::Number(0),
            key: Key::Number(note),
            velocity: 0.8,
        });
    }

    fn note_off(plugin: &mut MxmMono01, note: u8) {
        plugin.handle_event(NoteEvent::NoteOff {
            timing: 0,
            voice_id: VoiceID::Wildcard,
            channel: Channel::Number(0),
            key: Key::Number(note),
            velocity: 0.0,
        });
    }

    fn tune(plugin: &mut MxmMono01, note: u8, semitones: f32) {
        plugin.handle_event(NoteEvent::PolyTuning {
            timing: 0,
            voice_id: VoiceID::Wildcard,
            channel: Channel::Number(0),
            key: Key::Number(note),
            tuning: semitones,
        });
    }

    #[test]
    fn a_tuning_expression_for_the_sounding_note_reaches_the_patch() {
        let mut plugin = MxmMono01::default();
        note_on(&mut plugin, 48);
        assert_eq!(plugin.next_patch().expression_semitones, 0.0, "the premise");

        tune(&mut plugin, 48, -3.5);
        assert_eq!(
            plugin.next_patch().expression_semitones,
            -3.5,
            "the host's per-note pitch has to arrive in semitones, unscaled"
        );
    }

    #[test]
    fn an_expression_for_another_note_is_ignored() {
        let mut plugin = MxmMono01::default();
        note_on(&mut plugin, 48);
        tune(&mut plugin, 55, 7.0);
        assert_eq!(plugin.next_patch().expression_semitones, 0.0);
    }

    #[test]
    fn releasing_a_key_under_a_chord_does_not_hand_its_bend_to_the_key_that_stays() {
        // **The case with no note-on to notice it**, and the reason `Voice::sounding` exists.
        // Hold 60, then 67, bend 67, release 67: priority returns the voice to 60, which the host
        // never bent. Nothing in the event stream announces that move.
        let mut plugin = MxmMono01::default();
        note_on(&mut plugin, 60);
        note_on(&mut plugin, 67);
        tune(&mut plugin, 67, 7.0);
        assert_eq!(plugin.next_patch().expression_semitones, 7.0, "the premise");

        note_off(&mut plugin, 67);
        assert_eq!(
            plugin.next_patch().expression_semitones,
            0.0,
            "the bend belonged to the key that left, not to the one still held"
        );
    }

    #[test]
    fn the_last_key_released_keeps_its_bend_through_the_release() {
        // Going silent is a release, not a move: zeroing here would snap the pitch back mid-tail.
        let mut plugin = MxmMono01::default();
        note_on(&mut plugin, 60);
        tune(&mut plugin, 60, 4.0);
        note_off(&mut plugin, 60);
        assert_eq!(plugin.next_patch().expression_semitones, 4.0);
    }

    /// **A non-finite tuning is dropped at the event**, and the note keeps the offset it had. A
    /// NaN in the pitch sum makes the oscillator's frequency NaN, and its phase never recovers. The
    /// reference is the same instance never sent it.
    #[test]
    fn a_non_finite_tuning_expression_is_dropped_and_the_pitch_stays_finite() {
        let plugin = || {
            let mut plugin = MxmMono01::default();
            for (_, ptr, _) in plugin.params.param_map() {
                unsafe { ptr._internal_update_smoother(48_000.0, true) };
            }
            note_on(&mut plugin, 48);
            tune(&mut plugin, 48, 3.0);
            plugin
        };
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let (mut actual, mut reference) = (plugin(), plugin());
            tune(&mut actual, 48, bad);
            assert_eq!(actual.next_patch().expression_semitones, 3.0, "{bad}");
            reference.next_patch();
            let (mut heard, mut expected) = ([0.0f32; 2048], [0.0f32; 2048]);
            actual.render_block_for_test(&mut heard);
            reference.render_block_for_test(&mut expected);
            assert!(heard.iter().all(|s| s.is_finite()), "{bad}");
            assert_eq!(heard, expected, "{bad}");
            assert!(heard.iter().any(|&s| s != 0.0), "the note sounds");
        }
    }
}

#[cfg(test)]
mod developer_channel_tests {
    use super::*;

    fn cc(plugin: &mut MxmMono01, cc: u8, raw: u8) {
        plugin.handle_event(NoteEvent::MidiCC {
            timing: 0,
            channel: 0,
            cc,
            value: f32::from(raw) / 127.0,
        });
    }

    /// The developer channel reaches the editor only when the instance was started with it; a
    /// host sending the same control change to an ordinary instance changes nothing.
    #[test]
    fn the_developer_channel_is_off_unless_the_environment_asked_for_it() {
        let mut plugin = MxmMono01 {
            dev_cc: false,
            ..Default::default()
        };
        cc(&mut plugin, DEV_VIEW_CC, 1);
        cc(&mut plugin, DEV_DISCLOSURE_CC, 127);
        cc(&mut plugin, DEV_BROWSER_CC, 127);
        cc(&mut plugin, DEV_THEME_CC, 1);
        assert_eq!(plugin.telemetry.take_view_request(), None);
        assert_eq!(plugin.telemetry.take_disclosure_request(), None);
        assert_eq!(plugin.telemetry.take_browser_request(), None);
        assert_eq!(plugin.telemetry.take_theme_request(), None);

        plugin.dev_cc = true;
        cc(&mut plugin, DEV_VIEW_CC, 1);
        cc(&mut plugin, DEV_DISCLOSURE_CC, 127);
        cc(&mut plugin, DEV_BROWSER_CC, 127);
        cc(&mut plugin, DEV_THEME_CC, 1);
        assert_eq!(plugin.telemetry.take_view_request(), Some(1));
        assert_eq!(plugin.telemetry.take_disclosure_request(), Some(true));
        assert_eq!(plugin.telemetry.take_browser_request(), Some(true));
        assert_eq!(
            plugin.telemetry.take_theme_request(),
            Some(1),
            "1 is dark, as mxm_ui::theme::from_index reads it"
        );
        // Light is index 0 — a request like any other, not the absence of one.
        cc(&mut plugin, DEV_THEME_CC, 0);
        assert_eq!(plugin.telemetry.take_theme_request(), Some(0));
        // View zero is a request too, not the absence of one.
        cc(&mut plugin, DEV_VIEW_CC, 0);
        assert_eq!(plugin.telemetry.take_view_request(), Some(0));
    }
}

/// The pre-conversion baseline `plans/plan-modulation-routing.md` M0 exists to capture.
///
/// **Both figures stop existing the moment the conversion starts**, which is why they are taken
/// first and recorded rather than re-derived later. Neither is an assertion: they print, and the
/// numbers go into the plan's revision table and this crate's NOTES.md with the machine named,
/// because a cost probe is reproducible on one machine and not portable across machines —
/// mxm-bucket-delay's `crates/mxm-bucket-delay-dsp/AGENTS.md` states that hedge for its own figure.
///
/// ```bash
/// cargo test -p mxm-mono-01 --release baseline -- --ignored --nocapture
/// ```
///
/// **Release, or the numbers mean nothing**, exactly as mxm-kit's
/// `crates/ui/tests/flow_resize_bench.rs` says of its own bench.
#[cfg(test)]
mod baseline {
    use super::*;
    use nice_plug::params::Params;
    use std::time::Instant;

    const FS: f32 = 48_000.0;
    const BLOCK: usize = 64;

    /// A plugin with every smoother activated, which is gotcha 13 in mxm-kit's
    /// `docs/adding-an-instrument.md`: nice-plug initialises a smoother in `activate`, not in
    /// `FloatParam::new`, so without this every smoothed parameter reads zero and the measurement
    /// is of the wrong thing.
    fn plugin() -> MxmMono01 {
        let mut plugin = MxmMono01::default();
        for (_, ptr, _) in plugin.params.param_map() {
            unsafe { ptr._internal_update_smoother(FS, true) };
        }
        // What `activate` does, minus the host: the voice needs its rate and the telemetry does not
        // matter here.
        plugin.sample_rate = FS;
        plugin.voice.set_sample_rate(FS);
        plugin.voice.reset();
        plugin
    }

    fn note_on(plugin: &mut MxmMono01, note: u8) {
        plugin.handle_event(NoteEvent::NoteOn {
            timing: 0,
            voice_id: VoiceID::Wildcard,
            channel: Channel::Number(0),
            key: Key::Number(note),
            velocity: 0.8,
        });
    }

    /// Samples per second through the plugin's own per-sample path, held note, init patch.
    #[test]
    #[ignore = "a measurement, not an assertion; release only"]
    fn throughput_of_the_init_patch() {
        let mut plugin = plugin();
        note_on(&mut plugin, 48);

        let mut out = [0.0f32; BLOCK];
        // Warm the caches first: the first blocks pay for page faults, which is not the question.
        for _ in 0..64 {
            plugin.render_block_for_test(&mut out);
        }

        let blocks = 40_000;
        let start = Instant::now();
        for _ in 0..blocks {
            plugin.render_block_for_test(&mut out);
        }
        let taken = start.elapsed().as_secs_f64();
        let samples = (blocks * BLOCK) as f64;

        println!("\n  mxm-mono-01, pre-conversion baseline, init patch, held note");
        println!("  {FS} Hz, block {BLOCK}, {blocks} blocks");
        println!("  {:.0} samples/s", samples / taken);
        println!("  {:.2}x realtime", samples / taken / f64::from(FS));
        println!("  {:.3} ns/sample\n", taken * 1e9 / samples);
    }

    /// FNV-1a over the raw bits, the same digest `plugins/mxm-mono-01/host-tests/tests/golden_audio.rs:81`
    /// uses — no dependency, and a bit-for-bit comparison expressed compactly rather than a
    /// perceptual one.
    fn digest(samples: &[f32]) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for sample in samples {
            for byte in sample.to_bits().to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        format!("{hash:016x}")
    }

    /// One note, held then released, long enough for the release to finish.
    ///
    /// Deliberately the same shape for every preset: what is being pinned is *this patch through
    /// this voice*, so the gesture must not vary between them or a digest change could not be
    /// attributed.
    fn render_one(plugin: &mut MxmMono01) -> Vec<f32> {
        let held = (FS * 1.5) as usize;
        let tail = (FS * 2.5) as usize;
        let mut out = vec![0.0f32; held + tail];

        note_on(plugin, 48);
        let (a, b) = out.split_at_mut(held);
        for block in a.chunks_mut(BLOCK) {
            plugin.render_block_for_test(block);
        }
        plugin.handle_event(NoteEvent::NoteOff {
            timing: 0,
            voice_id: VoiceID::Wildcard,
            channel: Channel::Number(0),
            key: Key::Number(48),
            velocity: 0.0,
        });
        for block in b.chunks_mut(BLOCK) {
            plugin.render_block_for_test(block);
        }
        out
    }

    /// Applies a factory preset's stored values by id, the way the preset system does: **only `v`
    /// is read**, because `text` exists so the file diffs (`plugins/AGENTS.md`).
    fn apply(plugin: &MxmMono01, json: &str) -> usize {
        let map = plugin.params.param_map();
        let mut applied = 0;
        // The format is two numbers per parameter under `"params"`; this reads `v` positionally
        // rather than pulling in a parser, which is all a measurement harness needs.
        for (id, ptr, _) in &map {
            let key = format!("\"{id}\"");
            let Some(at) = json.find(&key) else { continue };
            let rest = &json[at + key.len()..];
            let Some(vpos) = rest.find("\"v\"") else {
                continue;
            };
            let after = &rest[vpos + 3..];
            let num: String = after
                .chars()
                .skip_while(|c| *c == ':' || c.is_whitespace())
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == 'e')
                .collect();
            if let Ok(v) = num.parse::<f32>() {
                unsafe { ptr._internal_set_normalized_value(v) };
                applied += 1;
            }
        }
        for (_, ptr, _) in &map {
            unsafe { ptr._internal_update_smoother(FS, true) };
        }
        applied
    }

    /// Prints a digest per factory sound, which is the reference the conversion is held to.
    ///
    /// **Captured before the conversion and recorded**, because after it the old renders cannot be
    /// produced. It prints rather than asserts at M0; the runs-by-default comparison arrives with
    /// the conversion, against the numbers this produced.
    #[test]
    #[ignore = "a capture, not an assertion; release only"]
    fn factory_bank_reference_digests() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("presets");
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .expect("presets directory")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".json"))
            .collect();
        names.sort();

        println!(
            "
  mxm-mono-01 factory bank — pre-conversion reference digests"
        );
        println!(
            "  {FS} Hz, 1.5 s held then 2.5 s release, note 48
"
        );
        for name in &names {
            let json = std::fs::read_to_string(dir.join(name)).expect("preset");
            let mut plugin = plugin();
            let applied = apply(&plugin, &json);
            let audio = render_one(&mut plugin);
            let peak = audio.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            println!(
                "  {:<28} {}  peak {:>6.4}  ({applied} params)",
                name.trim_end_matches(".json"),
                digest(&audio),
                peak
            );
        }
        println!(
            "
  {} sounds
",
            names.len()
        );
    }
}

/// The host's sample rate at activation: the floor the DSP's clamps are safe above.
#[cfg(test)]
mod sample_rate_floor {
    use super::*;
    use mxm_mono_01_dsp::MIN_SAMPLE_RATE;

    struct Activation;

    impl ActivateContext<MxmMono01> for Activation {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn execute(&self, _task: ()) {}
        fn set_latency_samples(&self, _samples: u32) {}
        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    fn activate_at(plugin: &mut MxmMono01, sample_rate: f32) -> bool {
        plugin.activate(
            &MxmMono01::AUDIO_IO_LAYOUTS[0],
            &BufferConfig {
                sample_rate,
                min_buffer_size: Some(1),
                max_buffer_size: 4096,
                process_mode: ProcessMode::Realtime,
            },
            &mut Activation,
        )
    }

    fn render(plugin: &mut MxmMono01, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; frames];
        plugin.render_block_for_test(&mut out);
        out
    }

    /// **The floor activates and plays, whatever the parameters say.** Every parameter at its
    /// default, then all at the bottom of their ranges, then all at the top — every route present
    /// at full — with a note held for four seconds at 1 kHz.
    #[test]
    fn the_rate_floor_activates_and_plays_at_every_parameter_extreme() {
        for extreme in [None, Some(0.0), Some(1.0)] {
            let mut plugin = MxmMono01::default();
            for (_, ptr, _) in plugin.params.param_map() {
                if let Some(value) = extreme {
                    let _ = unsafe { ptr._internal_set_normalized_value(value) };
                }
                unsafe { ptr._internal_update_smoother(MIN_SAMPLE_RATE, true) };
            }
            assert!(activate_at(&mut plugin, MIN_SAMPLE_RATE), "{extreme:?}");
            assert_eq!(plugin.sample_rate, MIN_SAMPLE_RATE);
            plugin.handle_event(NoteEvent::NoteOn {
                timing: 0,
                voice_id: VoiceID::Wildcard,
                channel: Channel::Number(0),
                key: Key::Number(48),
                velocity: 0.8,
            });
            let out = render(&mut plugin, 4_000);
            assert!(out.iter().all(|s| s.is_finite()), "{extreme:?}");
        }
    }

    /// **A rate the DSP's clamps cannot hold is refused at activation.** `f32::clamp` panics on
    /// a NaN or inverted bound, so a NaN rate or one low enough to cross a corner's floor over its
    /// Nyquist fraction panicked on the audio thread. A refusal leaves the plugin as it was.
    #[test]
    fn activation_refuses_a_non_finite_rate_and_any_below_the_floor() {
        for unsupported in [
            MIN_SAMPLE_RATE.next_down(),
            100.0,
            20.0,
            1.0,
            0.0,
            -48_000.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            let mut refused = MxmMono01::default();
            assert!(
                !activate_at(&mut refused, unsupported),
                "accepted {unsupported} Hz"
            );
            assert_eq!(refused.sample_rate, 48_000.0, "{unsupported} Hz");
        }
    }
}

/// **A synced value reaches the patch** (`plans/plan-tempo-sync-controls.md`): what a sync resolved
/// for this callback is what the DSP is given, and with none the free value is.
#[cfg(test)]
mod tempo_sync_path {
    use super::*;

    #[test]
    fn the_synced_lfo_rate_is_the_patchs() {
        let mut plugin = MxmMono01::default();
        let free = plugin.next_patch().lfo_rate_hz;
        plugin.synced_lfo_hz = Some(free + 1.0);
        assert_eq!(plugin.next_patch().lfo_rate_hz, free + 1.0);
        plugin.synced_lfo_hz = None;
        assert_eq!(plugin.next_patch().lfo_rate_hz, free);
    }
}

/// **Activation forgets the last session's tempo and resolved syncs**: the first callback reports the
/// tempo, so nothing — the audio, or an editor frame before it — starts from the previous session's
/// divisions.
#[cfg(test)]
mod activation_forgets_the_tempo {
    use super::*;

    #[test]
    fn activation_forgets_the_last_tempo_and_resolved_syncs() {
        use nice_plug::prelude::Plugin as _;
        let mut plugin = MxmMono01::default();
        plugin.telemetry.tempo.publish(Some(120.0));
        plugin.synced_lfo_hz = Some(1.0);
        let layout = MxmMono01::AUDIO_IO_LAYOUTS[0];
        let config = BufferConfig {
            sample_rate: 48_000.0,
            min_buffer_size: None,
            max_buffer_size: 512,
            process_mode: ProcessMode::Realtime,
        };
        let _ = plugin.activate(&layout, &config, &mut NoInit);
        assert_eq!(plugin.telemetry.tempo.get(), None);
        assert_eq!(plugin.synced_lfo_hz, None);
    }

    /// An activation context that asks nothing of a host.
    struct NoInit;

    impl ActivateContext<MxmMono01> for NoInit {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn execute(&self, _task: <MxmMono01 as Plugin>::BackgroundTask) {}
        fn set_latency_samples(&self, _samples: u32) {}
        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }
}

/// What a player reads — on hover in the editor, and in a host's plugin browser — speaks to the
/// player about the sound, never about the machine or the code (`mxm_plugin_test::hover_text`).
#[cfg(test)]
mod speaks_to_the_player {
    #[test]
    fn hover_text() {
        mxm_plugin_test::hover_text::speaks_to_the_player(env!("CARGO_MANIFEST_DIR"));
    }

    #[test]
    fn host_description() {
        mxm_plugin_test::hover_text::host_description_speaks_to_the_player(env!(
            "CARGO_MANIFEST_DIR"
        ));
    }
}

/// A NoteOn on a host's wildcard channel indexed the 16-entry per-channel tables with 255 and
/// panicked (found while porting to nice-plug 0.4.2, 2026-10-06); it now plays on channel 15's.
#[cfg(test)]
mod wildcard_channel {
    use super::*;
    use nice_plug::midi::{Channel, Key, VoiceID};

    #[test]
    fn a_note_on_a_wildcard_channel_plays_instead_of_panicking() {
        let mut plugin = MxmMono01::default();
        plugin.handle_event(NoteEvent::NoteOn {
            timing: 0,
            voice_id: VoiceID::Wildcard,
            channel: Channel::Wildcard,
            key: Key::Number(60),
            velocity: 0.8,
        });
        let _ = plugin.next_patch();
        assert_eq!(usize::from(plugin.active_channel), NUM_CHANNELS - 1);
    }
}
