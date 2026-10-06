//! The monophonic voice: note handling, glide, and the full signal path.
//!
//! ```text
//! note stack -> glide -> VCO + sub + noise -> mixer -> DC block
//!                                                          |
//!            ENV -+-> filter cutoff -> 4-pole ladder <-----+
//!                 |                         |
//!                 +-> VCA <-----------------+ -> output gain
//!            LFO ---> pitch, cutoff, pulse width
//! ```
//!
//! One envelope drives both the filter and the amplifier. That is the instrument's
//! defining constraint and is deliberate.

use crate::envelope::Adsr;
use crate::filter::Ladder;
use crate::lfo::{Lfo, LfoShape};
use crate::oscillator::{DcBlocker, MixLevels, Oscillator, SubShape};
use crate::routing::{self, Graph, Routing, source, target};
use mxm_modulation::standard;

/// Full-scale filter envelope amount, in octaves.
pub const FILTER_ENV_OCTAVES: f32 = 6.0;
/// Full-scale filter LFO amount, in octaves.
pub const FILTER_LFO_OCTAVES: f32 = 4.0;
/// Full-scale LFO-to-pitch amount, in semitones.
pub const VCO_LFO_SEMITONES: f32 = 7.0;
/// Full-scale pulse-width swing, either side of the manual width.
pub const PWM_WIDTH_SWING: f32 = 0.45;
/// Gate rise/fall time, and the time over which a VCA source change crossfades.
pub const GATE_TIME_S: f32 = 0.002;

const MAX_HELD_NOTES: usize = 16;

/// What opens the amplifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VcaSource {
    /// Shaped by the envelope.
    #[default]
    Envelope,
    /// Straight on/off with the key, lightly smoothed. Organ-like.
    Gate,
}

/// How a new note affects the envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Retrigger {
    /// Only the first note of a phrase restarts the envelope. This is what makes
    /// 101-style basslines work.
    #[default]
    Legato,
    /// Every note restarts it.
    Always,
}

/// A held key. Matched by `voice_id` when the host supplies one, since that is the
/// only unambiguous identifier when the same note is pressed twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteId {
    pub voice_id: Option<i32>,
    pub channel: u8,
    pub note: u8,
}

impl NoteId {
    pub fn matches(&self, voice_id: Option<i32>, channel: u8, note: u8) -> bool {
        match (self.voice_id, voice_id) {
            // A voice id is authoritative when both sides have one.
            (Some(a), Some(b)) => a == b,
            _ => self.channel == channel && self.note == note,
        }
    }
}

/// Fixed-capacity stack of held notes, newest last.
///
/// Repeated presses of the same key are stored as **separate entries** rather than
/// replacing one another. Replacing loses key state: the first note-off would then
/// end a note whose key is still physically held.
#[derive(Debug, Clone, Default)]
pub struct NoteStack {
    entries: [Option<NoteId>; MAX_HELD_NOTES],
    len: usize,
}

impl NoteStack {
    pub fn clear(&mut self) {
        self.entries = [None; MAX_HELD_NOTES];
        self.len = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn newest(&self) -> Option<NoteId> {
        if self.len == 0 {
            None
        } else {
            self.entries[self.len - 1]
        }
    }

    /// Push a note. When full, the **oldest** entry is dropped: it is the least
    /// likely to still be held, and the newest is the one that is sounding.
    pub fn push(&mut self, id: NoteId) {
        if self.len == MAX_HELD_NOTES {
            self.entries.rotate_left(1);
            self.len -= 1;
        }
        self.entries[self.len] = Some(id);
        self.len += 1;
    }

    /// Remove the most recent entry matching the given note, returning whether it
    /// was the one currently sounding (i.e. the newest).
    pub fn remove(&mut self, voice_id: Option<i32>, channel: u8, note: u8) -> Removed {
        for i in (0..self.len).rev() {
            let Some(entry) = self.entries[i] else {
                continue;
            };
            if entry.matches(voice_id, channel, note) {
                let was_newest = i == self.len - 1;
                for j in i..self.len - 1 {
                    self.entries[j] = self.entries[j + 1];
                }
                self.entries[self.len - 1] = None;
                self.len -= 1;
                return if was_newest {
                    Removed::Sounding
                } else {
                    Removed::Held
                };
            }
        }
        Removed::NotFound
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removed {
    /// The removed note was the one being played.
    Sounding,
    /// It was held but not sounding, so nothing audible changes.
    Held,
    NotFound,
}

/// Everything the voice needs for one sample, as plain values.
///
/// Rebuilt per sample by the plugin shell from the framework's parameter smoothers,
/// which is what keeps modulation sample-accurate. Deliberately `Copy` and free of
/// any framework type.
#[derive(Debug, Clone, Copy)]
pub struct Patch {
    /// Semitone offset from the range switch: 16'/8'/4'/2' -> -12/0/+12/+24.
    pub range_offset: f32,
    pub tune_cents: f32,
    pub glide_time_s: f32,
    /// Pitch bend already scaled by the bend range, in semitones.
    pub bend_semitones: f32,
    /// The bender's lever, `-1..=1`: the Bend source, which is the lever and not the pitch it bends.
    pub bend: f32,
    /// Per-note pitch expression, in semitones.
    ///
    /// **Separate from `bend_semitones`, and deliberately.** Bend is a channel-wide performance
    /// control with a range the patch owns; this is the host's *per-note* offset — CLAP's
    /// `CLAP_NOTE_EXPRESSION_TUNING`, which is what a curve drawn against one note in a DAW's
    /// piano roll arrives as. They sum, because both are semitones on the same pitch, but folding
    /// them into one field would make the patch unable to say which of the two moved.
    ///
    /// Unsmoothed, exactly as bend is: the host already delivers it as a stream of timed events,
    /// and `process` splits its block on each one, so a smoother here would blur a value that is
    /// already sample-accurate.
    ///
    /// One voice means one expression. **When this crate grows a polyphonic sibling it becomes
    /// per-voice state**, which is the whole reason it is a patch field rather than a sum the
    /// plugin folds into bend before the DSP ever sees it.
    pub expression_semitones: f32,
    /// The velocity of the press that last triggered the envelope, `0..=1`, as a routable source.
    /// Published as the standard's `v − 1`.
    pub velocity: f32,
    /// The mod wheel, `0..=1`, as a routable source.
    pub wheel: f32,
    /// Channel pressure, `0..=1`, as a routable source.
    pub pressure: f32,

    pub levels: MixLevels,
    /// The width the pulse sits at with nothing modulating it. **The manual width only**: what used
    /// to be `pwmdepth` and its three-way source switch is now two routes into
    /// [`routing::target::PULSE_WIDTH`], one per source, each with its own depth.
    pub pulse_width: f32,
    pub sub_shape: SubShape,

    pub cutoff_hz: f32,
    pub resonance: f32,

    pub attack_s: f32,
    pub decay_s: f32,
    pub sustain: f32,
    pub release_s: f32,

    pub lfo_rate_hz: f32,
    pub lfo_shape: LfoShape,

    pub vca_source: VcaSource,
    /// Linear gain, never dB.
    pub output_gain: f32,
}

impl Default for Patch {
    fn default() -> Self {
        Self {
            range_offset: 0.0,
            tune_cents: 0.0,
            glide_time_s: 0.0,
            bend_semitones: 0.0,
            bend: 0.0,
            expression_semitones: 0.0,
            velocity: 1.0,
            wheel: 0.0,
            pressure: 0.0,
            levels: MixLevels {
                saw: 0.8,
                pulse: 0.0,
                sub: 0.0,
                noise: 0.0,
            },
            pulse_width: 0.5,
            sub_shape: SubShape::Oct1Square,
            cutoff_hz: 12_000.0,
            resonance: 0.0,
            attack_s: 0.005,
            decay_s: 0.3,
            sustain: 0.7,
            release_s: 0.2,
            lfo_rate_hz: 4.0,
            lfo_shape: LfoShape::Triangle,
            vca_source: VcaSource::Envelope,
            output_gain: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Voice {
    /// The routing graph: the source frame, and which routes are live into each target.
    graph: Graph,
    osc: Oscillator,
    dc: DcBlocker,
    filter: Ladder,
    env: Adsr,
    lfo: Lfo,

    stack: NoteStack,
    /// Note number currently sounding, used for pitch and keyboard tracking.
    current_note: f32,
    /// Glide state: the **remaining distance**, in semitones, from `glide_target` — the pitch the
    /// lag last aimed at, range switch included. The glided pitch is `glide_target + glide_offset`.
    ///
    /// Kept as the distance rather than the pitch because the pitch form,
    /// `target + (glide − target) × coef`, stops moving in `f32` once a step is under half an ulp
    /// of the target: a long glide came to rest short of its note, 18 cents at one second and
    /// 48 kHz, until the next note. The distance keeps full precision down to zero, so every glide
    /// lands.
    glide_offset: f32,
    glide_target: f32,
    /// Set by a note from idle: the next sample snaps to its target, range switch and all.
    glide_snap: bool,
    /// Smoothed gate, and smoothed VCA-source position (0 = envelope, 1 = gate).
    gate: f32,
    vca_mix: f32,

    sample_rate: f32,
    gate_coef: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}

impl Voice {
    pub fn new() -> Self {
        let mut v = Self {
            osc: Oscillator::new(),
            dc: DcBlocker::new(),
            filter: Ladder::new(),
            env: Adsr::new(),
            lfo: Lfo::new(),
            stack: NoteStack::default(),
            current_note: 60.0,
            glide_offset: 0.0,
            glide_target: 60.0,
            glide_snap: false,
            gate: 0.0,
            vca_mix: 0.0,
            sample_rate: 48_000.0,
            gate_coef: 0.0,
            graph: Graph::new(),
        };
        v.set_sample_rate(48_000.0);
        v
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.dc.set_sample_rate(sample_rate);
        self.env.set_sample_rate(sample_rate);
        self.gate_coef = (-1.0 / (GATE_TIME_S * sample_rate)).exp();
    }

    /// Clear every bit of state. Must leave no tail from the previous playback.
    /// Rebuilds which routes are live. **Once per processing interval, never per sample** — topology
    /// is discrete and changes only on a parameter event, which is the whole of the efficiency
    /// design.
    pub fn set_topology(&mut self, routing: &Routing) {
        self.graph.set_topology(routing);
    }

    pub fn reset(&mut self) {
        self.osc.reset();
        self.dc.reset();
        self.filter.reset();
        self.env.reset();
        self.lfo.reset();
        self.stack.clear();
        self.current_note = 60.0;
        self.glide_offset = 0.0;
        self.glide_target = 60.0;
        self.glide_snap = false;
        self.gate = 0.0;
        self.vca_mix = 0.0;
        // The frame's previous half is state a backward route reads, so leaving it would let one
        // render leak a sample into the next.
        self.graph.reset();
    }

    /// The note the voice is actually sounding, if any.
    ///
    /// **A routing accessor, not a telemetry one**, and it needs its own argument (see this
    /// crate's AGENTS.md). A per-note expression names the note it belongs to, so the plugin must
    /// decide whether an arriving `PolyTuning` is about the note being played — and after a
    /// note-off the answer is the crate's to give, not the plugin's: releasing the newest key
    /// returns the voice to an older *held* one under the priority rules, and `stack` is where
    /// that decision lives. A plugin-side mirror would be a second copy of a stored fact, free to
    /// disagree with it the moment a key is released under a chord.
    ///
    /// Read-only and additive, exactly as the envelope accessors are: no new state, no branch in
    /// the audio path, and the DSP does not know anyone is looking.
    pub fn sounding(&self) -> Option<NoteId> {
        self.stack.newest()
    }

    pub fn is_active(&self) -> bool {
        self.env.is_active() || self.gate > crate::envelope::ZERO_THRESHOLD
    }

    /// The pitch the glide has reached, in semitones, for tests.
    #[cfg(test)]
    fn glided(&self) -> f32 {
        self.glide_target + self.glide_offset
    }

    /// What this voice's frame holds for `source`, for tests.
    #[cfg(test)]
    pub(crate) fn published_for_test(&self, source: usize) -> f32 {
        self.graph.read_for_test(source)
    }

    /// The envelope's current level, `0..=1`.
    ///
    /// # Why this exists
    ///
    /// The editor's brief (§8) requires an envelope display with a position indicator moving
    /// through it while a note sounds, and there was no way to see that value: `env` is private
    /// and [`Voice::process`] computes the level into a local and returns only the output sample.
    /// Atomics can carry a value across a thread; they cannot invent one that is never published.
    ///
    /// # The rules that come with it
    ///
    /// This is **read-only telemetry**, and it adds no state, no branch in the audio path and no
    /// allocation. The DSP does not know it exists.
    ///
    /// **The audio thread reads it, once per block, and publishes to an atomic. A UI thread must
    /// never call it** — `Voice` is `&mut`-owned by the processor, and sharing it with a UI would
    /// need a lock the audio callback then has to take.
    pub fn env_level(&self) -> f32 {
        self.env.level()
    }

    /// Which segment the envelope is in. Pairs with [`Voice::env_level`]; same rules.
    ///
    /// The level alone is ambiguous — 0.7 could be a rising attack or a decaying tail — so a
    /// display that draws a position on the shape needs the segment as well as the height.
    pub fn env_stage(&self) -> crate::envelope::Stage {
        self.env.stage()
    }

    /// Samples of audible tail remaining. Reflects post-VCA output only: filter
    /// ringing behind a closed amplifier is inaudible and is deliberately not
    /// counted.
    pub fn tail_samples(&self, release_s: f32) -> u32 {
        let env_tail = self.env.tail_samples(release_s);
        let gate_tail = if self.gate > crate::envelope::ZERO_THRESHOLD {
            (GATE_TIME_S * self.sample_rate * 4.0) as u32
        } else {
            0
        };
        env_tail.max(gate_tail)
    }

    pub fn note_on(&mut self, id: NoteId, retrigger: Retrigger) {
        let was_empty = self.stack.is_empty();
        let starting_from_idle = !self.env.is_active();

        self.stack.push(id);
        self.current_note = id.note as f32;

        // A first note has no previous pitch to slide from, so snap rather than
        // gliding up from whatever was left over. **To the range switch too**: this used to snap
        // to the bare key, so a first note at 16', 4' or 2' still slid the switch's octaves.
        if starting_from_idle {
            self.glide_snap = true;
        }

        if retrigger == Retrigger::Always || was_empty {
            self.env.trigger();
        }
    }

    pub fn note_off(&mut self, voice_id: Option<i32>, channel: u8, note: u8, retrigger: Retrigger) {
        match self.stack.remove(voice_id, channel, note) {
            Removed::NotFound | Removed::Held => {}
            Removed::Sounding => self.after_losing_the_sounding_note(retrigger),
        }
    }

    /// Choke targets one specific note rather than everything.
    pub fn choke(&mut self, voice_id: Option<i32>, channel: u8, note: u8) {
        match self.stack.remove(voice_id, channel, note) {
            Removed::NotFound | Removed::Held => {}
            Removed::Sounding => {
                if self.stack.is_empty() {
                    // Choke is immediate, unlike a note-off.
                    self.env.silence();
                } else {
                    self.fall_back_to_newest(Retrigger::Legato);
                }
            }
        }
    }

    /// CC 120. Immediate, no release.
    ///
    /// **It clears the filter and the DC blocker ahead of it as well as the envelope.** The voice
    /// is inactive the moment this returns, so the plugin reports `Normal` and a host may stop
    /// calling at once; whatever the two recursive stages held would then be where the next note
    /// starts, however long the host slept. The oscillator and the LFO run free and are kept.
    pub fn all_sound_off(&mut self) {
        self.stack.clear();
        self.env.silence();
        self.gate = 0.0;
        self.dc.reset();
        self.filter.reset();
    }

    /// CC 123. Deliberately different from all-sound-off: notes release normally.
    pub fn all_notes_off(&mut self) {
        self.stack.clear();
        self.env.release();
    }

    fn after_losing_the_sounding_note(&mut self, retrigger: Retrigger) {
        if self.stack.is_empty() {
            self.env.release();
        } else {
            self.fall_back_to_newest(retrigger);
        }
    }

    /// Returning to a still-held note. In Legato this does not restart the
    /// envelope, which is the whole point of the mode. In Always it does, matching
    /// that mode's promise that every note change retriggers.
    fn fall_back_to_newest(&mut self, retrigger: Retrigger) {
        if let Some(next) = self.stack.newest() {
            self.current_note = next.note as f32;
            if retrigger == Retrigger::Always {
                self.env.trigger();
            }
        }
    }

    /// Render one sample.
    #[inline]
    pub fn process(&mut self, p: &Patch, routing: &Routing) -> f32 {
        let fs = self.sample_rate;

        let lfo = self.lfo.process(p.lfo_rate_hz, p.lfo_shape, fs);
        let env = self
            .env
            .process(p.attack_s, p.decay_s, p.sustain, p.release_s);

        // **Nothing routed costs nothing.** With no live route the frame is not opened, no source
        // is published and no sum is taken, so an unrouted patch runs the arithmetic it always ran.
        // That is the owner's requirement — the default must not cost more than it did — and it is
        // a branch per sample rather than a promise.
        let routed = self.graph.any_live();

        // Glide. A zero time means jump straight there. A new target leaves the pitch where it
        // was, so the distance takes up the difference before it decays. Before the sources are
        // published, because Key is the note the voice is sounding, glide and all.
        let target = self.current_note + p.range_offset;
        if p.glide_time_s <= 0.0 || self.glide_snap {
            self.glide_offset = 0.0;
            self.glide_snap = false;
        } else {
            let coef = (-1.0 / (p.glide_time_s * fs)).exp();
            self.glide_offset =
                crate::flush((self.glide_offset + (self.glide_target - target)) * coef);
        }
        self.glide_target = target;
        let glide = target + self.glide_offset;

        // Open the sample and publish the sources that exist by now, in the order
        // `routing::source` declares, the performance sources through the collection's standard so
        // each is zero at its rest. The audio sources are published after the oscillator has run,
        // further down, which is what makes a route from one of them a backward route.
        if routed {
            self.graph.begin_sample();
            self.graph.write(source::LFO, lfo);
            self.graph.write(source::ENVELOPE, env);
            // The keyboard's note, glided; the range switch is the oscillator's.
            self.graph.write(
                source::KEY,
                standard::key(
                    self.current_note + self.glide_offset,
                    routing::KEY_UNIT_SEMITONES,
                ),
            );
            self.graph
                .write(source::VELOCITY, standard::velocity(p.velocity));
            self.graph.write(source::WHEEL, standard::wheel(p.wheel));
            self.graph
                .write(source::PRESSURE, standard::pressure(p.pressure));
            self.graph.write(source::BEND, standard::bend(p.bend));
        }

        let pitch_st = glide
            + p.tune_cents / 100.0
            + p.bend_semitones
            + p.expression_semitones
            + if routed {
                self.graph.sum(target::PITCH, routing)
            } else {
                0.0
            };
        let freq_hz = 440.0 * ((pitch_st - 69.0) / 12.0).exp2();

        let pulse_width = p.pulse_width
            + if routed {
                self.graph.sum(target::PULSE_WIDTH, routing)
            } else {
                0.0
            };

        let mixed = self
            .osc
            .process(freq_hz, pulse_width, p.sub_shape, &p.levels, fs);
        let blocked = self.dc.process(mixed);

        // The instrument's own audio, published now that it exists. A route from one of these is a
        // backward route for anything upstream and reads last sample's value; that one-sample delay
        // is what keeps such a cycle finite, and at audio rate it is a comb.
        if routed {
            let parts = self.osc.parts();
            self.graph.write(source::SAW, parts.saw);
            self.graph.write(source::PULSE, parts.pulse);
            self.graph.write(source::SUB, parts.sub);
            self.graph.write(source::NOISE, parts.noise);
        }

        // Cutoff modulation is done in octaves, which is the only way it stays
        // musically consistent across the range.
        let octaves = if routed {
            self.graph.sum(target::CUTOFF, routing)
        } else {
            0.0
        };
        let cutoff = p.cutoff_hz * octaves.exp2();

        let filtered = self.filter.process(blocked, cutoff, p.resonance, fs);

        // Gate follows the keys; the VCA source crossfades rather than switching,
        // so changing it mid-note does not click.
        let gate_target = if self.stack.is_empty() { 0.0 } else { 1.0 };
        self.gate = gate_target + (self.gate - gate_target) * self.gate_coef;
        let mix_target = match p.vca_source {
            VcaSource::Envelope => 0.0,
            VcaSource::Gate => 1.0,
        };
        self.vca_mix = mix_target + (self.vca_mix - mix_target) * self.gate_coef;

        let amp = env + (self.gate - env) * self.vca_mix;

        // **Amplitude routes scale the envelope rather than adding to it.** Adding would make the
        // voice's lifetime circular: a latched key routed here would hold the voice audible after
        // its envelope ended, so ending on the envelope cuts a non-zero signal and ending on
        // audibility never ends. Scaling keeps the routes expressive through the release and leaves
        // an exhausted envelope at exact zero whatever is routed — which is what a CV into a VCA
        // does in hardware anyway. The collection's one amplitude law: silence to double, however
        // many routes.
        let amp = if routed {
            amp * standard::amplitude_factor(self.graph.sum(target::AMPLITUDE, routing))
        } else {
            amp
        };

        crate::flush(filtered * amp * p.output_gain)
    }
}

#[cfg(test)]
mod tests {
    /// The Sub bass factory preset, as a `Patch`.
    ///
    /// Reported clicking on a sustained note, on some pitches and not others. Written out here
    /// rather than derived from the preset file so the test says what it is testing.
    fn sub_bass() -> Patch {
        Patch {
            range_offset: -12.0, // 16'
            levels: MixLevels {
                saw: 0.0,
                pulse: 0.0,
                sub: 1.0,
                noise: 0.0,
            },
            sub_shape: SubShape::Oct2Square,
            cutoff_hz: 86.3,
            resonance: 0.0,
            sustain: 1.0,
            glide_time_s: 0.003,
            attack_s: 0.002,
            decay_s: 1.2,
            release_s: 0.075,
            ..Patch::default()
        }
    }

    /// The largest jump between consecutive samples, and where it was.
    ///
    /// `mxm-measure`'s observation. `mxm-bucket-delay-dsp` had the same computation under the name
    /// `worst_step`; two names for one concept is what the shared crate exists to end.
    fn worst_jump(samples: &[f32]) -> (usize, f64) {
        mxm_measure::observe::worst_step(samples).expect("a rendered note is finite and has steps")
    }

    /// Renders `note` held, after the envelope has settled.
    fn sustained(note: u8, seconds: f32) -> Vec<f32> {
        let sample_rate = 48_000.0;
        let patch = sub_bass();
        let mut voice = Voice::new();
        voice.set_sample_rate(sample_rate);
        voice.note_on(id(note), Retrigger::Legato);

        let total = (sample_rate * seconds) as usize;
        let settle = (sample_rate * 0.5) as usize;
        let mut out = Vec::with_capacity(total);
        for i in 0..total {
            let y = voice.process(&patch, &Routing::new());
            if i >= settle {
                out.push(y);
            }
        }
        out
    }

    /// What pitch the sub actually lands on, which is not always the one you meant.
    ///
    /// This is what caught a **real** defect while the click turned out to be elsewhere: the Sub
    /// bass preset stacked 16' and a two-octave sub, so C3 sounded at 32 Hz and C2 at 16 Hz — below
    /// hearing. Range and sub shape both shift octaves, and it is easy to add them by accident.
    #[test]
    #[ignore = "prints the pitch the sub lands on"]
    fn what_pitch_does_the_sub_land_on() {
        for note in [48u8, 60, 72] {
            let samples = sustained(note, 1.0);
            // Count rising zero crossings: for a square that is one per cycle.
            let crossings = samples
                .windows(2)
                .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                .count();
            let seconds = samples.len() as f32 / 48_000.0;
            let played = 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0);
            eprintln!(
                "note {note:>3}  played {played:>7.1} Hz   heard {:>6.1} Hz",
                crossings as f32 / seconds
            );
        }
    }

    /// The worst discontinuity per note, for when something is suspected of clicking.
    ///
    /// It found nothing the one time it was used in anger — worst jump 0.002 across the keyboard —
    /// which is exactly what made it useful: it moved the search out of the plugin.
    #[test]
    #[ignore = "prints the worst jump per note"]
    fn the_worst_jump_per_note() {
        for note in 36u8..=84 {
            let samples = sustained(note, 1.5);
            let (at, jump) = worst_jump(&samples);
            eprintln!("note {note:>3}  jump {jump:.4}  at {at}");
        }
    }

    #[test]
    fn a_sustained_note_holds_without_a_discontinuity() {
        // Written to chase a reported click on the Sub bass preset, and kept although **the click
        // was Bluetooth headphones dropping out** — the clicks landed in different places on each
        // playback of the same recording, which is the signature of a transport dropout and not of
        // anything in the material.
        //
        // It is worth keeping because it says the thing that was in doubt: nothing in this signal
        // path introduces a discontinuity while a note is held. Recorded through a virtual cable it
        // was clean, and this is that same claim, cheaper and every time.
        for note in [48u8, 55, 60, 62, 64, 67, 72] {
            let samples = sustained(note, 2.0);
            let (at, jump) = worst_jump(&samples);

            // A 32 Hz square at 48 kHz moves about 0.004 per sample across its flat parts and takes
            // its band-limited edges over a couple of samples. Anything an order of magnitude past
            // that is an edge that was not band-limited — which is what a click is.
            assert!(
                jump < 0.35,
                "note {note} jumps {jump} between samples {at} and {}, which is a discontinuity",
                at + 1
            );
        }
    }

    use super::*;

    const FS: f32 = 48_000.0;

    #[test]
    fn a_pitch_expression_moves_the_pitch_exactly_as_bend_does() {
        // **The contract as an equivalence.** A per-note expression is semitones on the same pitch
        // as the bend, added at the same point — so seven semitones of one must render
        // bit-for-bit what seven semitones of the other renders. The two are separate fields so
        // the patch can say which moved, never because they mean different things to the pitch.
        let mut bent = sub_bass();
        bent.bend_semitones = 7.0;
        let mut expressed = sub_bass();
        expressed.expression_semitones = 7.0;

        let render = |patch: &Patch| {
            let mut voice = Voice::new();
            voice.set_sample_rate(48_000.0);
            voice.note_on(id(48), Retrigger::Legato);
            (0..12_000)
                .map(|_| voice.process(patch, &Routing::new()))
                .collect::<Vec<_>>()
        };

        assert_eq!(
            render(&bent),
            render(&expressed),
            "expression and bend must be the same semitones on the same pitch"
        );
    }

    #[test]
    fn the_sounding_note_is_the_one_an_expression_should_be_routed_to() {
        // Why `Voice::sounding` exists rather than a mirror in the plugin: releasing the newest
        // key hands the voice back to an older *held* one, so which note an arriving expression
        // belongs to is a question only the note stack can answer.
        let mut voice = Voice::new();
        voice.set_sample_rate(48_000.0);
        assert_eq!(voice.sounding(), None, "nothing held, nothing sounding");

        voice.note_on(id(60), Retrigger::Legato);
        voice.note_on(id(67), Retrigger::Legato);
        assert_eq!(
            voice.sounding().map(|n| n.note),
            Some(67),
            "the newest key is the one sounding"
        );

        voice.note_off(None, 0, 67, Retrigger::Legato);
        assert_eq!(
            voice.sounding().map(|n| n.note),
            Some(60),
            "and releasing it returns the voice to the key still held — which a plugin-side              mirror of \"the last note-on\" would have got wrong. That move happens with no              note-on to notice it, which is why the plugin drops the expression on this event              and not only on a new note."
        );

        voice.note_off(None, 0, 60, Retrigger::Legato);
        assert_eq!(
            voice.sounding(),
            None,
            "the last key released leaves nothing sounding — a release, not a move, which is              the case the plugin must *not* treat as the voice changing notes"
        );
    }

    fn id(note: u8) -> NoteId {
        NoteId {
            voice_id: None,
            channel: 0,
            note,
        }
    }

    fn voice() -> Voice {
        let mut v = Voice::new();
        v.set_sample_rate(FS);
        v
    }

    fn run(v: &mut Voice, p: &Patch, secs: f32) -> f32 {
        let n = (FS * secs) as usize;
        let mut peak = 0.0f32;
        for _ in 0..n {
            peak = peak.max(v.process(p, &Routing::new()).abs());
        }
        peak
    }

    #[test]
    fn silent_until_a_note_arrives() {
        let mut v = voice();
        let p = Patch::default();
        for _ in 0..10_000 {
            assert_eq!(v.process(&p, &Routing::new()), 0.0);
        }
    }

    #[test]
    fn a_note_produces_sound_and_release_returns_to_silence() {
        let mut v = voice();
        let p = Patch::default();
        v.note_on(id(60), Retrigger::Legato);
        assert!(run(&mut v, &p, 0.2) > 0.01, "note produced no sound");

        v.note_off(None, 0, 60, Retrigger::Legato);
        run(&mut v, &p, 2.0);
        assert!(!v.is_active(), "voice still active after release");
        for _ in 0..1_000 {
            assert_eq!(
                v.process(&p, &Routing::new()),
                0.0,
                "not silent after release"
            );
        }
    }

    #[test]
    fn all_levels_at_zero_is_digital_silence() {
        let mut v = voice();
        let p = Patch {
            levels: MixLevels::default(),
            ..Patch::default()
        };
        v.note_on(id(60), Retrigger::Legato);
        for _ in 0..20_000 {
            assert_eq!(v.process(&p, &Routing::new()), 0.0);
        }
    }

    #[test]
    fn repeated_presses_of_the_same_key_stack_separately() {
        let mut v = voice();
        let p = Patch::default();
        v.note_on(id(60), Retrigger::Legato);
        v.note_on(id(60), Retrigger::Legato);
        assert_eq!(v.stack.len(), 2);

        // Releasing one press must not end the note, because a key is still held.
        v.note_off(None, 0, 60, Retrigger::Legato);
        run(&mut v, &p, 0.05);
        assert!(v.is_active(), "note ended while a duplicate press was held");
        assert_eq!(v.stack.len(), 1);

        v.note_off(None, 0, 60, Retrigger::Legato);
        run(&mut v, &p, 2.0);
        assert!(!v.is_active());
    }

    #[test]
    fn voice_ids_disambiguate_repeated_notes() {
        let mut v = voice();
        let a = NoteId {
            voice_id: Some(1),
            channel: 0,
            note: 60,
        };
        let b = NoteId {
            voice_id: Some(2),
            channel: 0,
            note: 60,
        };
        v.note_on(a, Retrigger::Legato);
        v.note_on(b, Retrigger::Legato);

        // Release the *older* one by id: the sounding note must be untouched.
        v.note_off(Some(1), 0, 60, Retrigger::Legato);
        assert_eq!(v.stack.len(), 1);
        assert_eq!(v.stack.newest().unwrap().voice_id, Some(2));
    }

    #[test]
    fn releasing_the_top_note_falls_back_to_the_one_still_held() {
        let mut v = voice();
        let p = Patch::default();
        v.note_on(id(48), Retrigger::Legato);
        v.note_on(id(60), Retrigger::Legato);
        run(&mut v, &p, 0.05);

        v.note_off(None, 0, 60, Retrigger::Legato);
        assert_eq!(v.current_note, 48.0, "did not fall back to the held note");
        assert!(v.is_active(), "fallback should keep sounding");
    }

    #[test]
    fn legato_does_not_retrigger_but_always_does() {
        let p = Patch::default();

        let mut legato = voice();
        legato.note_on(id(48), Retrigger::Legato);
        run(&mut legato, &p, 0.5);
        let level_before = legato.env.level();
        legato.note_on(id(60), Retrigger::Legato);
        legato.process(&p, &Routing::new());
        assert!(
            (legato.env.level() - level_before).abs() < 0.05,
            "legato retriggered the envelope"
        );

        let mut always = voice();
        always.note_on(id(48), Retrigger::Always);
        run(&mut always, &p, 0.5);
        always.note_on(id(60), Retrigger::Always);
        assert_eq!(
            always.env.stage(),
            crate::envelope::Stage::Attack,
            "Always mode did not retrigger"
        );
    }

    #[test]
    fn stack_overflow_drops_the_oldest_and_keeps_the_sounding_note() {
        let mut v = voice();
        for n in 0..(MAX_HELD_NOTES + 4) {
            v.note_on(id(40 + n as u8), Retrigger::Legato);
        }
        assert_eq!(v.stack.len(), MAX_HELD_NOTES);
        let newest = v.stack.newest().unwrap();
        assert_eq!(newest.note, 40 + (MAX_HELD_NOTES + 3) as u8);
        assert_eq!(v.current_note, newest.note as f32);
    }

    #[test]
    fn choke_targets_one_note_and_leaves_the_others() {
        let mut v = voice();
        v.note_on(id(48), Retrigger::Legato);
        v.note_on(id(60), Retrigger::Legato);

        // Choking a held-but-not-sounding note must not disturb the sounding one.
        v.choke(None, 0, 48);
        assert_eq!(v.stack.len(), 1);
        assert_eq!(v.current_note, 60.0);
        assert!(v.env.is_active());

        v.choke(None, 0, 60);
        assert!(v.stack.is_empty());
        assert!(
            !v.env.is_active(),
            "choking the last note should silence it"
        );
    }

    #[test]
    fn cc120_is_immediate_and_cc123_releases() {
        let p = Patch::default();

        let mut sound_off = voice();
        sound_off.note_on(id(60), Retrigger::Legato);
        run(&mut sound_off, &p, 0.2);
        sound_off.all_sound_off();
        assert!(!sound_off.env.is_active(), "CC120 should cut immediately");

        let mut notes_off = voice();
        notes_off.note_on(id(60), Retrigger::Legato);
        run(&mut notes_off, &p, 0.2);
        notes_off.all_notes_off();
        assert_eq!(
            notes_off.env.stage(),
            crate::envelope::Stage::Release,
            "CC123 should release rather than cut"
        );
    }

    /// **A panic clears the filter and the DC blocker ahead of it**, not only the envelope. The
    /// voice reports itself inactive the moment All Sound Off lands, so the plugin answers `Normal`
    /// and a host may stop calling right there; whatever the ladder and the blocker held would then
    /// be where the next note starts. Two voices play the same pitches for the same time, so the
    /// free-running oscillator and LFO — which a panic does not touch — are in step, but through
    /// different mixer levels, cutoffs, resonances and envelopes. Both are panicked, neither is
    /// processed again (the host slept), and the same note must render bit-identically. The panic
    /// retires every press, so a late release for a pre-panic key moves nothing.
    #[test]
    fn panic_clears_the_filter_and_dc_blocker_before_a_host_sleeps() {
        let bright = Patch {
            levels: MixLevels {
                saw: 1.0,
                pulse: 1.0,
                sub: 0.0,
                noise: 0.5,
            },
            cutoff_hz: 6_000.0,
            resonance: 0.95,
            sustain: 1.0,
            ..Patch::default()
        };
        let dark = Patch {
            levels: MixLevels {
                saw: 0.3,
                pulse: 0.0,
                sub: 0.0,
                noise: 0.0,
            },
            cutoff_hz: 250.0,
            resonance: 0.2,
            sustain: 0.4,
            ..Patch::default()
        };
        let mut bright_history = voice();
        let mut dark_history = voice();
        for (v, p) in [(&mut bright_history, &bright), (&mut dark_history, &dark)] {
            v.note_on(id(48), Retrigger::Legato);
            run(v, p, 0.05);
            v.note_on(id(55), Retrigger::Legato);
            run(v, p, 0.05);
            v.all_sound_off();
            assert!(v.sounding().is_none());
            assert!(
                !v.is_active() && v.tail_samples(p.release_s) == 0,
                "the plugin reports Normal at once, so the host may stop calling here"
            );
        }

        let p = Patch::default();
        let mut wakes = [Vec::new(), Vec::new()];
        for (v, wake) in [&mut bright_history, &mut dark_history]
            .into_iter()
            .zip(wakes.iter_mut())
        {
            v.note_on(id(60), Retrigger::Legato);
            v.note_off(None, 0, 55, Retrigger::Legato);
            assert_eq!(
                v.sounding(),
                Some(id(60)),
                "a pre-panic release moves nothing"
            );
            *wake = (0..512).map(|_| v.process(&p, &Routing::new())).collect();
        }
        assert!(
            wakes[0].iter().any(|y| y.abs() > 0.01),
            "the new note sounds"
        );
        let first_difference = wakes[0]
            .iter()
            .zip(&wakes[1])
            .position(|(a, b)| a.to_bits() != b.to_bits())
            .map(|i| (i, wakes[0][i], wakes[1][i]));
        assert_eq!(
            first_difference, None,
            "the ladder or the DC blocker leaked through All Sound Off"
        );
    }

    #[test]
    fn first_note_does_not_glide_from_a_stale_pitch() {
        let mut v = voice();
        let p = Patch {
            glide_time_s: 1.0,
            ..Patch::default()
        };

        v.note_on(id(36), Retrigger::Legato);
        v.process(&p, &Routing::new());
        assert_eq!(v.glided(), 36.0, "a first note snaps");

        // **And snaps with the range switch**: it used to snap to the bare key, so a first note at
        // 16' slid an octave down from it.
        let mut v = voice();
        let p = Patch {
            range_offset: -12.0,
            ..p
        };
        v.note_on(id(36), Retrigger::Legato);
        v.process(&p, &Routing::new());
        assert_eq!(v.glided(), 24.0, "a first note at 16' snaps to its octave");
    }

    /// **A glide lands exactly on its note.** The pitch form, `target + (glide − target) × coef`,
    /// stopped moving once a step fell under half an ulp of the target: at half a second it came to
    /// rest 5 cents short of middle C at 48 kHz and 9 at 96 kHz, until the next note.
    ///
    /// Falsified before trusted: with the pitch form back, it rests at 59.954 at 48 kHz.
    #[test]
    fn a_glide_lands_exactly_on_its_note() {
        for rate in [48_000.0f32, 96_000.0] {
            let p = Patch {
                glide_time_s: 0.5,
                ..Patch::default()
            };
            let mut v = Voice::new();
            v.set_sample_rate(rate);
            v.note_on(id(36), Retrigger::Legato);
            v.process(&p, &Routing::new());
            v.note_on(id(60), Retrigger::Legato);
            for _ in 0..(10.0 * rate) as usize {
                v.process(&p, &Routing::new());
            }
            assert_eq!(v.glided(), 60.0, "at {rate} Hz the glide rests short");
        }
    }

    #[test]
    fn glide_slides_between_notes() {
        let mut v = voice();
        let p = Patch {
            glide_time_s: 0.5,
            ..Patch::default()
        };

        v.note_on(id(36), Retrigger::Legato);
        run(&mut v, &p, 0.1);
        v.note_on(id(60), Retrigger::Legato);
        v.process(&p, &Routing::new());
        assert!(v.glided() < 40.0, "glide jumped straight to the new note");
        run(&mut v, &p, 3.0);
        assert!(
            (v.glided() - 60.0).abs() < 0.5,
            "glide never arrived: {}",
            v.glided()
        );
    }

    #[test]
    fn reset_leaves_no_tail() {
        let mut v = voice();
        let p = Patch::default();
        v.note_on(id(60), Retrigger::Legato);
        run(&mut v, &p, 0.3);

        v.reset();
        assert!(!v.is_active());
        for _ in 0..5_000 {
            assert_eq!(v.process(&p, &Routing::new()), 0.0, "state survived reset");
        }
    }

    /// **With the machine's own wiring at full depth**, not merely at extreme knob settings.
    ///
    /// The five modulation depths used to be `Patch` fields and were set to one here; they are
    /// routes now, so the extremes have to be set where they live or this test quietly stopped
    /// exercising any modulation at all when they moved.
    #[test]
    fn output_is_finite_and_bounded_under_extreme_settings() {
        let mut extreme = Routing::init();
        for (t, src) in crate::routing::INIT_PRESENT {
            extreme.amounts[t][src] = 1.0;
        }

        for fs in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
            let mut v = Voice::new();
            v.set_sample_rate(fs);
            v.set_topology(&extreme);
            let p = Patch {
                levels: MixLevels {
                    saw: 1.0,
                    pulse: 1.0,
                    sub: 1.0,
                    noise: 1.0,
                },
                resonance: 1.0,
                lfo_rate_hz: 30.0,
                attack_s: 0.001,
                decay_s: 0.001,
                release_s: 0.001,
                output_gain: 2.0,
                ..Patch::default()
            };

            for note in [0u8, 60, 127] {
                v.note_on(id(note), Retrigger::Always);
                for _ in 0..(fs as usize / 10) {
                    let y = v.process(&p, &extreme);
                    assert!(y.is_finite(), "non-finite at note {note}, {fs} Hz");
                    assert!(y.abs() < 32.0, "runaway output {y}");
                }
                v.note_off(None, 0, note, Retrigger::Always);
            }
        }
    }

    #[test]
    fn note_off_for_a_note_that_was_never_pressed_is_ignored() {
        let mut v = voice();
        let p = Patch::default();
        v.note_on(id(60), Retrigger::Legato);
        run(&mut v, &p, 0.1);
        v.note_off(None, 0, 72, Retrigger::Legato);
        assert!(v.is_active(), "an unrelated note-off ended the note");
        assert_eq!(v.stack.len(), 1);
    }

    #[test]
    fn tail_is_zero_once_idle() {
        let v = voice();
        assert_eq!(v.tail_samples(0.2), 0);
    }
}

#[cfg(test)]
mod pitch_tests {
    use super::*;

    const FS: f32 = 48_000.0;

    /// Fundamental via zero crossings of a raw saw with the filter wide open and a
    /// flat envelope, so nothing colours the measurement.
    fn measure_note(note: u8, range_offset: f32, tune_cents: f32) -> f32 {
        let mut v = Voice::new();
        v.set_sample_rate(FS);
        let p = Patch {
            range_offset,
            tune_cents,
            levels: MixLevels {
                saw: 1.0,
                pulse: 0.0,
                sub: 0.0,
                noise: 0.0,
            },
            cutoff_hz: 20_000.0,
            resonance: 0.0,
            attack_s: 0.001,
            decay_s: 0.001,
            sustain: 1.0,
            ..Patch::default()
        };
        v.note_on(
            NoteId {
                voice_id: None,
                channel: 0,
                note,
            },
            Retrigger::Always,
        );
        // Let the envelope settle first.
        for _ in 0..(FS as usize / 10) {
            v.process(&p, &Routing::new());
        }

        // Sub-sample interpolated zero crossings. Counting whole crossings over a
        // fixed window quantises badly at low frequencies -- at 32.7 Hz, two
        // seconds is only 65 cycles, so the count is good to about +/-13 cents,
        // which is far too coarse to tell a real tuning error from rounding.
        // Timing the first and last crossing precisely removes that entirely.
        let n = (FS * 4.0) as usize;
        let (mut first, mut last, mut count) = (None::<f64>, 0.0f64, 0usize);
        let mut prev = 0.0f32;
        for i in 0..n {
            let y = v.process(&p, &Routing::new());
            if prev <= 0.0 && y > 0.0 {
                // Linear interpolation of the crossing instant between the two
                // samples that straddle zero.
                let frac = (-prev / (y - prev)) as f64;
                let t = (i as f64 - 1.0 + frac) / FS as f64;
                if first.is_none() {
                    first = Some(t);
                } else {
                    last = t;
                    count += 1;
                }
            }
            prev = y;
        }
        let first = first.expect("no zero crossings: the voice produced no signal");
        (count as f64 / (last - first)) as f32
    }

    fn expected(note: u8, semitones: f32) -> f32 {
        440.0 * ((note as f32 + semitones - 69.0) / 12.0).exp2()
    }

    #[test]
    fn voice_plays_the_note_it_is_given() {
        for note in [24u8, 36, 48, 60, 72] {
            let measured = measure_note(note, 0.0, 0.0);
            let want = expected(note, 0.0);
            let cents = 1200.0 * (measured / want).log2();
            assert!(
                cents.abs() < 2.0,
                "MIDI {note}: measured {measured:.2} Hz, expected {want:.2} Hz ({cents:+.1} cents)"
            );
        }
    }

    #[test]
    fn range_switch_transposes_by_exact_octaves() {
        for (offset, label) in [(-12.0, "16'"), (0.0, "8'"), (12.0, "4'"), (24.0, "2'")] {
            let measured = measure_note(48, offset, 0.0);
            let want = expected(48, offset);
            let cents = 1200.0 * (measured / want).log2();
            assert!(
                cents.abs() < 2.0,
                "{label}: measured {measured:.2} Hz, expected {want:.2} Hz ({cents:+.1} cents)"
            );
        }
    }

    #[test]
    fn tune_shifts_by_the_requested_cents() {
        let base = measure_note(48, 0.0, 0.0);
        for cents in [-100.0f32, -50.0, 50.0, 100.0] {
            let measured = measure_note(48, 0.0, cents);
            let shift = 1200.0 * (measured / base).log2();
            assert!(
                (shift - cents).abs() < 3.0,
                "tune {cents}: measured shift {shift:+.1} cents"
            );
        }
    }
}

/// The read-only accessors the editor uses for brief §8's envelope display.
#[cfg(test)]
mod telemetry_tests {
    use super::*;

    /// The accessors exist for the editor's envelope display, and they are only useful if they
    /// agree with what the audio path actually used. A telemetry value that lags by a block is a
    /// display that lags; one that reports a different number is a display that lies.
    #[test]
    fn envelope_telemetry_tracks_the_rendered_envelope() {
        let mut voice = Voice::new();
        voice.set_sample_rate(48_000.0);

        let patch = Patch {
            attack_s: 0.05,
            decay_s: 0.1,
            sustain: 0.6,
            release_s: 0.1,
            ..Patch::default()
        };

        assert_eq!(voice.env_stage(), crate::envelope::Stage::Idle);
        assert_eq!(voice.env_level(), 0.0);

        voice.note_on(
            NoteId {
                voice_id: None,
                channel: 0,
                note: 60,
            },
            Retrigger::Legato,
        );

        // Partway into the attack: rising, and not yet at the top.
        for _ in 0..1_000 {
            voice.process(&patch, &Routing::new());
        }
        let rising = voice.env_level();
        assert_eq!(voice.env_stage(), crate::envelope::Stage::Attack);
        assert!(rising > 0.0 && rising < 1.0, "attack level was {rising}");

        // Long enough to settle on sustain, which is a value the caller chose - so this checks the
        // accessor reports the DSP's number rather than one of its own.
        for _ in 0..48_000 {
            voice.process(&patch, &Routing::new());
        }
        assert_eq!(voice.env_stage(), crate::envelope::Stage::Sustain);
        assert!(
            (voice.env_level() - patch.sustain).abs() < 1e-3,
            "sustain telemetry {} does not match the patch's {}",
            voice.env_level(),
            patch.sustain
        );
    }

    /// Reading telemetry must not disturb the thing it measures. Two reads with no `process`
    /// between them must be identical, or the accessor has become a side effect.
    #[test]
    fn reading_telemetry_changes_nothing() {
        let mut voice = Voice::new();
        voice.set_sample_rate(48_000.0);
        let patch = Patch::default();
        voice.note_on(
            NoteId {
                voice_id: None,
                channel: 0,
                note: 60,
            },
            Retrigger::Legato,
        );
        for _ in 0..500 {
            voice.process(&patch, &Routing::new());
        }

        let (level, stage) = (voice.env_level(), voice.env_stage());
        for _ in 0..10 {
            assert_eq!(voice.env_level(), level);
            assert_eq!(voice.env_stage(), stage);
        }
    }
}
