# NOTES.md — crates/mxm-mono-01-dsp

The detail behind this folder's AGENTS.md: history, measurements, rationale and worked examples. AGENTS.md is the contract; this file is the reference it links to.

## Ownership: the spikes live in dsp-lab

The measurement harnesses behind `docs/oscillators/` and `docs/modulation/` — `osc_spike.rs` and
`mod_spike.rs` — live in [`../dsp-lab/`](https://github.com/mxm-audio/mxm-tools/blob/main/crates/dsp-lab/AGENTS.md), because they serve
collection-level references rather than this plugin. The examples that remain here all verify
*this* crate's shipped DSP.

## Read-only accessors are allowed; nothing else looks outward

`Voice::env_level` and `Voice::env_stage` exist because mxm-mono-01's brief (§8) requires an envelope
display with a position indicator, and there was no way to see the value: `env` is private, and
`process` computes the level into a local and returns only the output sample. Atomics can carry a
value across a thread; they cannot invent one that is never published.

The rules that keep this from becoming a hole in the crate's independence:

- **Read-only, and additive.** No new state, no branch in the audio path, no allocation. The DSP
  does not know anyone is looking.
- **The audio thread reads them**, once per block, and publishes to an atomic owned by the plugin.
  A UI thread must never call them — `Voice` is `&mut`-owned by the processor, and sharing it would
  need a lock the audio callback then has to take.
- **The transport is not this crate's business.** `plugins/mxm-mono-01/src/telemetry.rs` owns it, and
  this crate stays framework-free and zero-dependency.
- **This is not a licence to add outputs.** A new accessor needs the same *shape* of argument the
  envelope one had: something outside this crate must do its job, the value it needs exists only in
  here, and there is no other source.

**`Voice::sounding` is the second accessor, and it is not telemetry.** It returns the `NoteId` the
voice is actually playing, and its argument is routing rather than display: a CLAP per-note pitch
expression names the note it belongs to, so the plugin has to decide whether an arriving one is
about the note being played. After a note-off that answer is this crate's to give — releasing the
newest key returns the voice to an older *held* key under the priority rules, and `stack` is where
that decision lives. A mirror in the plugin would be a second copy of a stored fact, free to
disagree with it the moment a key is released under a chord.
`the_sounding_note_is_the_one_an_expression_should_be_routed_to` is what says so.

## A source that becomes needed starts from silence

`routing::Graph` publishes only the sources some live route reads, so an unread slot keeps whatever
it last held. `Graph::set_topology` therefore **clears every source that has just become needed**
(`mxm_modulation::SourceFrame::clear`): a backward route added to a running voice reads a
deterministic zero on its first sample rather than a value from an earlier phrase, however long the
source went unread. `a_source_that_becomes_needed_starts_from_silence_not_from_an_old_phrase` holds
it in both gaps — another route ticking the frame, and nothing routed at all.

**The declared evaluation order is proved by rendering, not by a table.** The instrument's audio is
published after the pitch and width sums and before the cutoff sum, so a route from it is one sample
late into pitch and on time into cutoff.
`a_route_from_the_instruments_audio_is_a_sample_late_into_pitch_and_on_time_into_cutoff` renders
both against an unrouted voice; moving the publication in `Voice::process` fails it.

## The performance sources are the collection's standard

Key, Velocity, Wheel, Pressure and Bend are published through `mxm_modulation::standard`, so each
is zero at its rest and means what it means on every instrument
(`plans/plan-modulation-standard.md`): **Key is the keyboard's glided note**, published after the
glide and without the range switch; **Velocity is `v − 1`**; **Bend is the lever** (`Patch::bend`),
where it used to be the bent semitones over twelve, so its reach no longer depended on the bend
range. **Amplitude is `standard::amplitude_factor`**, bounded at one where the sum used to be bounded
at 64 — identical for any single route. A path the SH-101 has keeps its reach; every added path
takes the standard's (`routing::FULL_SCALE`). `conformance.rs`'s `Declared` runs the standard's
checks over the real tables and a real graph — `every_pair_means_what_the_standard_says`,
`a_voice_publishes_what_the_standard_says`, `key_follows_the_glide_and_not_the_range_switch` and
`after_a_release_no_performance_route_holds_a_note_open`, each falsified once — and the plugin's
tests reuse it through the `conformance` feature, which only `[dev-dependencies]` enable.

## A panic clears the recursive stages, not only the envelope

`Voice::all_sound_off` (CC 120) retires every press, silences the envelope, zeroes the gate, and
resets the ladder — its integrators, its feedback memory and its excitation noise — and the DC
blocker ahead of it. **The voice is inactive the moment it returns**, so the plugin reports `Normal`
and a host may stop calling at once: whatever those two stages held would otherwise be where the
next note starts, however long the host slept. It keeps what runs free — the oscillator's phases and
noise, the LFO's phase and held value — and the glide state, which the first note after a panic
restarts from its own key as any note from an inactive envelope does. All Notes Off (CC 123) clears
the stack and releases the envelope, and resets no stage.
`panic_clears_the_filter_and_dc_blocker_before_a_host_sleeps` plays two voices through the same
pitches with different levels, cutoffs, resonances and envelopes, panics both, processes neither,
and requires the same note to render bit-identically; without either reset it differs from the
first sample.

## The resonance saturator is a diode clamp, and `tanh` must not come back

The resonance feedback path uses **back-to-back diode clamping**, not `tanh`.
`research:filters/machines/ir3109-roland.md` §4 identifies the diode pair as characteristic of this
machine: it is linear below the knee and limits firmly above it. These contracts are test-pinned:

- **`diode_clamp` is the deep-dive's §7 curve** — bounded by `CLAMP_KNEE` for every finite input
  (up to `f32::MAX`, where the plain formula overflows and a guard returns the knee), strictly
  increasing where the solve can land, odd, with the closed-form derivative matching a central
  difference and a slope of exactly 1 at zero. `diode_clamp_is_bounded_monotonic_odd_and_its_derivative_is_exact`.
- **The threshold did not move.** Unity slope at zero means the linearised loop is the ideal ladder,
  so `k` = 3.97–4.00 at all four rates, as before. What changed is everything above it.
- **The loop is clean below the knee and limits above it**, and that is *measurable from outside*:
  near threshold the peak gain at cutoff is `1/(4 − k_eff)`, so any compression of the fed-back
  signal is amplified into a gain change. `the_resonance_loop_is_clean_below_the_knee_and_limits_above_it`
  measures 0.20 dB of compression at a moderate level where `tanh` measured 1.03 dB, and a
  cube-law growth where `tanh` grows with the square. **That test was sabotaged before it was
  trusted**: `tanh` substituted back into the loop fails it, and the numbers on both sides are in
  the test's comment.
- **`CLAMP_KNEE` is chosen, not measured** — 1.0, the loop's full scale after the drive stage, so
  the feedback path is clean for any signal the drive can deliver and limits only once the resonant
  peak carries the loop past it. The deep-dive's SH-101 preset starts from 0.55 *with* output-side
  compensation this filter does not have. The two bench measurements that would replace the number
  are named beside the constant. Self-oscillation now settles at 0.55 / 0.69 / 0.87 peak at
  `k` = 4.14 / 4.28 / 4.50 (48 kHz, no input; `mono_01_filter_spike` section 5).
- **`tanh` stays on the input as the drive.** That is headroom, not the resonance circuit, and the
  SH-101's single-supply headroom is a separate and less certain question the deep-dive ranks near
  the bottom. Not changed, not because it is right but because nothing established that it is wrong.
- **The bound is unchanged at 8.0**: `1 + K_MAX·CLAMP_KNEE + 2.5`, the same argument on the knee
  instead of on `tanh`'s unit bound.

**The golden score cannot see this change** (`plugins/mxm-mono-01/host-tests/tests/golden_audio.rs`): it plays
the init patch, whose resonance is zero, so the feedback saturator is out of the loop and the digest
stayed bit-identical. Its comment says so. A green golden run is not evidence about resonance.

Still open from the deep-dive's list, deliberately: Q compensation (the research is in conflict on
whether the SH-101 has any, and this doc's own *do not fix the droop* ruling stands until that is
settled), returning the taps, capacitor mismatch, and the input drive's shape.

## Dependencies: none at runtime

**No *runtime* dependencies, and that is what earns the MSRV 1.87 override** and makes the crate
genuinely portable — it is a claim about the shipped graph, checkable with
`cargo tree -e normal,build`.

`[dependencies]` holds **`mxm-modulation` and nothing else**: zero dependencies of its own, at this
same floor, so it costs the portability claim nothing. `[dev-dependencies]` holds **`mxm-measure`**,
the collection's measurement rulers, on the same terms — a test-only edge, never in a shipped `.clap`,
which [`../mxm-measure/AGENTS.md`](https://github.com/mxm-audio/mxm-kit/blob/main/crates/mxm-measure/AGENTS.md)'s verification section checks rather
than asserts. It also holds `mxm-audio-file` and `mxm-audio-file-decode`, for the render demo's write
and its read-back test, on the same test-only terms. Anything that would need a dependency either does not belong here or
gets written here — `Rng` in `lib.rs` is the precedent.

## Numeric contracts

### A lag toward a target is carried as the remaining distance

**A lag toward a target is carried as the remaining distance, never as the value.** The value
form, `target + (value − target) × coef`, rounds every step's decrement to the value's ulp and
stops moving once a step is under half of it: this crate's glide came to rest 5 cents short of
middle C at half a second and 48 kHz, and `mxm-poly-06`'s at its two-second maximum 37 cents
short, until the next note. `glide_offset × coef` keeps full precision down to zero, so the pitch
lands exactly (`a_glide_lands_exactly_on_its_note`, found and fixed 2026-09-26 in all four
instruments that had the value form — this one, `mxm-poly-06`, `mxm-mono-03` and `mxm-mono-00`).
A new target leaves the pitch where it was: the distance takes up the difference. And a first
note snaps to its target **with the range switch**; it used to snap to the bare key, so a first
note at 16' still slid an octave.

## Randomness is deterministic

**A sample-and-hold draws its first value at `reset`, never zero.** `LfoShape::Random` only draws
on a phase wrap, so a `held` of zero means the shape emits nothing until the first cycle finishes —
and this LFO reaches down to 0.05 Hz, which is twenty seconds of a route that looks connected
doing nothing. `Lfo::reset` draws one, and `Lfo::new` goes through `reset` so a fresh voice is in
the same state as a reset one. Determinism is unaffected: the seed is fixed, so a replayed take is
still bit-identical, which `random_is_bit_repeatable_after_reset` holds.

Found on 2026-09-22 while auditing every sample-and-hold in the collection after a worse one turned
up in `mxm-drum-machine-dsp`. Two of the five hide under the name `Random` rather than
`SampleHold`, so grep for the behaviour and not the word.

## Work Guidance: the theory references

- Filter theory, topologies, saturator choice, aliasing and measurement method are documented in
  [`docs/filters/`](https://github.com/mxm-audio/mxm-kit/blob/main/docs/filters/README.md). Read the relevant chapter before changing
  `filter.rs`; open gaps for this crate are listed in its `07-rust-recipes.md` §7.5 and
  `machines/ir3109-roland.md` §11.
- LFO and envelope behaviour is documented in
  [`docs/modulation/`](https://github.com/mxm-audio/mxm-kit/blob/main/docs/modulation/README.md), measured by
  `../dsp-lab/examples/mod_spike.rs`.
  **Parameter smoothing is not this crate's** — that reference is careful about the boundary, and
  it must stay that way.
- Oscillator antialiasing, waveshapes, analog character and measurement method are documented in
  [`docs/oscillators/`](https://github.com/mxm-audio/mxm-kit/blob/main/docs/oscillators/README.md). Read it before changing `oscillator.rs`;
  the verdict for the shipped algorithm, the changes worth making and the open gaps are in its
  `07-rust-recipes.md`. Every measured number in that reference comes from
  `../dsp-lab/examples/osc_spike.rs`, so a change to `oscillator.rs` means re-running the spike and
  updating what it moves.

## Verification notes

### The rulers are shared, the thresholds are not

**The rulers are shared, the thresholds are not.** `mxm-measure` is a `[dev-dependencies]` entry —
zero dependencies at this same 1.87 floor, and **not in the shipped graph**, which is what the
manifest's *no runtime dependencies* comment means. Measurements come from there; every bound and
its headroom stays in the test that argues for it.

### Tuning is asserted at a tenth of a cent

**Tuning is asserted at a tenth of a cent, and the bound was re-derived rather than rescaled.** It
read one cent while being measured by a crossing *count*, which quantises to ±1 cycle — ±9 cents at
55 Hz over two seconds — so it could not have failed for any tuning error smaller than its own
ruler's. With `mxm-measure`'s interpolated ruler the worst case over the twenty rate/pitch
combinations is **0.009 cents**.

### `resonance_gain`: the level a ladder loses to resonance is right

`resonance_gain` answers a question that recurs: **the filter loses a lot of level as resonance
rises — is that right?** It is. A ladder's DC gain is `1/(1+k)`, and ours tracks that to within
0.01 dB up to self-oscillation. The loss is the topology, not a defect, and
`docs/filters/05-seminal-machines.md` records that it is specifically what separates an SH-101 from
a Juno: same IR3109, but the Juno's external circuit has Q compensation and the 101's does not.
**Do not "fix" it** — adding compensation would make it a Juno.

### `mod_spike` and `osc_spike`

The oscillator and modulation numbers come from `dsp-lab` (`osc_spike` and `mod_spike`; the
commands are in AGENTS.md's Verification).

`mod_spike` does the same for `lfo.rs` and `envelope.rs`; its §4 measures a *generic* smoothing
model rather than the plugin's, because smoothing is plugin-owned and this crate cannot depend on
the plugin. That boundary is stated in `docs/modulation/AGENTS.md` and must not be blurred.

`osc_spike` measures the shipped oscillator through its public API alongside the algorithms we did
not choose — eight alternative waveform generators, a parameterised wavetable bank, a granular
engine, an additive/resynthesis bank, FM/PM operators, Casio-style phase distortion and vintage
sampler playback — and is the sole source of the numbers in
[`docs/oscillators/`](https://github.com/mxm-audio/mxm-kit/blob/main/docs/oscillators/README.md). Run it **in release**: its cost figures are
meaningless otherwise.
