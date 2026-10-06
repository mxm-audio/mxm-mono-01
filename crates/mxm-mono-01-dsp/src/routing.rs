//! What mxm-mono-01 can modulate, and with what.
//!
//! `plans/plan-modulation-routing.md` §2 and §7.4. The shared machinery is
//! [`mxm_modulation`]; this module is the instrument's own declaration — its **source list**, its
//! **target list**, and each target's **full scale**. Those three are per instrument by design and
//! are the one place a copy still says something about the machine it copies.
//!
//! # Everything the machine did not have is the collection's standard
//!
//! Key, Velocity, Wheel, Pressure and Bend mean what they mean on every instrument, and a route the
//! SH-101 never had reaches what it reaches on every instrument ([`mxm_modulation::standard`];
//! `plans/plan-modulation-standard.md`). Velocity is `v − 1`, so a route from it does nothing at the
//! hardest note; Bend is the lever, not the pitch it bends; Amplitude is the standard factor.

use mxm_modulation::standard::{self, Law, Offer, Performance, reach};
use mxm_modulation::{Compacted, SourceFrame};

use crate::voice::{FILTER_ENV_OCTAVES, FILTER_LFO_OCTAVES, PWM_WIDTH_SWING, VCO_LFO_SEMITONES};

/// Every source this instrument can route, in **declared evaluation order**.
///
/// The order is load-bearing rather than cosmetic: a route whose source comes earlier in it reads
/// *this* sample, and one whose source comes later reads *last* sample. That is the unit delay, and
/// it is what makes a player-made cycle finite. The audio sources are last because they are produced
/// last, so a route from one of them into pitch or cutoff is a **backward** route and is one sample
/// late — audible as a comb at audio rate, and declared here rather than discovered.
pub mod source {
    /// The LFO, free-running. Bipolar.
    pub const LFO: usize = 0;
    /// The envelope. Unipolar.
    pub const ENVELOPE: usize = 1;
    /// The played note **after glide**, as a signed distance from middle C over five octaves
    /// (`standard::key`). The range switch is the oscillator's, not the keyboard's.
    pub const KEY: usize = 2;
    /// The velocity of the press that last triggered the envelope, `v − 1`
    /// (`standard::velocity`): zero at the hardest note.
    pub const VELOCITY: usize = 3;
    /// The mod wheel, CC 1. Unipolar.
    pub const WHEEL: usize = 4;
    /// Channel pressure. Unipolar.
    pub const PRESSURE: usize = 5;
    /// The bender's lever, −1…+1 (`standard::bend`) — not the pitch it bends, which is the bend
    /// range's.
    pub const BEND: usize = 6;
    /// The main oscillator's sawtooth, at audio rate.
    pub const SAW: usize = 7;
    /// The main oscillator's pulse, at audio rate.
    pub const PULSE: usize = 8;
    /// The sub oscillator, at audio rate.
    pub const SUB: usize = 9;
    /// The noise generator, at audio rate.
    pub const NOISE: usize = 10;
}

/// How many sources the instrument declares.
pub const SOURCES: usize = 11;

/// Their names, in source order, for the interface and for accessibility.
pub const SOURCE_NAMES: [&str; SOURCES] = [
    "LFO", "Envelope", "Key", "Velocity", "Wheel", "Pressure", "Bend", "Saw", "Pulse", "Sub",
    "Noise",
];

/// Every target this instrument declares.
///
/// **Deliberate rather than maximal** (the owner): the places worth modulating, not every continuous
/// parameter. A target nobody would reach for costs a full column of permanent parameters, so the
/// list is chosen once and recorded rather than swept.
pub mod target {
    /// Oscillator pitch, summed in semitones.
    pub const PITCH: usize = 0;
    /// Pulse width, summed as a width offset.
    pub const PULSE_WIDTH: usize = 1;
    /// Filter cutoff, summed in octaves.
    pub const CUTOFF: usize = 2;
    /// The amplifier. **Multiplicative on the envelope**, not additive — see [`Routing`].
    pub const AMPLITUDE: usize = 3;
}

/// How many targets the instrument declares.
pub const TARGETS: usize = 4;

/// Their names, in target order.
pub const TARGET_NAMES: [&str; TARGETS] = ["Pitch", "Pulse width", "Cutoff", "Amplitude"];

/// The key source's unit, in semitones: a voice publishes `(note − 60) / 60`.
pub const KEY_UNIT_SEMITONES: f32 = 60.0;

/// Which of the standard's performance sources each source is — `None` for the machine's own
/// generators and the voice's audio, which keep their own meaning.
pub const PERFORMANCE: [Option<Performance>; SOURCES] = [
    None,
    None,
    Some(Performance::Key),
    Some(Performance::Velocity),
    Some(Performance::Wheel),
    Some(Performance::Pressure),
    Some(Performance::Bend),
    None,
    None,
    None,
    None,
];

/// Each target's law, for the standard's offer: three sums and the amplitude factor.
pub const LAW: [Law; TARGETS] = [Law::Sum, Law::Sum, Law::Sum, Law::Factor];

/// Whether **the SH-101 itself** has this path, so its reach is the machine's: the five
/// [`INIT_PRESENT`] routes, and the pulse width from the envelope, which the PWM source switch
/// offered beside the LFO.
#[must_use]
pub const fn machine(target: usize, source: usize) -> bool {
    if target == target::PULSE_WIDTH && source == source::ENVELOPE {
        return true;
    }
    let mut i = 0;
    while i < INIT_PRESENT.len() {
        if INIT_PRESENT[i].0 == target && INIT_PRESENT[i].1 == source {
            return true;
        }
        i += 1;
    }
    false
}

/// Whether and how a pair is offered — `standard::offer`. Every target here sums or scales, so
/// every pair is offered on both halves.
#[must_use]
pub const fn offer(target: usize, source: usize) -> Offer {
    standard::offer(LAW[target], PERFORMANCE[source], machine(target, source))
}

/// Each target's full scale, in the target's own domain, at an amount of one — **per route, not
/// per target**.
///
/// A route is `amount × source × full_scale`, summed in that domain and converted once, which is
/// what these voices already did with the constant written into each expression.
///
/// # Why the table has a column per source
///
/// Because the machine's own wiring is not uniform, and `plans/plan-modulation-routing.md` §5 takes
/// the conservative form: **a route the instrument itself wires keeps the scale it always had.** The
/// SH-101's filter envelope reaches six octaves and its filter LFO four, from the same cutoff; the
/// key follows the keyboard at one octave per octave. Declaring one scale per target would rescale
/// all three, which moves every stored value's meaning and every pinned digest for no musical gain.
///
/// So the machine's own pairs carry their reach, and **every pair the SH-101 did not have takes the
/// collection's standard reach** (`standard::reach`): an octave of pitch, twelve semitones per
/// octave of keyboard from Key, four octaves of cutoff, 45 % of width, the whole amplitude factor,
/// and a fifth of a linear reach per octave from Key. Before the standard, an added pitch route
/// took the LFO's seven semitones and an added cutoff route the envelope's six octaves. Each
/// amount reads in its target's own unit (`mxm_modulation_params::reading`).
pub const FULL_SCALE: [[f32; SOURCES]; TARGETS] = {
    let key_linear = reach::KEY_LINEAR_FRACTION_PER_OCTAVE;
    // Semitones. The VCO's MOD slider reached `VCO_LFO_SEMITONES`, so an old LFO-to-pitch depth is
    // the same number on the route that replaced it; Key tracks an octave per octave, so −100 %
    // holds one pitch across the keyboard.
    let mut pitch = [reach::PITCH_SEMITONES; SOURCES];
    pitch[source::LFO] = VCO_LFO_SEMITONES;
    pitch[source::KEY] =
        standard::key_scale(reach::KEY_PITCH_SEMITONES_PER_OCTAVE, KEY_UNIT_SEMITONES);
    // A pulse-width offset, matching the `± 0.45` the PWM path already swung from either source the
    // switch chose — which is the standard's reach too.
    let mut width = [reach::WIDTH; SOURCES];
    width[source::LFO] = PWM_WIDTH_SWING;
    width[source::ENVELOPE] = PWM_WIDTH_SWING;
    width[source::KEY] = standard::key_scale(reach::WIDTH * key_linear, KEY_UNIT_SEMITONES);
    // Octaves: the VCF's ENV (six), MOD (four) and KYBD sliders are the machine's. KYBD's five is
    // arithmetic rather than taste: the key source is published over five octaves either side of
    // middle C, so `× 5` is one octave of cutoff per octave of keyboard.
    let mut cutoff = [reach::OCTAVES; SOURCES];
    cutoff[source::ENVELOPE] = FILTER_ENV_OCTAVES;
    cutoff[source::LFO] = FILTER_LFO_OCTAVES;
    cutoff[source::KEY] = 5.0;
    // The amplitude factor's swing: one route can silence the amplifier or double it.
    let mut amplitude = [reach::AMPLITUDE; SOURCES];
    amplitude[source::KEY] = standard::key_scale(reach::AMPLITUDE * key_linear, KEY_UNIT_SEMITONES);
    [pitch, width, cutoff, amplitude]
};

/// The routes **the machine itself wires**, present in the init patch at zero depth.
///
/// The owner's requirement from the first sentence of the request: *the sources in the init patch
/// should be the ones that follow the original signal flow*. These five are the SH-101's own
/// modulation paths, and they used to be five knobs — `vcolfo`, `pwmdepth`, `filterenv`,
/// `filterlfo`, `keytrack`. They are routes now, so a player can take them off, invert them, or put
/// something else in their place, and a fresh instance is still the instrument it always was because
/// **every one of them starts at zero**, exactly as those knobs did.
pub const INIT_PRESENT: [(usize, usize); 5] = [
    (target::PITCH, source::LFO),
    (target::PULSE_WIDTH, source::LFO),
    (target::CUTOFF, source::ENVELOPE),
    (target::CUTOFF, source::LFO),
    (target::CUTOFF, source::KEY),
];

/// Which sources are live into which targets, and how much of each.
///
/// **Presence is what the DSP reads.** An absent route contributes nothing whatever its amount
/// holds, which is what makes removing a source one parameter write and re-adding it restore the
/// depth the player last set.
#[derive(Debug, Clone, Copy)]
pub struct Routing {
    /// Per target, per source: whether that route exists.
    pub present: [[bool; SOURCES]; TARGETS],
    /// Per target, per source: how much, signed, as a **fraction of that route's full scale**.
    ///
    /// The scale is applied by [`Graph::sum`] rather than folded in here, because
    /// `(amount × source) × scale` is the instruction sequence these voices executed before they had
    /// routing and the other order does not round the same way.
    ///
    /// Only a **live** route's amount is filled by the caller each sample; an absent one is left at
    /// zero and never read, which is what leaves its stored depth alone.
    pub amounts: [[f32; SOURCES]; TARGETS],
    /// The live pairs, `(target, source)`, compacted by [`Routing::compact`].
    ///
    /// **The per-sample amount read visits this, not the presence grid.** Walking all forty-four
    /// pairs every sample to find the five that are live cost 37 ns of the owner's cost gate — more
    /// than everything else the routing added put together. Topology is discrete and changes only on
    /// a parameter event, so the list is built once per interval like every other compaction here.
    live: [(u8, u8); TARGETS * SOURCES],
    live_len: usize,
}

impl Default for Routing {
    fn default() -> Self {
        Self::new()
    }
}

impl Routing {
    /// Nothing routed anywhere.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            present: [[false; SOURCES]; TARGETS],
            amounts: [[0.0; SOURCES]; TARGETS],
            live: [(0, 0); TARGETS * SOURCES],
            live_len: 0,
        }
    }

    /// Rebuilds [`Routing::live`] from [`Routing::present`]. **Once per interval, never per
    /// sample**, and every caller that changes `present` owes this before the next render.
    pub fn compact(&mut self) {
        self.live_len = 0;
        for (t, target) in self.present.iter().enumerate() {
            for (s, &on) in target.iter().enumerate() {
                if on {
                    self.live[self.live_len] = (t as u8, s as u8);
                    self.live_len += 1;
                }
            }
        }
    }

    /// The live pairs, `(target, source)`, as compacted.
    #[inline]
    #[must_use]
    pub fn live(&self) -> &[(u8, u8)] {
        &self.live[..self.live_len]
    }

    /// The machine's own wiring, at zero depth — [`INIT_PRESENT`]. What the init patch holds.
    #[must_use]
    pub const fn init() -> Self {
        let mut routing = Self::new();
        let mut i = 0;
        while i < INIT_PRESENT.len() {
            let (t, s) = INIT_PRESENT[i];
            routing.present[t][s] = true;
            routing.live[i] = (t as u8, s as u8);
            i += 1;
        }
        routing.live_len = INIT_PRESENT.len();
        routing
    }
}

/// The voice's routing state: one frame, and one compacted list per target.
///
/// Compaction runs **once per processing interval**, not per sample, because topology is discrete
/// and changes only on a parameter event. The per-sample loop then runs over the live routes rather
/// than over every source.
#[derive(Debug, Clone)]
pub struct Graph {
    frame: SourceFrame<SOURCES>,
    live: [Compacted<SOURCES>; TARGETS],
    /// Which sources any live route actually reads, cached at [`Graph::set_topology`].
    ///
    /// **A source nothing reads is not published.** The init patch wires three sources into five
    /// routes, and publishing the other eight cost a finite check, a clamp and two array stores each
    /// for a value no sum would ever look at — eight elevenths of the frame's per-sample work, for
    /// nothing. The tick still happens (an LFO's phase must advance whether or not anyone is
    /// listening); what is skipped is the *publication*.
    needed: [bool; SOURCES],
    /// Whether any route at all is live, cached at [`Graph::set_topology`].
    ///
    /// The whole point of the routing being able to cost nothing: a voice with nothing routed skips
    /// opening the frame, publishing any source and taking any sum, so it runs the arithmetic it ran
    /// before routing existed.
    any: bool,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    /// An empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frame: SourceFrame::new(),
            live: [const { Compacted::new() }; TARGETS],
            needed: [false; SOURCES],
            any: false,
        }
    }

    /// Rebuilds which routes are live. Call once per interval, never per sample.
    ///
    /// **A source that becomes needed starts from silence.** While nothing read it, nothing
    /// published it, so its slot still holds whatever it held the last time something did — which
    /// may be from a different phrase entirely. A backward route added to a running voice would
    /// then read that ancient value for exactly one sample. Clearing the slot makes the first
    /// sample a deterministic zero instead: bounded, the same however long the source went unread,
    /// and therefore independent of how the host split its buffers. `mxm-mono-pr1` found this;
    /// `a_source_that_becomes_needed_starts_from_silence_not_from_an_old_phrase` is the proof here.
    pub fn set_topology(&mut self, routing: &Routing) {
        for (live, present) in self.live.iter_mut().zip(routing.present.iter()) {
            live.build(present);
        }
        self.any = self.live.iter().any(|l| !l.is_empty());
        let was_needed = self.needed;
        self.needed = [false; SOURCES];
        for present in routing.present.iter() {
            for (needed, &on) in self.needed.iter_mut().zip(present.iter()) {
                *needed |= on;
            }
        }
        for (source, (&needed, &before)) in self.needed.iter().zip(was_needed.iter()).enumerate() {
            if needed && !before {
                self.frame.clear(source);
            }
        }
    }

    /// Whether anything reads this source, so a caller can skip producing a value for it.
    #[inline]
    #[must_use]
    pub fn needs(&self, source: usize) -> bool {
        self.needed[source]
    }

    /// Whether anything is routed at all.
    #[inline]
    #[must_use]
    pub fn any_live(&self) -> bool {
        self.any
    }

    /// Opens a sample.
    #[inline]
    pub fn begin_sample(&mut self) {
        self.frame.begin_sample();
    }

    /// Publishes a source's value for this sample, **if anything reads it**.
    ///
    /// The guard is the cheap half of the cost model: one array load and a branch against a finite
    /// check, a clamp and two stores.
    #[inline]
    pub fn write(&mut self, source: usize, value: f32) {
        if self.needed[source] {
            self.frame.write(source, value);
        }
    }

    /// This target's summed modulation, in its own domain.
    #[inline]
    #[must_use]
    pub fn sum(&self, target: usize, routing: &Routing) -> f32 {
        // The bound is generous rather than tight: each target applies its own limit where it
        // matters — the filter clamps its cutoff, the oscillator clamps its width,
        // `standard::amplitude_factor` the amplitude sum at one — and a bound here as well would
        // silently narrow what a player can reach.
        mxm_modulation::sum_scaled(
            &self.frame,
            &self.live[target],
            &routing.amounts[target],
            &FULL_SCALE[target],
            64.0,
        )
    }

    /// Whether any route is live into that target, which is what lets a caller skip the work.
    #[inline]
    #[must_use]
    pub fn is_empty(&self, target: usize) -> bool {
        self.live[target].is_empty()
    }

    /// Clears the frame, leaving no tail between renders.
    pub fn reset(&mut self) {
        self.frame.reset();
    }

    /// What the frame holds for one source, for tests.
    #[cfg(test)]
    pub(crate) fn read_for_test(&self, source: usize) -> f32 {
        self.frame.read(source)
    }
}
