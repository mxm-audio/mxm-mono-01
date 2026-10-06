# AGENTS.md — apps/mxm-mono-01-standalone

Parent: [`../../AGENTS.md`](../../AGENTS.md)

# Purpose

Runs mxm-mono-01 with its real editor, outside a host. It is `src/main.rs` and nothing else: one call
to nice-plug's `nice_export_standalone`, which supplies audio through cpal, MIDI through midir, and
opens `Plugin::editor` in a window it owns.

**It is no longer the only way to see the editor** — `apps/mxm-player` now opens it as a floating
window. What this crate still gives you is the editor *without a host at all*: no plugin instance
lifecycle, no CLAP handshake, no host state to rule out when something looks wrong. That makes it
the right place to work on the editor itself, and the wrong place to test hosting.

The windowing is entirely in the vendored nice-plug, for all three platforms; this crate writes
none of it.

# Ownership

Owns `src/main.rs` and `Cargo.toml`. Owns no interface: the editor belongs to
[`plugins/mxm-mono-01`](../../plugins/AGENTS.md) and the controls to
[`crates/ui`](https://github.com/mxm-audio/mxm-kit/blob/main/crates/ui/AGENTS.md). **If this crate ever grows a widget, it is in the wrong
place.**

# Local Contracts

## It is a separate crate on purpose, and outside `default-members`

The obvious shape — a `[[bin]]` inside `plugins/mxm-mono-01` — is what nice-plug's own documentation
suggests, and it is wrong here. The root `Cargo.toml` states the rule it would break:

> *"Kept for build time, NOT for MSRV: a plain `cargo build` should not pull in eframe, cpal and
> midir."*

`plugins/mxm-mono-01` **is** a default member. nice-plug's `standalone` feature adds eight crates —
`clap`, `cpal`, `jack`, `midir`, `rtrb`, `fixed-resample`, `audioadapter-buffers`, `ctrlc` — and
`wrapper/standalone/backend.rs` declares `mod jack;` unconditionally, so it link-depends on libjack
whether or not anyone runs JACK. A bin in the plugin would put all of that into the shipped
`.clap`.

So this crate mirrors `apps/mxm-player`: a separate member, excluded from `default-members`.
`plugins/mxm-mono-01` gains only `crate-type = ["cdylib", "lib"]` and no dependencies.

**Cargo unifies features across a single build graph**, so this is a property of *which* crates are
built together, not a wall. `cargo build --workspace` does include this crate. The check is
`cargo tree -p mxm-mono-01`, which must show no audio backend — run it by hand when this crate or
the plugin's dependencies change, since no CI is used (root *Windows, Linux and macOS*) —
because the failure mode is silent: the bundle would still build, still load and still work, while
carrying cpal and a libjack link into every user's DAW.

## Linux needs libjack

`libjack-jackd2-dev`. The `jack` crate is taken with default features, so it links rather than
loading at runtime. If that becomes awkward on a runner, the alternative is the crate's
runtime-loading path — but that is a change to a vendored dependency's feature set, so it is a
decision, not a fix to apply quietly.

## What running the editor here proves, and what it does not

**Proves:** layout and section sequence, parameter coverage, reflow at every breakpoint and scale,
measured contrast, the `Synth`/`Parameters` switch, telemetry under a live audio thread, and — in a
debug build — that `process()` still allocates nothing with the editor open.

**Does not prove the CLAP path.** This wrapper *creates and owns* its window; under CLAP the editor
is handed a parent and is resized and scaled **by the host**. Host parenting, host-driven resize,
scale changes and open/close ordering never run here, and they are where embedded editors typically
break. `docs/briefs/mxm-mono-01.md`'s M4b sign-off records that gate as **unmet**, and it stays unmet
until the editor runs in a real host. Do not let the harness be described as standing in for one.

**And it is built from the library, not from the bundle.** That is the useful part — it shows an
editor change the moment it compiles — and it is also a trap, because `target/bundled/mxm-mono-01.clap`
is a separate artifact that `cargo build` never touches. Verify a change here and the plugin someone
opens can still be the previous build; the two screenshots disagree and only one of them is the
product. Run `cargo xtask bundle mxm-mono-01 --release` before believing the plugin matches. See
[`plugins/AGENTS.md`](../../plugins/AGENTS.md)'s verification section.

## The wrapper is unproven ground

Nothing in this repository used nice-plug's standalone path before M4b, and the copy in
[`vendor/nice-plug`](https://github.com/mxm-audio/newdawn-workspace/blob/main/vendor/AGENTS.md) is patched. A failure here is as likely to be the
wrapper as the editor — check `vendor/nice-plug/src/wrapper/standalone/` before assuming the
editor is at fault.

# Work Guidance

- `cargo run -p mxm-mono-01-standalone -- --help` lists the backend, device and sample-rate options.
- `--backend dummy` opens the editor with no audio device, which is the right choice for looking at
  layout, contrast or reflow.
- A **debug** run is the one that checks allocations: `assert_process_allocs` only fires in debug.

# Verification

```bash
cargo run -p mxm-mono-01-standalone -- --backend dummy
cargo run -p mxm-mono-01-standalone --release
```

This verifies the standalone-owned window only. Bundle and real-host verification remain under the
plugin contract.

# Child DOX Index

No child `AGENTS.md` files.
