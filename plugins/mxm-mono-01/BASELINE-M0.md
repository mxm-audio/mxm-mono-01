# mxm-mono-01 — pre-conversion reference, captured at M0

`plans/plan-modulation-routing.md` (in the private archive) M0. **These figures stop existing once the conversion starts**,
which is why they are captured first and committed rather than re-derived later.

Produced by `plugins/mxm-mono-01/src/lib.rs`'s `baseline` module:

```bash
cargo test -p mxm-mono-01 --release baseline -- --ignored --nocapture
```

Release only, as mxm-kit's `crates/ui/tests/flow_resize_bench.rs` says of its own bench.

## Throughput — the owner's cost gate

The plugin's own per-sample path: `next_patch()` per sample, which advances every smoother and
rebuilds the `Patch`, then `Voice::process`. It **omits** the wrapper's per-block event handling and
buffer plumbing, which is not where the routing work lands — said plainly rather than implied.

| | |
|---|---|
| Rate, block | 48 000 Hz, 64 samples |
| Patch | Init, one held note (48) |
| **Cost** | **≈ 206 ns/sample**, ≈ 4.83 M samples/s, ≈ 100× realtime |
| Spread over four runs | 205.95, 206.36, 206.37, 206.93 ns/sample — under 0.5 % |

**Not portable across machines.** Windows development machine, `cargo` release profile; the same
hedge mxm-bucket-delay's `crates/mxm-bucket-delay-dsp/AGENTS.md` states for its own probe. What it is good for is a
before/after comparison **on this machine**, which is what the gate needs.

**The claim it will be held to:** with the init patch loaded, throughput after the conversion is not
lower than 206 ns/sample here. §6.2 of the plan enumerates what the design adds and removes; neither
list settles it, which is why this number exists.

## Factory bank — fifty reference digests

FNV-1a over the raw sample bits, the digest `apps/mxm-player/tests/t4_golden_audio.rs:81` already
uses. One note (48) held 1.5 s then released with 2.5 s of tail, identical for every preset so a
digest change is attributable to the patch and not the gesture.

**Verified reproducible**: two consecutive runs produced identical digests for all fifty.

Every preset applied **all 27 parameters** the instrument had at M0, which independently confirms the bank's full-coverage
property — the same property that becomes load-bearing once absent parameters would leave stale
routing behind (plan §8).

| Sound | Digest | Peak |
|---|---|---|
| `acid-line` | `d07f201d6522c2bb` | 0.1464 |
| `bell` | `736ea91d0e2db3e7` | 0.2402 |
| `blip` | `4e362b02097d2adb` | 0.1362 |
| `brass` | `ae30e7c5260855f5` | 0.3335 |
| `breath-pad` | `38c603e196db504c` | 0.2415 |
| `bright-lead` | `1e40816f8235a3bf` | 0.3214 |
| `cello` | `b64a88b4f16508d4` | 0.3539 |
| `choir-pad` | `3e77e5bb4a06c6ff` | 0.3104 |
| `clav` | `dd22e61fed50d45f` | 0.2286 |
| `dark-pad` | `2cbadcbbaaf46102` | 0.4792 |
| `deep-sub` | `1885e73da2624b32` | 0.4098 |
| `drone` | `1bf8a1abd01ceb96` | 0.4608 |
| `electric-piano` | `ce1a484c45518006` | 0.3153 |
| `glass-pad` | `30463f1088cc95ee` | 0.3380 |
| `glide-lead` | `9c10edb2618bb1a4` | 0.3744 |
| `growl-bass` | `40ab40e5b314b4cc` | 0.1464 |
| `harp` | `b80404a3a47aa91a` | 0.2689 |
| `hat` | `5a7c3878883ef300` | 0.2356 |
| `hollow-lead` | `267dec54cc2a4df3` | 0.2814 |
| `kick` | `8069f589f60b2656` | 0.2392 |
| `laser` | `66ef9d9a417a220f` | 0.1489 |
| `marimba` | `be59a133964ccc54` | 0.2775 |
| `music-box` | `685803c7e730cc23` | 0.2388 |
| `noise-sweep` | `9741d9473d1aecf1` | 0.4830 |
| `octave-lead` | `35b0bac7f26ffc75` | 0.4888 |
| `organ` | `a1b22519e0f951dd` | 0.5984 |
| `pick-bass` | `e2f6c1fabe034ebf` | 0.2036 |
| `plucked-bass` | `d25cc1bcd8fa931c` | 0.1979 |
| `pulse-gate` | `7ceb69aa0ce0cc71` | 0.2872 |
| `rain` | `58aa40aa6d4790c0` | 0.2445 |
| `ramp-sweep` | `8915aa5b057a267c` | 0.3019 |
| `reed-lead` | `c4906463f2b752ef` | 0.2187 |
| `round-bass` | `cbcf29ffda43dad3` | 0.5673 |
| `rubber-bass` | `de71479cf5586036` | 0.2454 |
| `screaming-lead` | `1abbff68ce77ea57` | 0.6932 |
| `siren` | `a78dacdded3ab78f` | 0.3202 |
| `snare` | `099ded921e10dcfc` | 0.2128 |
| `soft-pad` | `2ec0d41961675f31` | 0.2598 |
| `square-lead` | `19e2dd834726de52` | 0.3535 |
| `string-pad` | `73aa9f0d77cd0f45` | 0.3588 |
| `sub-bass` | `39b8a794efbc570a` | 0.5552 |
| `sweep-pad` | `0bfeeed8b22d9571` | 0.3369 |
| `swell` | `b1849f3f54c6079b` | 0.3534 |
| `tom` | `b4ddef21224ab642` | 0.1771 |
| `vibrato-lead` | `0174e85bbac6e242` | 0.3149 |
| `violin` | `084663a1d209bf38` | 0.3059 |
| `warm-pad` | `a8543de8b91976a0` | 0.4803 |
| `whistle-lead` | `cbd424e957a2e58d` | 0.3434 |
| `wind` | `94ed7df69dd87258` | 0.1418 |
| `wood-pluck` | `bcf0742219f19337` | 0.2710 |

Every peak is non-zero and below unity — audible and not clipping, which is what
`tests/the_factory_bank.rs` asserts of the shipped bank.

## After the conversion — measured 2026-09-10

The pilot's modulation is routing now, and **the machine's own five paths are routes in the init
patch**: `vcolfo`, `pwmdepth` and its source switch, `filterenv`, `filterlfo` and `keytrack` are
retired ids, and the pairs that replaced them are present at zero depth.

| | Before | After |
|---|---|---|
| Init patch | ≈ 206 ns/sample | **≈ 224 ns/sample** (223.7) |
| The player's golden digest, `b0791ab43fe7c079` | — | **unchanged** |
| Fifty factory peaks | — | **all fifty identical to four decimal places** |
| Fifty factory digests | — | **23 unchanged, 27 moved** |

### The fifty digests after the conversion

| Sound | Digest | Peak |
|---|---|---|
| `acid-line` | `e073516ed86d85e4` | 0.1464 |
| `bell` | `a13e63f53f413d8d` | 0.2402 |
| `blip` | `de3fc346de8d59f3` | 0.1362 |
| `brass` | `7a17f8026a98a148` | 0.3335 |
| `breath-pad` | `db7abaded0ca194c` | 0.2415 |
| `bright-lead` | `d70ee1254ca16150` | 0.3214 |
| `cello` | `8acc0dac9898bfb4` | 0.3539 |
| `choir-pad` | `f0ab74fabcdd3383` | 0.3104 |
| `clav` | `dd22e61fed50d45f` | 0.2286 |
| `dark-pad` | `2cbadcbbaaf46102` | 0.4792 |
| `deep-sub` | `1885e73da2624b32` | 0.4098 |
| `drone` | `1bf8a1abd01ceb96` | 0.4608 |
| `electric-piano` | `3f6bec0bddf2961c` | 0.3153 |
| `glass-pad` | `2ebae342b42df379` | 0.3380 |
| `glide-lead` | `9c10edb2618bb1a4` | 0.3744 |
| `growl-bass` | `331464a5dae235f4` | 0.1464 |
| `harp` | `35055715c6151fc3` | 0.2689 |
| `hat` | `5a7c3878883ef300` | 0.2356 |
| `hollow-lead` | `9313c69f6d0df07d` | 0.2814 |
| `kick` | `8069f589f60b2656` | 0.2392 |
| `laser` | `66ef9d9a417a220f` | 0.1489 |
| `marimba` | `be59a133964ccc54` | 0.2775 |
| `music-box` | `c577b60366e9a978` | 0.2388 |
| `noise-sweep` | `cca95afbafc62035` | 0.4830 |
| `octave-lead` | `35b0bac7f26ffc75` | 0.4888 |
| `organ` | `a1b22519e0f951dd` | 0.5984 |
| `pick-bass` | `e2f6c1fabe034ebf` | 0.2036 |
| `plucked-bass` | `d25cc1bcd8fa931c` | 0.1979 |
| `pulse-gate` | `7ceb69aa0ce0cc71` | 0.2872 |
| `rain` | `58aa40aa6d4790c0` | 0.2445 |
| `ramp-sweep` | `9e9592c497610d0c` | 0.3019 |
| `reed-lead` | `c4906463f2b752ef` | 0.2187 |
| `round-bass` | `cbcf29ffda43dad3` | 0.5673 |
| `rubber-bass` | `6d5148d9d34faf9d` | 0.2454 |
| `screaming-lead` | `6f7dccf1d00d724b` | 0.6932 |
| `siren` | `a78dacdded3ab78f` | 0.3202 |
| `snare` | `099ded921e10dcfc` | 0.2128 |
| `soft-pad` | `e6526759cc9b088b` | 0.2598 |
| `square-lead` | `53684ef5416e28f9` | 0.3535 |
| `string-pad` | `0b6c13f58809908d` | 0.3588 |
| `sub-bass` | `39b8a794efbc570a` | 0.5552 |
| `sweep-pad` | `6871aa4442fb002d` | 0.3369 |
| `swell` | `b1849f3f54c6079b` | 0.3534 |
| `tom` | `0dccf7fe86233803` | 0.1771 |
| `vibrato-lead` | `fc50d66a715aa4d5` | 0.3149 |
| `violin` | `1373371ad66707ae` | 0.3059 |
| `warm-pad` | `a8543de8b91976a0` | 0.4803 |
| `whistle-lead` | `c1aa1b6a338034a3` | 0.3434 |
| `wind` | `a73f3f22e44f11f1` | 0.1418 |
| `wood-pluck` | `bcf0742219f19337` | 0.2710 |

### What moved, and why it is rounding

**Each route reproduces the knob it replaced bit-for-bit.** Measured between the commit that added
the routes and the one that deleted the hard-wired path: worst sample deviation exactly `0` over
24 000 samples at a depth of 0.6, for all five, key tracking included at five different notes.
`crates/mxm-mono-01-dsp/tests/equivalence.rs` records the table.

Two things bought that, and both are load-bearing. **§5's conservative form**: a route the machine
itself wires keeps the scale it always had, so the filter LFO still reaches four octaves where the
filter envelope reaches six, and an old depth is the same number on the route that replaced it — no
patch had to be re-dialled. And **§6.2's multiply order**, `(amount × source) × scale`, which is the
instruction sequence the voice already executed; an earlier revision of the params layer folded the
scale into the amount, and every digest moved.

**What did move is the order three terms are added in.** The cutoff was one written expression,
`key + envelope + LFO`; it is a sum over live routes now, and those run in declared source order —
LFO, envelope, key. Floating-point addition is not associative, so a patch with **two or more**
cutoff routes lands on a different last bit and a patch with one does not, which is exactly the
23/27 split. `reordering_the_cutoff_terms_moves_only_the_last_bits` measures it: under a thousandth
of a cent of cutoff. Every peak is identical to four decimal places.

The digests above are re-pinned rather than the sum order contorted to match a legacy expression.
**Re-pinned, not listened to** — that is the owner's at M4.

### The cost gate

**≈ 224 ns/sample against 206, about 9 % over**, and reported rather than tuned away. The init patch
is no longer the free case: five routes are live in it, so the frame is opened, three sources are
published and four sums are taken every sample where the old code ran five inline multiply-adds.

Three fixes on the way, and the numbers are worth keeping because two of them were invisible without
measuring:

| | ns/sample |
|---|---|
| Nothing routed at all — the old free case | 210 |
| Five routes live, first working version | 263 |
| … publishing only the sources some route reads | 257 |
| … and compacting the live pairs at topology time | **224** |

**A fourth fix followed, on the owner's question — *"even if there are 1000 sources but only 2 are
connected, we should only ever need to sum 2?"*** The sum always did; `SourceFrame` did not.
Opening a sample copied `current` into `previous` and cleared an N-slot written array, which is work
proportional to how many sources the instrument *has*. It is now one counter increment: an unwritten
source's `current` slot already holds last sample's value, so the unit-delay read needs no branch and
no per-sample bookkeeping, and `previous` is saved lazily by the first write of each sample. On this
machine, three runs each: **150.9 → 146.2 ns/sample, about 3.1%**, with every digest unchanged
because `read` returns exactly what it returned before. The figures in this document are from
another machine and are not comparable to those two; the ratio is.

The largest single cost was the least obvious: **walking all forty-four pairs every sample to find
the five that are live**, 37 ns of it, more than everything else the routing added put together.
Topology is discrete and changes only on a parameter event, so that list is compacted once per
interval like every other compaction here.

What remains is the frame's own bookkeeping and the four sums. **Topology is still re-read once per
buffer** even when nothing has changed, which could be skipped if a parameter event were the only
way a parameter could move — but a host **state restore changes parameters without sending events**,
and a stale topology would silently drop a route from a loaded project. A silent wrong-sound defect
is worse than the read.

An intermediate revision measured **295 ns/sample**, 43 % over, from two mistakes worth recording
because they are the obvious ones: `Routing` was a field of `Patch`, which is rebuilt per sample, so
220 bytes were copied 48 000 times a second for a patch with nothing routed; and the voice published
its sources and took its sums whether or not anything was live.

## What this does not establish

- **Nothing about how it sounds.** A digest proves *unchanged*; it cannot prove *good*, and the
  owner's listening gate at M4 is not inferable from it.
- **Nothing about the bundle or the host.** This is the library, in process. The player's own golden
  (`apps/mxm-player/tests/t4_golden_audio.rs`, digest `b0791ab43fe7c079`) covers the real bundle at
  its defaults and must also not move. *Since the split (2026-10-06):* that test is this
  repository's `plugins/mxm-mono-01/host-tests/tests/golden_audio.rs`, same digest, pinned on
  Windows only.
- **Nothing about the other seven instruments.** Each captures its own baseline before its own
  conversion.
