# AGENTS.md — plugins/mxm-mono-01

Parent: [`../AGENTS.md`](../AGENTS.md)

# Purpose

The nice-plug shell for **mxm-mono-01**, a monophonic subtractive synthesizer inspired by the Roland
SH-101. Identity, parameters, MIDI, presets, telemetry and the editor. The voice is
[`crates/mxm-mono-01-dsp`](../../crates/mxm-mono-01-dsp/AGENTS.md).

Shared conventions live in the parent and are not restated here. This doc holds what is **local to
this plugin**; the history, measurements and worked examples behind it are in [NOTES.md](NOTES.md).

# Ownership

`Cargo.toml`, `README.md`, `BASELINE-M0.md`, `control-map.json`, `presets/`, `tests/`,
and `src/` — `lib.rs`, `params.rs`, `preset.rs`, `routes.rs`, `telemetry.rs`, and `editor.rs` with
its `editor/{binding, sections, visuals}.rs`. Its licence is the repository's root `LICENSE`.

# Local Contracts

## Permanent identifiers

| What | Value |
|---|---|
| `CLAP_ID` | `dk.mxm.mxm-mono-01` — assembled from `plugin_name!` in `src/lib.rs`, **not** from `CARGO_PKG_NAME` |
| Parameter `#[id]`s | see `src/params.rs` and `src/routes.rs`; **22 of its own plus 88 routing pairs, all permanent**. `lfosync` (2026-09-25) is the newest. It was 27 until the six that named the machine's own modulation became routes — the only retirements this instrument has, each an owner decision under plan 1.13 |

**`CLAP_ID` has been changed once, and will not be again**: user presets, favourites and saved locks
are keyed to it ([NOTES.md § Permanent identifiers](NOTES.md#permanent-identifiers)).

## Glide is an *amount* here, and starts at zero

Glide is always engaged, so its time is the amount of portamento and zero means off. It is not
smoothed: a time constant, not a signal ([NOTES.md § Glide](NOTES.md#glide-is-an-amount-here-and-starts-at-zero)).

## `preset.rs` owns this product’s `Instrument` implementation

The shared system is mxm-kit's `mxm-preset`; this file owns the `Instrument` implementation and fifty
categorized factory presets in `presets/`, generated from test-module `FACTORY_DESIGN`.

## The editor, and its brief

Carries the collection's **developer channel** (`plugins/AGENTS.md`, *A developer channel in every
editor*): with `MXM_DEV_CC` in the process environment, CC 119 selects a category (0–5) or Parameters (127), as defined by the parent, CC 117 opens and closes the preset browser. CC 118 opens and closes the Oscillator card's Advanced expander. CC 116 sets the theme by index — 0 light, 1 dark, 2 system — without saving it.

- **Four indivisible cards** under dynamic paging — *LFO and envelope* (LFO, envelope and amplifier;
  every name keeps its prefix), *Oscillator*, *Mixer*, *Filter* — plus a developer Parameters
  surface and the brief's §8 visualizations ([NOTES.md § The editor](NOTES.md#the-editor-and-its-brief)).
- **`src/telemetry.rs` is the only DSP → editor channel**: `Arc`-shared, atomics only, written once
  per block; the peak is max-combined and reset on read, the clip latches until acknowledged.
- **The output level (`outgain`) is in the app bar, not on a card**: `Bound::slider_inline` in
  `mxm_ui::navigation::bar_card` under `editor::OUTPUT_CARD`, cursor through
  `navigation::paged_with_bar`, classified `Surface::AppBar` in `Section::LfoAndEnvelope`
  (`the_output_level_is_drawn_once_in_the_app_bar`).
- **Tempo sync**: `lfosync` on `params::LFO_SYNC`, resolved once a buffer by
  `MxmMono01Params::synced_lfo_rate` in place of the free smoother (which keeps advancing).
- **Sizes are derived**: `REFERENCE` is the quarter-4K budget hugged
  (`the_opening_size_is_the_budget_hugged`), `MINIMUM` one card wide plus both gutters
  (`the_frame_is_the_layouts_own`); `every_dynamic_page_fits_and_every_card_is_reachable`. Native
  DPI, user inspection and DAW gates remain open.
- **Every card is a `mxm_ui::tree`** (`sections::card`, drawn by `sections::paint`): floors are
  computed, a card's ceiling is its floor, no card declares a usability minimum. Displays state their
  size (`visuals::panel_height`), route stacks theirs (`mxm_modulation_params::ui::stack_size`).
- **The Advanced disclosure's state is egui's** (`disclosure_id("Advanced")`, CC 118 through
  `editor::set_advanced`); its body is reserved open, so opening it never grows the card.
- `sections::draw` keeps its signature for `apps/mxm-layout-lab` (in the private archive since the
  split); `disclosed` is no longer read.
- `every_card_passes_the_tree_checks_in_every_state`: every card at Init, Advanced open, every route
  at full negative depth, and a note sounding.

## The keyboard cursor has a product-specific deep proof

- `sections::ASSIGNMENT` cross-checks the painted frame's registry, beside the shared
  `editor::tests::the_keyboard_cursor_reaches_and_operates_every_parameter`; `sections::binding_for`
  is `pub` so a test can resolve a cursor's parameter back to its `Bound`.
- The keys are the kit's keyboard language (design system §11, the parent's contract; every editor
  since 2026-10-08): a bare arrow moves inside the card, VIEW + an arrow card to card, a step key
  (or VALUE) + an arrow edits (FINE, COARSE or MICRO; ↑ ↓ by the size, ← → to the next line of it,
  2026-10-09), OUT or letting go of a held key keeps the edit and BACK cancels it, DELETE restores
  the default. The cursor starts on the first card's first parameter,
  not the app bar's output level.
- `tests/keyboard_editing.rs` drives the real panel with real key events. **The feel under the
  owner's own hands remains the deciding manual gate**
  ([NOTES.md § The keyboard cursor](NOTES.md#the-keyboard-cursor-has-a-product-specific-deep-proof)).

## Modulation routing

Four targets and eleven sources, declared in `mxm-mono-01-dsp`'s `routing` module
([NOTES.md § Modulation routing](NOTES.md#modulation-routing)):

- A route is a *(target, source)* pair with a presence and an amount; `mod_<target>_<source>` and
  `mod_<target>_<source>on` are permanent ids.
- **A fresh instance is still the instrument it always was**: the machine's own paths are routes
  present at Init. [`BASELINE-M0.md`](BASELINE-M0.md) is what the conversion was measured against.
- **Nothing routed costs nothing**; topology is still re-read once per buffer **deliberately**,
  because a host state restore changes parameters without sending events.
- **A route that becomes present arrives at its stored depth** (`Routes::topology_from`,
  `TargetRoutes::arm`): `a_re_added_route_arrives_at_its_stored_depth_rather_than_ramping_from_a_stale_one`.
- **A control-map role binds a route amount only where Init wires that route**:
  `a_control_map_role_never_points_at_a_dead_route`.
- **The amplitude target scales the envelope rather than adding to it**; adding makes the voice's
  lifetime circular.
- **Routing is drawn beneath the control it moves**, never in a footer; an empty target costs one
  line and no border; a row ends in a **cross**, not a switch, and the cross sits on its slider's
  track (`a_routes_remove_is_vertically_centred_on_its_sliders_track`).
- A route stack's floor is its widest row at its widest reading; the tree checks read **what was
  painted**, not only what the layout admits to.

## The machine's own modulation *is* routing

**There are no modulation depth knobs.** The retired ids' five paths are routes **present in the
init patch at zero depth** (the owner's ruling;
[NOTES.md § The machine's own modulation](NOTES.md#the-machines-own-modulation-is-routing)):

| Was | Is | Scale |
|---|---|---|
| `vcolfo` | `mod_pitch_lfo` | 7 semitones |
| `pwmdepth` + `pwmsource` | `mod_width_lfo`, `mod_width_env` | ± 0.45 of width |
| `filterenv` | `mod_cutoff_env` | 6 octaves |
| `filterlfo` | `mod_cutoff_lfo` | 4 octaves |
| `keytrack` | `mod_cutoff_key` | 5 × the five-octave key source |

- **A route the machine itself wires keeps the scale it always had** — the scale table is per
  *(target, source)*, and each route is bit-identical to the knob it replaced.
- **Every route the SH-101 did not have is the collection's standard**: Key the glided note, Velocity
  `v − 1` of the press that last triggered the envelope (`a_legato_joint_keeps_the_phrase_velocity`),
  Bend the lever (`Patch::bend`), Amplitude the standard factor.
- **A route's amount reads what its pair delivers**, in the target's unit (`amount_param` through
  `routes::reach`; Key's unit is `routing::KEY_UNIT_SEMITONES`), never a negative zero:
  `a_route_reads_what_its_pair_delivers_and_reads_back`, `every_route_parameter_says_what_the_dsp_does`,
  `every_reading_survives_the_hosts_round_trip_a_rounded_zero_included`.
- **`vcasource` stays a switch**: it swaps the envelope out for the gate, which no amplitude route can
  express.
- **The mod wheel stays hard-wired**, added into the `mod_pitch_lfo` amount **whichever way it
  points**, never taking its sign (`the_wheel_push_adds_the_same_whichever_way_the_route_points`).
- **Presets carry routing**: `Instrument::parameters` includes all 88 pairs, named by `ROUTE_IDS` in
  `src/routes.rs`; `every_factory_preset_covers_every_parameter`.
- **An old project loses the depths its retired ids held, and keeps everything else.** No
  `filter_state` translation is built (the owner, 2026-09-15).

## The pre-conversion baseline protects migration

- [`BASELINE-M0.md`](BASELINE-M0.md)'s M0 cost and fifty reference digests are **committed rather
  than re-derived**: they stop existing once the conversion starts. `src/lib.rs`'s `baseline` module
  produces them and is `#[ignore]`d (the `baseline` command under Verification).
- **`render_block_for_test` is a measurement seam, not a second `process()`**: exactly `process()`'s
  loop, without the per-block event handling. It lives here because the cost is plugin-side.
- **The figure is reproducible on this machine and not portable off it**; its use is the
  before/after comparison, the owner's cost gate
  ([NOTES.md § The pre-conversion baseline](NOTES.md#the-pre-conversion-baseline-protects-migration)).

## Smaller contracts

- **Every parameter's text survives the host's round trip**: a formatter must not switch unit or
  precision at the raw value; choose the unit from what the finer branch would print (`v2s_time`,
  and `v2s_hertz` on the cutoff in place of nice-plug's `v2s_f32_hz_then_khz(1)`).
  `params::tests::every_parameter_reads_the_same_after_the_hosts_round_trip`
  ([NOTES.md § Every parameter's text](NOTES.md#every-parameters-text-survives-the-hosts-round-trip)).
- **The upstream-defect test**: `upstream_defects` pins the arithmetic behind one of the nice-plug
  defects recorded in [`docs/known-issues.md`](https://github.com/mxm-audio/mxm-kit/blob/main/docs/known-issues.md), so the recorded diagnosis cannot go stale
  silently. If it fails, recheck the diagnosis before changing the number.
- **`editor`, `params` and `telemetry` are public**, with the `Section` enum, its `SECTIONS`,
  `title()` and the card grouping, **permanently**, so `apps/mxm-layout-lab` (private archive) draws these real cards
  ([NOTES.md § `editor`, `params` and `telemetry`](NOTES.md#editor-params-and-telemetry-are-public)).
- **Activation refuses a rate the DSP cannot hold**: `activate` returns `false`, before anything
  changes, for a non-finite rate or one below `mxm_mono_01_dsp::MIN_SAMPLE_RATE`
  (`activation_refuses_a_non_finite_rate_and_any_below_the_floor`,
  `the_rate_floor_activates_and_plays_at_every_parameter_extreme`).

# Work Guidance

# Verification

```bash
cargo test -p mxm-mono-01
cargo test -p mxm-mono-01 --lib every_card_passes_the_tree_checks_in_every_state
# Every page, light and dark, for review -> target/layout-tree/mxm-mono-01/<MXM_PICTURES tag>/
MXM_PICTURES=after cargo test -p mxm-mono-01 --lib tree_pictures -- --ignored
cargo test -p mxm-mono-01 --release baseline -- --ignored --nocapture   # the M0 captures; prints
cargo clippy -p mxm-mono-01 --all-targets
cargo xtask bundle mxm-mono-01 --release
clap-validator validate "target/bundled/mxm-mono-01.clap"
cargo run -p mxm-mono-01-standalone      # the editor with no host at all
```

**A real DAW at a small buffer size is a recorded-unmet gate** — see the conventions' *Verification*
the parent links to, and
[`docs/briefs/mxm-mono-01.md`](../../docs/briefs/mxm-mono-01.md)'s three claims.

# Child DOX Index

No child AGENTS.md files.
