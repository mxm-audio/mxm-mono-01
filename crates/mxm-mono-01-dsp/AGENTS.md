# AGENTS.md — crates/mxm-mono-01-dsp

Parent: [`../../AGENTS.md`](../../AGENTS.md)

# Purpose

The complete mxm-mono-01 voice as plain Rust: oscillators, filter, envelope, LFO, and the monophonic
voice that wires them together. Free of any plugin-framework types so the whole signal path is
testable with `cargo test` and no host involved. The history, measurements and worked examples
behind the rules below are in [NOTES.md](NOTES.md).

# Ownership

Owns `src/` (`lib.rs`, `oscillator.rs`, `filter.rs`, `envelope.rs`, `lfo.rs`, `routing.rs`,
`voice.rs`, `conformance.rs`), `tests/` (`equivalence.rs`, `routing.rs`), `examples/` (`mono_01_filter_spike.rs`,
`mono_01_render_demo.rs`, `resonance_gain.rs`), and `Cargo.toml`.

`osc_spike.rs` and `mod_spike.rs` live in mxm-tools' `dsp-lab`, not here
([NOTES.md § Ownership](NOTES.md#ownership-the-spikes-live-in-dsp-lab)); the examples here verify *this* crate's shipped DSP.

Does **not** own parameter definitions, ranges, or smoothing — those belong to
[`plugins/AGENTS.md`](../../plugins/AGENTS.md). This crate takes plain values and a sample rate.

# Local Contracts

## Read-only accessors are allowed; nothing else looks outward

`Voice::env_level` and `Voice::env_stage` feed the brief's envelope display; `Voice::sounding` routes
per-note expressions ([NOTES.md § Read-only accessors](NOTES.md#read-only-accessors-are-allowed-nothing-else-looks-outward)).

- **Read-only, and additive.** No new state, no branch in the audio path, no allocation.
- **The audio thread reads them**, once per block, and publishes to an atomic owned by the plugin.
  A UI thread must never call them (`Voice` is `&mut`-owned by the processor).
- **The transport is not this crate's business**: `plugins/mxm-mono-01/src/telemetry.rs` owns it.
- **This is not a licence to add outputs.** A new accessor needs the envelope one's argument: the
  job is outside this crate, the value exists only in here, and there is no other source.
- **`Voice::sounding` is routing, not telemetry**: the `NoteId` playing, decided by `stack`; the
  plugin must not mirror it (`the_sounding_note_is_the_one_an_expression_should_be_routed_to`).

## A source that becomes needed starts from silence

- `Graph::set_topology` **clears every source that has just become needed**
  (`mxm_modulation::SourceFrame::clear`), so a new backward route reads a deterministic zero, never a
  value from an earlier phrase. `a_source_that_becomes_needed_starts_from_silence_not_from_an_old_phrase`.
- **The evaluation order is proved by rendering, not by a table**: audio is published after the
  pitch and width sums, before the cutoff sum
  (`a_route_from_the_instruments_audio_is_a_sample_late_into_pitch_and_on_time_into_cutoff`;
  [NOTES.md § A source that becomes needed](NOTES.md#a-source-that-becomes-needed-starts-from-silence)).

## The performance sources are the collection's standard

- Key, Velocity, Wheel, Pressure and Bend go through `mxm_modulation::standard`, each zero at rest:
  **Key is the keyboard's glided note** (after the glide, without the range switch), **Velocity is
  `v − 1`**, **Bend is the lever** (`Patch::bend`), **Amplitude is `standard::amplitude_factor`**.
- A path the SH-101 has keeps its reach; every added path takes the standard's (`routing::FULL_SCALE`).
- `conformance.rs`'s `Declared` runs the standard's checks over the real tables and a real graph;
  the plugin's tests reuse it through the `conformance` feature, which only `[dev-dependencies]`
  enable ([NOTES.md § The performance sources](NOTES.md#the-performance-sources-are-the-collections-standard)).

## A panic clears the recursive stages, not only the envelope

- `Voice::all_sound_off` (CC 120) retires every press, silences the envelope, zeroes the gate, and
  resets the ladder (integrators, feedback memory, excitation noise) and the DC blocker. **The voice
  is inactive the moment it returns**, so a host may stop calling at once
  (`panic_clears_the_filter_and_dc_blocker_before_a_host_sleeps`).
- It keeps what runs free — oscillator phases and noise, the LFO's phase and held value — and the
  glide state. All Notes Off (CC 123) clears the stack, releases the envelope, and resets no stage
  ([NOTES.md § A panic](NOTES.md#a-panic-clears-the-recursive-stages-not-only-the-envelope)).

## The resonance saturator is a diode clamp, and `tanh` must not come back

Back-to-back diode clamping in the resonance feedback (`research:filters/machines/ir3109-roland.md`
§4); numbers in [NOTES.md § The resonance saturator](NOTES.md#the-resonance-saturator-is-a-diode-clamp-and-tanh-must-not-come-back).

- `diode_clamp_is_bounded_monotonic_odd_and_its_derivative_is_exact` (bounded by `CLAMP_KNEE`, slope 1
  at zero) and `the_resonance_loop_is_clean_below_the_knee_and_limits_above_it` (`tanh` fails it).
- **`CLAMP_KNEE` is chosen, not measured. `tanh` stays on the input as the drive** (headroom).
  **`filter::OUTPUT_BOUND` is `1 + K_MAX·CLAMP_KNEE + 2.5`.**
- **The golden score cannot see resonance** (Init's is zero): a green golden run is no evidence.
- Still open, deliberately: Q compensation, returning the taps, capacitor mismatch, the input drive's shape.

## Dependencies: none at runtime, and no framework types

- **No *runtime* dependencies but `mxm-modulation`** (dependency-free, same floor): that earns the
  MSRV 1.87 override. Check the shipped graph with `cargo tree -e normal,build`.
- `[dev-dependencies]` (`mxm-measure`, `mxm-audio-file`, `mxm-audio-file-decode`) never reach a `.clap`.
- Anything that would need a dependency either does not belong here or gets written here — `Rng` in
  `lib.rs` is the precedent ([NOTES.md § Dependencies](NOTES.md#dependencies-none-at-runtime)).
- No `nice_plug::` anywhere. Every public function takes plain values and a sample rate. If a
  signature would be easier with a framework type, that is a sign the logic belongs in
  `plugins/mxm-mono-01/`.

## Realtime rules

These apply to every per-sample path, not only to a host's `process()`:

- No allocation, no `Vec::push`, no `format!`, no logging.
- No locks, no blocking channels, no file or network I/O.
- Preallocate where the maximum size is known; `reset()` must leave **no tail** from previous
  playback.
- **Flush denormals in the DSP itself** on every recursive state — `flush()` in `lib.rs`, applied to
  every integrator, feedback memory and delay write. Not a framework FTZ guard: it may be a no-op
  without an opt-in feature, and DSP-level flushing is what preserves exact digital silence.
- Cap internal blocks: `min(next_event, host_end, start + MAX_BLOCK)`. Splitting only on events lets
  an event-free buffer become one arbitrarily long block.

## Numeric contracts

- `f32` in the audio path; `f64` for filter coefficients and anything recursive where precision loss
  compounds. Prewarping is computed in `f64` because `tan` near `π/2` loses significance fast.
- Every saturator must be **bounded exactly in `f32`** (clamp the output, do not merely approach the
  bound) and **monotonic**. Boundedness arguments downstream depend on it; a non-monotonic curve
  gives the Newton solve multiple roots.
- Cutoff is clamped to `[20 Hz, 0.45 · fs]` inside the DSP, not only in the parameter layer, which a
  modulation sum can drive past.
- **A clamp's bounds must not cross.** `f32::clamp` panics on a NaN or crossed bound, and the cutoff's
  crosses below 44.4 Hz, so `MIN_SAMPLE_RATE` (1 kHz) is the lowest rate the plugin activates at.
- Any struct with a stated output bound must have that bound written down as a `pub const` with the
  argument that establishes it — `filter::OUTPUT_BOUND` is the pattern.
- **A lag toward a target is carried as the remaining distance, never as the value**
  (`glide_offset × coef`); a new target leaves the pitch where it was, and a first note snaps to its
  target **with the range switch**. `a_glide_lands_exactly_on_its_note`
  ([NOTES.md § A lag toward a target](NOTES.md#a-lag-toward-a-target-is-carried-as-the-remaining-distance)).

## Randomness is deterministic

`Rng` is seeded explicitly so every noise source is bit-repeatable. The filter's self-oscillation
excitation depends on this being testable; do not introduce an unseeded source.
**A sample-and-hold draws its first value at `reset`, never zero** (`Lfo::new` goes through
`Lfo::reset`; `random_is_bit_repeatable_after_reset`). Some are named `Random`, not `SampleHold`:
grep for the behaviour ([NOTES.md § Randomness](NOTES.md#randomness-is-deterministic)).

# Work Guidance

- **Document the technique.** Every nontrivial algorithm carries a comment naming the technique or
  paper — TPT/ZDF, PolyBLEP, Newton iteration. Where a cheaper design was tried and rejected, record
  the measured numbers that rejected it rather than the conclusion alone. `filter.rs` is the
  reference for this style.
- Read the theory chapter before changing a module: `docs/filters/` for `filter.rs`,
  `docs/modulation/` for the LFO and envelope, `docs/oscillators/` for `oscillator.rs` (links and
  open gaps: [NOTES.md § Work Guidance](NOTES.md#work-guidance-the-theory-references)).
- **Parameter smoothing is not this crate's** — keep that boundary.
- A change to `oscillator.rs` means re-running `osc_spike` and updating the numbers it moves.
- Do not extract shared filter or oscillator crates. Per the root contract, per-plugin DSP stays
  per-plugin until a second instrument demonstrates a shared API.
- **Do not "fix" the resonance level loss**: a ladder's DC gain is `1/(1+k)` and ours tracks it;
  adding Q compensation would make it a Juno
  ([NOTES.md § `resonance_gain`](NOTES.md#resonance_gain-the-level-a-ladder-loses-to-resonance-is-right)).
- Prefer a clear implementation to a clever one. This is reference-quality open source.

# Verification

**The rulers are shared, the thresholds are not.** `mxm-measure` (dev-only, same 1.87 floor, not in
the shipped graph) supplies measurements; every bound and its headroom stays in the test that argues
for it. **Tuning is asserted at a tenth of a cent** on its interpolated ruler
([NOTES.md § Tuning](NOTES.md#tuning-is-asserted-at-a-tenth-of-a-cent)).

```bash
cargo test -p mxm-mono-01-dsp
cargo clippy -p mxm-mono-01-dsp --all-targets
```

Properties the tests must keep asserting, because each regresses silently:

- silence in gives **exactly** zero out, after decay (this is the denormal-flush test too)
- no NaN or inf across a sample-rate × cutoff × resonance sweep
- output within the stated bound under overdrive past the oscillation threshold
- `reset()` leaves no tail
- a panic leaves no filter or DC-blocker state for a note after the host has slept
- the filter's self-oscillation threshold, measured at 44.1 / 48 / 96 / 192 kHz
- the diode clamp's bound, monotonicity, oddness and exact derivative — the boundedness argument
  rests on the first, the single Newton root on the second
- the resonance loop is clean below the knee and limits above it, with the cube-law growth that
  separates a clamp from `tanh` — the test `tanh` fails
- the sawtooth's aliasing stays far below what a trivial modulo counter would produce, measured
  against an additive reference that validates the measurement inside the same test

Three `examples/` cover what unit tests cannot, and `dsp-lab` gives the oscillator and modulation
numbers (`osc_spike` **in release** — its cost figures are meaningless otherwise; `mod_spike`
measures a generic smoothing model, see [NOTES.md § `mod_spike` and `osc_spike`](NOTES.md#mod_spike-and-osc_spike)):

```bash
cargo run -p mxm-mono-01-dsp --release --example mono_01_filter_spike   # the filter's real numbers
cargo run -p mxm-mono-01-dsp --example mono_01_render_demo              # WAV — does it sound like a synthesizer
cargo run -p mxm-mono-01-dsp --release --example resonance_gain # does resonance cost the level theory says
cargo run -p dsp-lab --release --example osc_spike
cargo run -p dsp-lab --release --example mod_spike
```

# Child DOX Index

No child AGENTS.md files. `src/` and `examples/` are covered by this doc.
