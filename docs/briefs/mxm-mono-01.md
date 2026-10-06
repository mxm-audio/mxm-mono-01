# mxm-mono-01 — UI design brief

Required by `MXM_DESIGN_SYSTEM.md` §14, written before implementation. Answers the ten questions in
order, then records the two deliberate deviations from the system.

*Since the split (2026-10-06):* the design system is mxm-kit's
[`docs/MXM_DESIGN_SYSTEM.md`](https://github.com/mxm-audio/mxm-kit/blob/main/docs/MXM_DESIGN_SYSTEM.md),
and MXM Player is its own repository, [mxm-player](https://github.com/mxm-audio/mxm-player).

**Instrument:** monophonic subtractive synthesizer. Architecture inspired by the Roland SH-101; the
interface is not.

---

## 1. Primary sound-design task

Building and shaping **monophonic bass and lead lines** — fast. One oscillator, one filter, one
envelope. The instrument's whole appeal is that a usable patch is four or five moves away, so the
editor's job is to keep those moves visible and immediate rather than to expose depth.

The task is *tweaking while playing*, not patch programming. Filter and envelope controls must be
reachable and legible without hunting.

## 2. The three to five parameters users reach for most

1. **Cutoff** — the primary performance control.
2. **Resonance** — paired with cutoff; the two are always adjusted together.
3. **Filter envelope** — the amount control that turns a static tone into a plucked or swept one.
4. **Decay** — with a single shared envelope, decay shapes both timbre and articulation.
5. **Sub level** — the difference between a thin line and a bass that carries a track.

These five get **Primary or Standard** control sizing (§7.1). Everything else is Standard or
Compact. Cutoff and Resonance sit adjacent, at the visual centre of gravity.

## 3. Signal flow that must be visible

```
Oscillator + mixer  →  Filter  →  Envelope / VCA  →  out
                          ↑            │
                         LFO ──────────┘   (LFO → pitch, cutoff, pulse width)
                     Envelope ──────────→  (envelope → cutoff)
```

Two things must read without a manual:

- The **left-to-right chain**: sources → filter → amplifier.
- That **one envelope feeds both filter and amplifier**. This is the instrument's defining
  constraint and the most common source of confusion for anyone expecting two envelopes. The
  envelope display's hover text says so — not a line printed on the card (the owner, 2026-09-27:
  no help text on the panel).

Modulation from ENV and LFO into the Filter card is shown with the collection's modulation colours
(`mod-envelope`, `mod-lfo`) on the affected control, per §8.4 — a coloured secondary arc around the
cutoff control's base value.

## 4. Which controls belong in Play view

**Not applicable — mxm-mono-01 ships no `Play` view.** It has derived musician pages and developer-only Parameters;
see §6.

## 5. Advanced controls and their disclosure

Every one of the 21 parameters is classified — 27 until the six that named the machine's own
modulation became routes, which are classified by the card that owns their target instead. A control with a section but no surface priority can
be buried behind an expander at 820x560 while still passing a coverage test, so both are stated.

**The rule is provenance, not frequency.** Everything the SH-101 had on its panel is on this panel.
Hiding an original control behind a disclosure is what makes an instrument stop being recognisable
— which is the requirement §9 and §11 exist to meet, and it outranks tidiness. **Advanced holds
only what the original did not have**: additions that expand the classic architecture.

Two qualify today, and that is the whole of it.

**Primary surface — always visible, 25 controls.** Everything the original had, and everything
added that belongs beside it: oscillator range, tune, glide, pulse width, PWM source and depth, sub
shape, LFO-to-pitch amount, the four mixer levels, cutoff, resonance, filter envelope amount, filter
LFO amount, key tracking, ADSR, LFO rate and shape, VCA source.

| Control | Section | Why it is primary |
|---|---|---|
| `oscrange` | Oscillator | The octave switch is reached constantly |
| `tune` | Oscillator | The original's TUNE knob |
| `glide` | Oscillator | Portamento is played, not configured |
| `pulsewidth` | Oscillator | Shapes the core tone |
| `pwmsource` `pwmdepth` | Oscillator | The original's PWM slider and its MAN/LFO/ENV switch. `pwmdepth` does nothing while the source is Manual, and is on the panel anyway — the rule is provenance, not whether a control currently does something |
| `subtype` | Oscillator | Belongs beside the sub's level, which is a top-five control (§2) |
| `vcolfo` | Oscillator | LFO to pitch is always live, and is half the LFO's reason for existing |
| `sawlevel` `pulselevel` `sublevel` `noiselevel` | Mixer | The four sources, compared against each other |
| `cutoff` `resonance` | Filter | The two primary performance controls (§2) |
| `filterenv` | Filter | Turns a static tone into a swept one (§2) |
| `filterlfo` | Filter | LFO to cutoff is core routing in §3's diagram, and always live |
| `keytrack` | Filter | The original's KYBD slider |
| `vcasource` | Amplifier | The original's VCA ENV/GATE switch |
| `attack` `decay` `sustain` `release` | Envelope | One envelope shapes both timbre and articulation |
| `retrigger` (Legato/Retrig) | Envelope | Reclassified from disclosed by the owner: the original's GATE / GATE+TRIG trigger modes are this same choice by another name |
| `lforate` `lfosync` `lfowave` | LFO | What the LFO *is*. `lfosync` is the collection's tempo sync, added 2026-09-25: the rate picks a division of the host's tempo |

**Behind a labelled disclosure — 1 control**, an addition the SH-101 did not have:

| Control | Section | What it adds to the original |
|---|---|---|
| `bendrange` | Oscillator | The original's bender had its own VCO and VCF depth sliders; a semitones-per-deflection value is the MIDI-era equivalent, set once per setup |

**In the app bar — 1 control**, always visible but on no card. Its section is where it sits in the
chain and in the `Parameters` list:

| Control | Section | Why it is in the app bar |
|---|---|---|
| `outgain` | Amplifier | The master output. Design system §3.1 item 6 puts it beside the level meter (owner, 2026-09-18), as an inline slider; level is checked constantly, and the bar is always on screen |

**Where a modulation depth lives** is a separate question from its surface, and the answer is: with
what it modulates, not with the LFO. `pwmdepth` is in Oscillator, `filterlfo` in Filter, `vcolfo` in
Oscillator. The LFO's own controls are what the LFO *is* — its rate and shape, on one row, with
the five shapes **drawn** rather than named — at the top of the *LFO and envelope* card, which also
holds the envelope and the amplifier (the owner, 2026-09-24): the six sections below are four cards
on the panel, *LFO and envelope*, *Oscillator*, *Mixer* and *Filter*.

Disclosure is a labelled expander, never a hover-reveal — §4.3 requires essential controls stay
reachable, and hidden-until-hover fails keyboard navigation.

## 6. Views

**Space-derived pages**, following design-system §3.2. *LFO and envelope* — the LFO, the
Envelope and the Amplifier as one card (the owner, 2026-09-24) — is Modulators, Oscillator is
Generators, and Mixer/Filter are Tone. The Oscillator's embedded
performance controls stay in its indivisible Generators card. Mixer/Filter remain a preferred
group; former cross-category pairings split. Full names, no fixed tab count, no bar for one page.
`Parameters` remains the separately formatted developer testing surface at CC 119 value 127,
never a musician tab. Controller roles/pages remain unchanged. The output level is on no card: it
is in the app bar beside the level meter (§5), where the keyboard cursor reaches it too.

## 7. Identity accent

**Deferred to M4a, chosen by measurement.** The reserved hues are unavailable: blue is `mod-lfo`,
red `mod-envelope`/`danger`, amber `mod-random`/`warning`, green `success`/`mod-performance`,
violet `mod-key-voice`, teal the default `accent`.

Candidates, to be tested in both themes: **magenta/orchid**, **lime/chartreuse**, **warm coral**.

Each candidate must record measured contrast ratios — **4.5:1 against `surface-1` for text, 3:1 for
control boundaries, in dark *and* light** — before selection. Any candidate failing either
threshold in either theme is discarded rather than adjusted by eye. §5.3 also forbids recreating the
source hardware's signature colour arrangement, which rules out the grey-blue-red combination.

Contrast results get recorded in this file once measured.

## 8. Live visualizations that materially improve understanding

Four, all earning their place under §1.3 and §9. **The test is whether a display answers a
question the controls cannot** — not what kind of display it is. An earlier version of this brief
excluded oscilloscopes and spectra outright; that was too blunt. A live scope of the output tells
you little here, because the filter and envelope already say what they are doing. A picture of what
the *mixer* produces tells you something four faders cannot.

1. **Filter response curve** — reacts to cutoff and resonance, and shows the envelope's modulation
   range as a secondary trace. This is the clearest possible answer to "what is the filter doing",
   and it makes the resonance-bass-loss behaviour visible instead of mysterious.
2. **Envelope shape** with a position indicator moving through it while a note sounds. With one
   envelope driving both destinations, seeing its shape is worth more here than in a synth with
   several.
3. **Output level with clip indication** in the app bar, per §3.1.
4. **The mixer's waveform** — one cycle of what the four sources add up to, in the Mixer card. Four
   levels are a set of numbers until you see the shape they make, and it is the only place pulse
   width's effect is visible as a shape rather than a percentage.

**Not excluded by kind.** A display earns its place by answering a question the controls do not.
A live output oscilloscope does not, *here*: the filter response and the envelope shape already
show what is being done to the sound, and a scope would mostly re-state them. That is a judgement
about this instrument, not a rule about scopes — one that earns its place is welcome.

### Ownership, because all three cross a thread boundary

A single `Telemetry` struct, `Arc`-shared: owned by the plugin, cloned into the editor. **Atomics
only** — no locks, no allocation, and the UI may drop frames. It lives in `plugins/mxm-mono-01`, not in
`mxm-mono-01-dsp`, which stays framework-free. All three stop updating when `EguiEditorState::is_open()`
reports the editor closed.

| Visualization | Writer | Transport | Truth model |
|---|---|---|---|
| Envelope position | Audio thread, once per block, via `Voice::env_level()` / `env_stage()` | `AtomicU32` (f32 bits) + `AtomicU8` stage | **Exact** — the value the DSP used |
| Output level + clip | Audio thread, once per block | Peak: `AtomicU32` (f32 bits), **max-combined**, reset when the UI reads it. Clip: sticky `AtomicBool`, set at `abs(x) >= 1.0`, cleared by the user | **Exact** — peak of the samples produced |
| Filter response | UI thread, computed | `cutoff`, `resonance`, `filterenv` from the parameters, plus sample rate from `Telemetry` (published once in `initialize`) | **Declared approximation**, below |
| Mixer waveform | UI thread, computed | The four source levels, pulse width and sub shape, from the parameters | **Declared approximation**: the ideal shape, not the antialiased one the oscillator renders. Noise is drawn as a fixed pseudo-random sequence so the picture does not shimmer while nothing is being changed |

The peak is **max-combined rather than overwritten**, and reset by the UI's read, so a dropped frame
cannot hide a transient. The value is always "loudest since you last looked".

### The filter curve's fidelity, stated

The curve is the **linear analytic 4-pole response at the current sample rate**. It ignores the
input drive stage and the resonance-feedback nonlinearity, so a measured sweep at high resonance or
high input level will not match it exactly.

Stated here rather than left as an implementation detail: an undeclared approximation is a bug
report from whoever compares the curve to a sweep. The linear model does show the resonance
bass-loss this visualization exists for, so the approximation serves its purpose.

## 9. What is removed from the source hardware layout, and why

**Kept:** section sequence and membership, because they are the signal flow.
**Removed:** appearance, geometry, control style, colour arrangement, typography, trade dress.

Someone who has used an SH-101 must recognise the *layout* — where things are and what sits next to
what. Nothing about how it looks is borrowed.

| Removed | Why |
|---|---|
| The panel layout itself | §2 forbids copying the inspiring instrument's panel. Controls are grouped by task and signal flow instead |
| Slider-per-parameter geometry | The hardware used sliders because they were cheap and physical. On screen, knobs are more compact for a 27-parameter instrument, and sliders are kept only where range comparison matters (mixer levels, ADSR) |
| Physical switch appearance | Replaced by segmented controls (§7.3), which must not imitate mechanical switches |
| The keyboard | §2 forbids a decorative keyboard, and on-screen note input is not a tested feature of v1 |
| Sequencer and arpeggiator | Out of scope for v1; they roughly double the UI surface and affect none of the core sound |
| Vintage typography, captions and colour arrangement | §2 and §5.3 — trade dress |
| The hand-grip/strap hardware identity | Not an interface element in any sense |

What is **kept** is architectural, not visual: one oscillator with sub and noise, the four-source
mixer, the 4-pole filter, one shared envelope, one LFO, and the mono voice's legato behaviour.

## 10. Minimum size and 200% scale behaviour

**Resizable: the editor's `REFERENCE` and `MINIMUM`, derived and held by its tests**, the minimum
one widest card plus both gutters. Modulation routing raised the opening height by its
`‹ modulate ›` lines and route rows. Category order is §6's; parameter membership stays
§5's. `every_dynamic_page_fits_and_every_card_is_reachable` checks all pages with Advanced reserved
open, both themes, at opening size, the quarter-4K content size and the minimum. Tall component
canvases retain card-floor and row proofs, not physical fit claims. Zoom stays independently chosen
at **75–200%**; only indivisible overflow scrolls. Keep the physical window fixed for §15's DPI/zoom
gate. Native-window, real-DAW and owner inspection remain open.

---

## 11. The recognisability trial

§9 makes a recognisable layout a requirement. Unlike contrast it has no measurement, and §15 can
pass while someone familiar with the instrument still cannot find anything. Conformance to §10's
sequence is **not** the gate — that only proves the editor matches this brief, and if the sequence
itself is wrong every check passes while the requirement fails. So the gate asks a person.

**The trial patch**, so results are comparable and no task is inert:

| | |
|---|---|
| Oscillator | range 8', tune 0, glide 0, pulse width 50% |
| Mixer | saw 80%, pulse 0, sub 0, noise 0 |
| Filter | cutoff 4 kHz, resonance 10%, filter env 0%, key track 0 |
| Envelope | A 5 ms, D 300 ms, S 70%, R 200 ms |
| LFO | rate 4 Hz, triangle |
| Amplifier | source Env |
| App bar | output 0 dB |

Run with **someone who has used an SH-101**, with no reference to the source hardware's panel.

### Stage 0 — four cards, before anything is drawn

The four card names on cards — *LFO and envelope*, *Oscillator*, *Mixer*, *Filter* (the owner merged
the LFO, Envelope and Amplifier on 2026-09-24, before this trial ran): *"lay these out in the
order you would expect to find them."*

**Pass:** the order matches the editor's. **Any other difference fails, and the card order is
revised.**
This is the only stage that can still change the architecture, and it costs a piece of paper.

### Stage 1 — on a wireframe, before responsive composition

Sections and controls placed in stage 0's confirmed order; no styling, no breakpoints. Three
questions that cannot be answered from labels:

1. *"Trace the sound from where it starts to where it leaves."* → the chain, in order.
2. *"Which sections does the envelope affect?"* → **both** filter and amplifier. This is §3's
   defining constraint; one answer is a fail.
3. *"With the labels covered — where would you expect the sub's level?"* → with the mixer's other
   three sources.

### Stage 2 — on the finished editor

Seven location tasks, each with exactly one correct control from the trial patch, together touching
all four cards and the app bar:

| Task | Control | Section |
|---|---|---|
| *Make it brighter.* | `cutoff` | Filter |
| *Make it whistle at the edge.* | `resonance` | Filter |
| *Add weight underneath.* | `sublevel` | Mixer |
| *Slow the wobble down.* | `lforate` | LFO and envelope |
| *Let it ring on after I lift my finger.* | `release` | LFO and envelope |
| *Drop it an octave.* | `oscrange` | Oscillator |
| *Quieter, without changing the sound.* | `outgain` | App bar |

**Per task:** the correct control identified within **ten seconds**, entering at most one wrong
section on the way. **Gate: six of seven.** Stage 2 confirms composition did not undo stage 0 and 1.

Results are recorded below, beside the contrast measurements. **If no SH-101-familiar person is
available, the gate is recorded as unmet** — an unrun trial is not a passed one.

*Results: not yet run.*

---

## Deliberate deviations from the design system

Recorded explicitly so they are decisions rather than oversights.

### Undo/redo omitted from the app bar (§3.1, §15)

mxm-mono-01 has **no destructive actions and no modulation routing**. Its only undoable events are
parameter edits, which every major host already tracks in its own undo history. A plugin-level undo
stack would duplicate that and can behave confusingly when the two histories disagree.

**Decision:** no app-bar undo/redo in v1. This is revisited when an MXM instrument gains genuinely
plugin-local undoable actions — preset editing, modulation assignment, or destructive operations —
at which point it belongs in mxm-kit's `crates/ui` for the whole collection rather than in one plugin.

### No preset *browser* (§3.1), but there is an Init action

§3.1 requires preset navigation and favourite/save actions "when presets are supported". v1 supports
none: no browser, no save, no favourites. The instrument ships a good default patch and documented
recipes in its README instead.

**One action does exist**: **Init**, which returns every control to that default patch — the contract
in `plugins/AGENTS.md`, *Every instrument has an init patch*. It sits in the **global utility menu**
(§3.1 slot 5), not in a preset slot, for two reasons: §3.1 assigns Init no slot of its own and
permits *"rare actions belong in the utility menu"*, and `mxm_ui::AppBar::show` exposes only slots 5
and 6 — so a slot of its own would mean designing a shared app-bar API for a single button.

The app bar reserves the space for a browser, so adding one later does not restructure the shell.
When it arrives it takes slots 2–4 and Init moves beside the preset actions.

---

## Sign-off checklist for M4b

- [ ] Identity accent chosen, with measured contrast ratios recorded above
- [ ] Signal flow readable left-to-right without documentation
- [ ] Shared-envelope relationship stated in the envelope display's hover text
- [ ] All five priority parameters at Primary/Standard sizing
- [ ] **Section sequence and membership match §10, for all 21 parameters, and every routing pair sits on its target's card**
- [ ] ~~**The `Synth` view does not reflow at any window size**~~ — superseded: every view reflows, and the gate is now that the *sequence* never changes and every row ends on one line
- [ ] **Every parameter at the surface §5 classifies it — 25 primary, 1 disclosed, 1 in the app bar**
- [ ] **Nothing the SH-101 had on its panel is behind a disclosure**
- [ ] **Both views reachable from the view bar, `Synth` active on open**
- [ ] **§11's trial run and recorded — stage 0 before the wireframe, stage 1 before composition,
      stage 2 on the finished editor — pass or unmet, never skipped**
- [ ] Verified at 820×560, 1200×760, and 200% scale
- [ ] Dark and light both complete, with all control states
- [ ] §15 QA gate passed in full

**Three claims, not one.**

- **Standalone-validated.** Everything above, through `apps/mxm-mono-01-standalone`.
- **Floating-host-validated, on Windows.** `apps/mxm-player` opens the editor as a floating window
  the plugin owns: `create`, `show`, `destroy`, and reopening afterwards. Linux and macOS build in
  CI and are **UNVERIFIED**. *Since the split (2026-10-06):* CI builds and tests all three on a
  release tag; the floating editor has still not been opened on Linux or macOS.
- **Embedded-host-validated — UNMET.** Host parenting, host-driven resize and scale changes never
  run: the player hosts floating windows only, and embedding is unsupported. So is
  **third-party floating hosting** — the vendored nice-plug (the MXM fork since the split) now advertises floating to *every*
  host, and `get_preferred_api` is only a hint a DAW may ignore, so a DAW may take that path. There
  is no DAW here to find out.
