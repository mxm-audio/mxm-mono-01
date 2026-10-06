# mxm-mono-01

A monophonic subtractive synthesizer. Architecture inspired by the Roland SH-101 — one oscillator, a
mixer, a four-pole filter, one envelope and an LFO — but the interface is not, and the name is not.
Not affiliated with or endorsed by Roland.

**The synth only.** There is no sequencer or arpeggiator: those are [MXM Player](https://github.com/mxm-audio/mxm-player/blob/main/apps/mxm-player)'s
or your DAW's, by the collection's rule that an instrument does not carry what the host already does.
No effects, because the machine has none.

## What it is

| | |
|---|---|
| Oscillator | **One phase, read three ways** — sawtooth, pulse and a sub-oscillator all follow the same core, so there is nothing to detune — plus noise. Four footages, 16' to 2' |
| Sub-oscillator | One octave square, two octaves square, or two octaves pulse |
| Mixer | A level each for saw, pulse, sub and noise |
| Filter | Four poles, with the resonance feedback limited by a **back-to-back diode clamp** rather than a smooth curve: linear below the knee, firm above it |
| Envelope | **One ADSR, shared** by the filter and the amplifier. The amplifier's source switches between that envelope and the gate |
| LFO | Triangle, square, ramp up, ramp down and random |
| Glide | Always engaged, so its time is how much portamento there is; zero is off |
| Keyboard | Legato or retrigger, with the voice returning to an older held key when the newest is released |

## Modulation

Four targets — **pitch, pulse width, cutoff and amplitude** — and eleven sources: the LFO, the
envelope, five performance inputs, and the instrument's own saw, pulse, sub and noise, which is what
makes FM reachable without inventing a generator.

**The machine's own modulation is routing.** Vibrato, pulse-width modulation, the filter envelope,
the filter LFO and key tracking are routes present in the init patch at zero depth, each at the scale
its hardware path always had, so turning one up is the SH-101's own knob. Removing a route is one
click, and re-adding it restores the depth it had. The mod wheel scales the vibrato.

Routing is drawn beneath the control it moves: pitch and pulse width on the Oscillator card, cutoff on
the Filter, amplitude on the Amplifier.

## Presets

Fifty factory sounds, compiled into the plugin — copy the `.clap` alone and the sounds travel with
it. **Init is not a file**: it is generated from the parameter defaults, so it cannot be deleted and
cannot drift from them. Your own presets are saved as readable JSON under the platform config
directory, browsed from the same bar, and can be starred to sort first.

## Status

**The editor shipped.** Six cards whose pages derive from the window's size, the keyboard cursor, the
envelope and filter displays of [its brief](../../docs/briefs/mxm-mono-01.md), and in the app bar a
preset browser, the output level beside its meter, and the scale and theme controls.

Validated in the standalone harness and as a floating editor in MXM Player on Windows. **A real DAW —
embedded hosting, host-driven resize and scale — is a recorded unmet gate**, and on Linux and macOS
the editor has not been opened by hand (CI builds and tests all three platforms on a release tag).

## Building

```bash
cargo xtask bundle mxm-mono-01 --release
clap-validator validate "target/bundled/mxm-mono-01.clap"
cargo run -p mxm-mono-01-standalone      # the editor with no host at all
```

GPL-3.0-or-later — see the repository's [`LICENSE`](../../LICENSE). All code is original.
