# NOTES.md — plugins/mxm-mono-01

The detail behind this folder's AGENTS.md: history, measurements, rationale and worked examples. AGENTS.md is the contract; this file is the reference it links to.

## Permanent identifiers

**`CLAP_ID` has been changed once, and will not be again.** MXM-101 became `mxm-mono-01` while the
project was pre-alpha, undistributed and untagged — the only moment the change cost nothing. It
orphaned every file keyed to the old id: the user preset directory (which `preset::user_root` *names*
after `CLAP_ID`), its `favourites.json`, parked locks in the player's settings, and locks inside any
saved sequence. That loss was accepted deliberately because nothing had been saved. After a release
none of it would be acceptable, which is what the permanence rule protects.

## Glide is an *amount* here, and starts at zero

Glide is always engaged on this instrument, so its time is how much portamento there is and zero
means off — which is what the parent's init-patch contract names it. The code comment beside the
parameter, *"Not smoothed: this is a time constant, not a signal"*, is the other half of it.

`mxm-mono-03` has no glide control at all, so no shared rule is needed to reconcile the two.

## The editor, and its brief

Four indivisible cards under dynamic paging, a separate developer Parameters surface, and the visualizations of
[`docs/briefs/mxm-mono-01.md`](../../docs/briefs/mxm-mono-01.md) §8. `src/telemetry.rs` is the
**only** DSP → editor channel: `Arc`-shared, atomics only, written once per block, with a peak that
is max-combined and reset on read and a clip that latches until acknowledged.

**The output level (`outgain`) is in the app bar, not on a card** — design system §3.1
item 6 puts the master output there, and the owner ruled every instrument's master volume into the
bar (2026-09-18). It is an inline slider (`Bound::slider_inline`) beside the level meter, drawn
inside `mxm_ui::navigation::bar_card` under `editor::OUTPUT_CARD` (**64**, outside the page keys),
and the cursor runs through `navigation::paged_with_bar`. `sections::ASSIGNMENT` classifies it
`Surface::AppBar` in `Section::LfoAndEnvelope`, the card that holds the amplifier: the section is its
place in the chain and the Parameters list, the surface is where it is drawn.

**The LFO, the envelope and the amplifier are one card, *LFO and envelope*** (the owner,
2026-09-24; `plans/plan-editor-standard.md` A4). Once cards hugged their content, the brief's
LFO was one knob and a shape row and its Amplifier one switch and a stack. The card holds the
LFO's rate and shapes, the one envelope, the VCA source and the amplitude routes; it holds three
modules, so every name keeps its prefix.

**The LFO rate has the collection's one tempo sync** (`plans/plan-tempo-sync-controls.md`,
`plugins/AGENTS.md`): `lfosync`, the quarter note between the rate and its shapes, on
`params::LFO_SYNC` (1/32 to four bars, the top the fastest). `MxmMono01Params::synced_lfo_rate`
resolves it once a buffer from the modulated position, the result stands in for the free smoother
(which keeps advancing), and `Telemetry::tempo` lets the knob read its division. The brief's six sections are four cards: *LFO and
envelope*, *Oscillator*, *Mixer*, *Filter*. `the_output_level_is_drawn_once_in_the_app_bar` holds that it is painted once,
there.

**The opening size is the quarter-4K budget hugged** (`REFERENCE`) — derived, not chosen:
`the_opening_size_is_the_budget_hugged` prints the number to take when a card changes. **The
minimum is one card wide plus both gutters** (`MINIMUM`); the widest floor must stay inside it
(`the_frame_is_the_layouts_own`). `page_items` keys 0–3 are the cards' positions in `SECTIONS`:
*LFO and envelope* is Modulators — its first operation, though it also holds the amplifier —
Oscillator is Generators, Mixer and Filter are Tone and remain a preferred group.
The Oscillator's embedded performance controls stay on its indivisible Generators card.
`every_dynamic_page_fits_and_every_card_is_reachable` checks every page in both themes at opening,
quarter-4K content and minimum sizes; component row proofs remain. Native DPI, user inspection and
DAW gates remain open.

**Every card is a `mxm_ui::tree`** (`crates/ui/AGENTS.md`, *A card body as data*).
`sections::card` describes a card's body once — the collection's knob rows (`tree::knob_row`), the
switches, the shape rows, the sliders, the route stacks, the three displays — and that one
description is measured for the card's floor and height and drawn leaf by leaf through the same
bindings (`sections::paint`); the paged view is `paging::editor::show`. The gaps are the hand
layout's: the body's `SPACE_3` rhythm, with the `add_space` it put on top as pads.

- **Floors are computed.** `page_items` takes each card's floor from its tree every frame, and the
  card is exactly as wide as that floor: its ceiling is its floor (`plans/plan-editor-standard.md`
  A1), and no card declares a usability minimum (A2).
- **The displays state their size**: `visuals::panel_height` — `FULL_HEIGHT` from `ROOMY` wide,
  `REDUCED_HEIGHT` below — is both what each display allocates and what its tree leaf says; they
  fill the card and have no minimum width of their own. A route stack states its own
  (`mxm_modulation_params::ui::stack_size`, every route revealed at its widest reading).
- **The Advanced disclosure's state is egui's**, under `mxm_ui::shell::disclosure_id("Advanced")`:
  its header reads and writes it, and CC 118 writes it there (`editor::set_advanced`). The card
  reserves its body open, so opening it never grows the card or moves anything outside it; the
  routes under it move down into the room at the card's foot. Its body sits in a row that spans the
  card, so the separator over it does.
- `sections::draw` stays for `apps/mxm-layout-lab`, with its signature: it builds the section's tree
  and shows it, and its `disclosed` argument is no longer read.
- `every_card_passes_the_tree_checks_in_every_state` runs `mxm_plugin_test::tree_checks` over every
  card at Init, with Advanced open, with every route revealed at full negative depth (closed and
  open), and with a note sounding — the envelope's marker, the one thing telemetry adds.

## The keyboard cursor has a product-specific deep proof

`sections::ASSIGNMENT` independently cross-checks the registry built by the painted frame. The
shared coverage check runs here too — `editor::tests::the_keyboard_cursor_reaches_and_operates_every_parameter`,
owed since the output level moved above the cards — and this product retains the deeper
table-backed proof beside it.

`Shift`+arrows move module/card to module/card, `Command`+arrows move inside a card, and bare arrows
set the value — left/right fine, up/down coarse, `Command`+`Backspace` back to the default. The
parent's *The keyboard cursor runs in every editor* owns the contract; what is local is that
`sections::binding_for` is `pub` so a test can resolve a cursor's parameter back to its `Bound`.

`tests/keyboard_editing.rs` drives the real panel with real key events: that a fresh cursor lands on
the app bar's output level (the bar is drawn before any card, so card tests walk to LFO with
`Shift` arrows first), registration, category-first
card order across paging, text and waveform segmented editing, the step law against each
parameter's own `stepping`, keyboard editing after a custom control loses egui focus, one balanced
gesture across a held continuous edit, cardless Parameters behavior, and that `Space` and the
browser's arrows are left alone. The hierarchy was corrected after live use: the higher `Shift` key
selects the higher module/card level, `Command` selects parameters and bare arrows edit values.
Native synthetic keys in MXM Player proved all three tiers against loaded-plugin readback on Windows;
**the revised feel under the owner's own hands remains the deciding manual gate.**

## Modulation routing

Four targets — pitch, pulse width, cutoff, amplitude — and eleven sources, declared in
`mxm-mono-01-dsp`'s `routing` module: the LFO and envelope it already had, the five performance
inputs, and **its own saw, pulse, sub and noise**, which is what makes FM reachable without inventing
a generator. A route is a *(target, source)* pair with a presence and an amount; `mod_<target>_<source>`
and `mod_<target>_<source>on` are permanent ids.

**A fresh instance is still the instrument it always was.** The machine's own paths are routes
present at Init, each at the scale its retired knob had (*The machine's own modulation is routing*,
below), and the routing is the freedom on top. [`BASELINE-M0.md`](BASELINE-M0.md) is what the
conversion was measured against: once those paths became routes, 27 of the fifty factory digests
moved from the order three cutoff terms are summed in, with every peak identical to four decimal
places (`plans/plan-modulation-routing.md` revision 12).

**Nothing routed costs nothing**, by construction rather than by promise: with no live route the
voice does not open its frame, publish a source or take a sum. What that leaves is topology re-read
once per buffer, worth about 2 % — kept deliberately, because a host state restore changes parameters
without sending events and a stale topology would silently drop a route from a loaded project.

**A route that becomes present arrives at its stored depth.** An absent route's smoother is never
advanced, but its parameter stays editable, so `process()` resolves topology with
`Routes::topology_from` against last buffer's and snaps each newly present route's smoother to its
value (`TargetRoutes::arm`). Without it, a route edited while absent and then re-added ramps in from
a stale depth over a span set by the host's buffers.
`a_re_added_route_arrives_at_its_stored_depth_rather_than_ramping_from_a_stale_one` holds it; the
DSP half — a newly read source starting from silence — is `mxm-mono-01-dsp`'s.

**A control-map role binds a route amount only where Init wires that route**, or its knob is dead on
a fresh instance. All five routing ids the map names are among the machine's own wiring;
`a_control_map_role_never_points_at_a_dead_route` holds that and counts them.

**The amplitude target scales the envelope rather than adding to it.** Adding would make the voice's
lifetime circular: a latched key routed there would hold the voice audible after its envelope ended,
so ending on the envelope cuts a non-zero signal and ending on audibility never ends.

**Routing is drawn beneath the control it moves**, never in a footer — pitch and pulse width on the
Oscillator card, cutoff on the Filter, amplitude on *LFO and envelope* beneath the VCA source. **A target with nothing routed
costs one line and no border**: no group is drawn around an empty stack, and `‹ modulate ›` stands
outside the group with the target's name beside it, so a card carrying two stacks says which control
each one reaches before it is used. A row ends in a **cross**, not a switch — *it is implied that it
is on when visible*. All four were the owner's, at first sight of the pilot.

A route stack's floor is its widest row at its widest reading, present or not.
`every_card_passes_the_tree_checks_in_every_state` holds every card with every route revealed at
full negative depth, and reads **what was painted** as well as what the layout admits to, because a
`Ui`'s `min_rect` is clamped to the rect it was given and once read back as fitting while a removal
button ran through the card's border.

**A route's cross sits on its slider's track**, not half a name line above it, which is where a
row's own centring leaves a control shorter than the name/value pair beside it.
`a_routes_remove_is_vertically_centred_on_its_sliders_track` holds it, and its oracle is the keyboard
cursor's registry rather than the painted shapes: both halves of a route register there under the
parameter they set, and a slider registers its **track**, so the two boxes that have to line up are
named rather than guessed at.

## The machine's own modulation *is* routing

**There are no modulation depth knobs on this instrument any more.** `vcolfo`, `pwmdepth`,
`pwmsource`, `filterenv`, `filterlfo` and `keytrack` are **retired permanent ids**. The five paths
they named are five routes, **present in the init patch at zero depth** — the owner's ruling on
seeing the pilot carry both: *they should just be preselected init parameters*, and it is what the
plan asked for from its first sentence.

**A route the machine itself wires keeps the scale it always had**, which is why the scale table is
per *(target, source)* and not per target: the filter envelope reaches six octaves from the same
cutoff the filter LFO reaches four from. That is plan §5's conservative form, and it is what makes an
old depth **the same number** on the route that replaced it — no patch had to be re-dialled, and each
route is bit-identical to the knob it replaced.

**Every route the SH-101 did not have is the collection's standard** (`plans/plan-modulation-
standard.md`, 2026-09-26): Key is the keyboard's glided note — not the range switch, which is the
oscillator's — Velocity is `v − 1` of **the press that last triggered the envelope** (a legato joint
keeps the phrase's, `a_legato_joint_keeps_the_phrase_velocity`), Bend is the lever rather than the
pitch it bends (`Patch::bend`), and Amplitude is the standard factor, silence to double. An added
route reaches an octave of pitch (12 st/oct from Key) and four octaves of cutoff; it used to take
the LFO's seven semitones and the envelope's six octaves. No factory design used an added route, so
no sound moved.

**A route's amount reads what its pair delivers**, in the target's own unit — semitones, a
percentage of width, octaves, a percentage of the amplifier's level, and per octave of keyboard for a
Key route — so the machine's own routes read, at full, the numbers the table's scales name: +7.00 st,
+45 %, +6.00 oct, +4.00 oct and +1.00 oct/oct. A bare percentage of the amount read `100` for all of
them, the same number for six octaves and for four, where `plans/plan-modulation-routing.md`
decision 1.11 asks for the target's unit. Every amount is the collection's one route parameter,
`mxm_modulation_params::reading::amount_param`, through `routes::reach`; the Key unit of 60 semitones
is the DSP's `routing::KEY_UNIT_SEMITONES`. `a_route_reads_what_its_pair_delivers_and_reads_back`
pins each full-scale reading and a typed reading back to its amount, and
`every_route_parameter_says_what_the_dsp_does` holds every pair's travel and reading to what
`mxm_mono_01_dsp::conformance` measures the graph delivering (`mxm_plugin_test::routing_checks`).
**It never prints a negative zero**, and
`every_reading_survives_the_hosts_round_trip_a_rounded_zero_included` sends every amount through
the host's conversion either side of zero, where the bare percentage printed `-0`.

**`vcasource` is not among the retirements, and that is not an oversight.** It swaps the envelope
*out* for the gate — `env + (gate − env) × mix` — where an amplitude route *scales* the envelope. No
route into the amplifier can reach `amp = gate`, so it stays a switch until this instrument has a
gate source and a law that can express a swap.

**The mod wheel stays a hard-wired path** and is now added into the `mod_pitch_lfo` amount rather
than into `vcolfo`, **whichever way that amount points**: the lever's depth sums with the route's, so
a negative route is partly cancelled rather than deepened, and the depth stays continuous as a swept
amount crosses zero (`the_wheel_push_adds_the_same_whichever_way_the_route_points`). A push that took
the route's sign was tried and reverted on 2026-09-15: it stepped the depth by twice the push at the
crossing. It scales the vibrato depth; routing the wheel to pitch would give a second
bender, which is a different thing. The raw wheel is a routable source beside it. Plan §5.2 keeps
this as named legacy until there is a multiplier module to say it natively.

**Presets carry routing now.** They did not while every route was absent at init, and nothing
noticed — which stopped being harmless the moment the machine's own modulation became routes:
`Instrument::parameters` includes all 88 pairs, so `every_factory_preset_covers_every_parameter` can
do its job. `ROUTE_IDS` in `src/routes.rs` names them, and a test holds it against what the derive
actually produces.

**An old project loses the depths its retired ids held, and keeps everything else.** No
`filter_state` translation is built. nice-plug leaves a parameter a restored state does not name at
its current value, so the six retired ids' values are dropped and the routes that replaced them load
at Init's zero depth. The instrument is unreleased and its factory sounds were regenerated either
way — the decision `mxm-mono-00` and `mxm-mono-pr1` recorded, taken for the pilot by the owner on
2026-09-15 rather than left implicit.

## The pre-conversion baseline protects migration

[`BASELINE-M0.md`](BASELINE-M0.md) holds what
`plans/plan-modulation-routing.md` (`plans/plan-modulation-routing.md` in the private archive) M0 captured before the
modulation conversion begins: **≈ 206 ns/sample** through the plugin's own per-sample path at 48 kHz
on the init patch, and **fifty reference digests**, one per factory sound. Both stop existing the
moment the conversion starts, which is the whole reason they are committed rather than re-derived.

`src/lib.rs`'s `baseline` module produces them and is `#[ignore]`d, because a measurement that fails
on a busy machine teaches nothing:

```bash
cargo test -p mxm-mono-01 --release baseline -- --ignored --nocapture
```

**`render_block_for_test` is a measurement seam, not a second `process()`.** It runs exactly the loop
`process()` runs — `next_patch()` per sample, then `Voice::process` — and omits the wrapper's
per-block event handling, which is stated in the record rather than left to be assumed.
`mxm-shimmer`, `mxm-grain-fx` and `mxm-bucket-delay` carry the same `_for_test` shape.

**Why the seam is here and not in the DSP crate**: the cost the conversion adds is plugin-side — the
smoothers and the per-sample `Patch` rebuild — and a framework-free bench cannot reach them. A review
round caught the plan pointing this measurement at the wrong layer.

**The figure is reproducible on this machine and not portable off it**, the hedge
`crates/mxm-bucket-delay-dsp/AGENTS.md` already states for its own probe. Its use is the before/after
comparison, which is the owner's stated cost gate.

## Every parameter's text survives the host's round trip

A host parses a parameter's text and **normalises the number before printing it again**, so the
value it prints lands a hair either side of where it started. A formatter that switches unit or
precision at the raw value is not idempotent there, and `clap-validator`'s `param-conversions` fails
only when its values land in that sliver. So in `src/params.rs` the unit is chosen from what the
finer branch would print: `v2s_time` prints seconds for anything that would read `1000 ms`, and
`v2s_hertz` — which replaces nice-plug's `v2s_f32_hz_then_khz(1)` on the cutoff — kilohertz for
anything that would read `1000.0 Hz`. The route readings never print a negative zero (*The
machine's own modulation is routing*).
`params::tests::every_parameter_reads_the_same_after_the_hosts_round_trip` holds all 110, converting
as the wrapper does, with the unit: at the validator's grids, either side of each branch point, and
at every representable normalised value near each. Attack, Decay, Release, Glide and the cutoff
failed it before the fix, and every route amount before its readings.

## `editor`, `params` and `telemetry` are public

They are `pub`, with the `Section` enum, its `SECTIONS`, `title()` and the card grouping the flow
reads, so [`apps/mxm-layout-lab`](https://github.com/mxm-audio/newdawn-workspace/blob/main/apps/mxm-layout-lab/AGENTS.md) can draw **these real cards**
on its bench instead of copying the section code, which would then drift.

It began as a branch-only change for that lab and **is now permanent**, because the reflowing layout
the lab was built to judge shipped on 2026-09-04: the same section data that feeds
the paging renderer in this editor is what the bench re-draws. Nothing else changes — no item's own
behaviour moves, and the shipped `cdylib` and its CLAP entry point are untouched.

## Activation refuses a rate the DSP cannot hold

`activate` returns `false`, before anything changes, for a non-finite host rate or one below
`mxm_mono_01_dsp::MIN_SAMPLE_RATE`, 1 kHz: a NaN rate, or one low enough for a corner's floor to
cross 0.45 of it, panicked on the audio thread.
`activation_refuses_a_non_finite_rate_and_any_below_the_floor` holds the refusal, and
`the_rate_floor_activates_and_plays_at_every_parameter_extreme` a held note at the floor with every
parameter at its default and at either end.
