//! **The routes reproduce the wiring they replaced**, and the init patch is unchanged by their
//! presence. `plans/plan-modulation-routing.md` §6.3 B and decision 1.13.
//!
//! # What ran here and is no longer runnable
//!
//! Between the commit that added the machine's own wiring as routes and the commit that deleted the
//! hard-wired path, this file rendered both from the same patch and the same note and measured the
//! distance. **All five were bit-identical** — worst sample deviation exactly `0` over 24 000
//! samples at a depth of 0.6, for LFO-to-pitch, PWM depth from the LFO, filter envelope, filter LFO,
//! and key tracking at notes 36, 48, 60, 72 and 84:
//!
//! | Was | Is | Scale | Deviation |
//! |---|---|---|---|
//! | `vcolfo` | (pitch, LFO) | 7 semitones | `0` |
//! | `pwmdepth` + `pwmsource` | (pulse width, LFO) and (pulse width, envelope) | ± 0.45 width | `0` |
//! | `filterenv` | (cutoff, envelope) | 6 octaves | `0` |
//! | `filterlfo` | (cutoff, LFO) | 4 octaves | `0` |
//! | `keytrack` | (cutoff, key) | 5 × the five-octave key source | `0` |
//!
//! **An old depth is the same number on the route that replaced it**, which is why no patch had to
//! be re-dialled and why the fifty factory digests could be held rather than re-pinned. Two things
//! bought that and both are load-bearing: §5's conservative form, so a route the machine itself
//! wires keeps the scale it always had rather than taking the target's declared one; and §6.2's
//! multiply order, `(amount × source) × scale`, which is the instruction sequence the voice already
//! executed.
//!
//! The comparison could only exist while both paths did. What survives is below.

use mxm_mono_01_dsp::oscillator::MixLevels;
use mxm_mono_01_dsp::routing::{FULL_SCALE, INIT_PRESENT, Routing, source, target};
use mxm_mono_01_dsp::voice::{
    FILTER_ENV_OCTAVES, FILTER_LFO_OCTAVES, NoteId, PWM_WIDTH_SWING, Patch, Retrigger,
    VCO_LFO_SEMITONES, Voice,
};

const FS: f32 = 48_000.0;

fn patch() -> Patch {
    Patch {
        cutoff_hz: 2_000.0,
        sustain: 0.7,
        attack_s: 0.002,
        decay_s: 0.3,
        release_s: 0.1,
        lfo_rate_hz: 5.0,
        output_gain: 1.0,
        levels: MixLevels {
            pulse: 0.8,
            ..MixLevels::default()
        },
        ..Patch::default()
    }
}

fn render(p: &Patch, r: &Routing, note: u8, n: usize) -> Vec<f32> {
    let mut v = Voice::new();
    v.set_sample_rate(FS);
    v.reset();
    v.note_on(
        NoteId {
            voice_id: None,
            channel: 0,
            note,
        },
        Retrigger::Always,
    );
    v.set_topology(r);
    (0..n).map(|_| v.process(p, r)).collect()
}

/// **A fresh instance is the instrument it always was.** The machine's own five routes are present
/// in the init patch, and present at zero depth is exactly nothing — which is the whole reason the
/// conversion could keep every factory digest.
#[test]
fn the_init_patch_is_bit_identical_with_the_machines_wiring_present_at_zero() {
    let p = patch();
    let bare = render(&p, &Routing::new(), 48, 24_000);
    let wired = render(&p, &Routing::init(), 48, 24_000);
    assert_eq!(
        bare, wired,
        "the machine's own routes at zero depth must add exactly nothing"
    );
    assert!(bare.iter().any(|x| x.abs() > 1e-4), "and it must sound");
}

/// Each wired route's scale is **the constant its knob carried**, named rather than re-typed.
///
/// This is what makes the table above a statement about the machine rather than about five numbers
/// that happened to agree on the day: change one of these constants and the route that replaced the
/// knob moves with it.
#[test]
fn each_wired_routes_scale_is_the_constant_its_knob_carried() {
    assert_eq!(FULL_SCALE[target::PITCH][source::LFO], VCO_LFO_SEMITONES);
    assert_eq!(
        FULL_SCALE[target::PULSE_WIDTH][source::LFO],
        PWM_WIDTH_SWING
    );
    assert_eq!(
        FULL_SCALE[target::CUTOFF][source::ENVELOPE],
        FILTER_ENV_OCTAVES
    );
    assert_eq!(FULL_SCALE[target::CUTOFF][source::LFO], FILTER_LFO_OCTAVES);
    // One octave of cutoff per octave of keyboard, through a source normalised over five octaves
    // either side of middle C. That is what `key_track` meant.
    assert_eq!(FULL_SCALE[target::CUTOFF][source::KEY], 5.0);
}

/// The init patch wires **exactly** the machine's own paths and nothing else.
///
/// A sixth presence here would be a modulation the SH-101 never had, arriving switched on.
#[test]
fn the_init_patch_wires_the_machines_own_paths_and_no_others() {
    let init = Routing::init();
    for (t, target_routes) in init.present.iter().enumerate() {
        for (s, &present) in target_routes.iter().enumerate() {
            assert_eq!(
                present,
                INIT_PRESENT.contains(&(t, s)),
                "target {t} from source {s} is present={present} at init, which is not the wiring"
            );
        }
    }
    assert_eq!(INIT_PRESENT.len(), 5);
}

/// An amount of one on a wired route reaches **exactly** what its knob reached at full.
///
/// Rendered rather than asserted on the constant: the scale has to survive the whole per-sample
/// path, and a table that is right while the sum applies it in the wrong order proves nothing.
#[test]
fn a_wired_route_at_full_depth_reaches_the_knobs_full_scale() {
    // Six octaves of filter envelope, from a 200 Hz cutoff, at a held sustain of one.
    let mut p = patch();
    p.cutoff_hz = 200.0;
    p.sustain = 1.0;
    p.attack_s = 0.001;
    p.levels.pulse = 0.0;
    p.levels.saw = 0.8;

    let mut r = Routing::new();
    r.present[target::CUTOFF][source::ENVELOPE] = true;
    r.amounts[target::CUTOFF][source::ENVELOPE] = 1.0;

    // With the envelope at its sustain of one the cutoff sits six octaves up, which is 12.8 kHz
    // against 200 Hz — above the saw's harmonics at this pitch, so the tone is open rather than
    // filtered. Compared against the same patch with the route absent, which is 200 Hz and dark.
    let open = render(&p, &r, 48, 24_000);
    let dark = render(&p, &Routing::new(), 48, 24_000);
    let energy = |v: &[f32]| v.iter().map(|s| s * s).sum::<f32>();
    assert!(
        energy(&open) > energy(&dark) * 4.0,
        "six octaves of filter envelope should open the filter a long way: {} against {}",
        energy(&open),
        energy(&dark)
    );
}

/// **What did move, and by how much.** Twenty-seven of the fifty factory digests, and this is why.
///
/// The cutoff used to be one written expression, `key + envelope + LFO`. It is a sum over the live
/// routes now, and those run in **declared source order** — LFO, envelope, key. Floating-point
/// addition is not associative, so a patch with two or more cutoff routes lands on a different last
/// bit; a patch with one is unchanged, which is why twenty-three digests held.
///
/// The size of it is what makes re-pinning the right answer rather than contorting the sum order to
/// match a legacy expression: it is a rounding step on a number of octaves, and every one of the
/// fifty peak levels is identical to four decimal places either way.
#[test]
fn reordering_the_cutoff_terms_moves_only_the_last_bits() {
    // Three plausible depths at three plausible source values, in octaves.
    let key = 0.5f32 * 0.2 * 5.0;
    let env = 0.85f32 * 0.73 * 6.0;
    let lfo = 0.3f32 * -0.41 * 4.0;

    let legacy = key + env + lfo;
    let routed = lfo + env + key;

    let gap = (legacy - routed).abs();
    assert!(
        gap <= f32::EPSILON * legacy.abs().max(1.0) * 2.0,
        "the two orders differ by {gap:e} octaves, which is more than rounding"
    );
    // And in the only unit that matters: the cutoff frequency it lands on.
    let cents = 1_200.0 * (legacy - routed).abs();
    assert!(cents < 0.001, "{cents} cents apart");
}
