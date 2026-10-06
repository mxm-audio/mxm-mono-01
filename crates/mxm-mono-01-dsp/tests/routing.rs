//! What the routing does to mxm-mono-01's voice, and what it must not do.
//!
//! `plans/plan-modulation-routing.md` §10. The shared layer's own contracts are proved in
//! mxm-kit's `crates/mxm-modulation/tests/contracts.rs`; these are the instrument's.

use mxm_mono_01_dsp::routing::{FULL_SCALE, Graph, Routing, source, target};
use mxm_mono_01_dsp::voice::{NoteId, Patch, Retrigger, Voice};

const FS: f32 = 48_000.0;

fn voice() -> Voice {
    let mut v = Voice::new();
    v.set_sample_rate(FS);
    v.reset();
    v
}

fn note(v: &mut Voice, note: u8) {
    v.note_on(
        NoteId {
            voice_id: None,
            channel: 0,
            note,
        },
        Retrigger::Always,
    );
}

fn release(v: &mut Voice, note: u8) {
    v.note_off(None, 0, note, Retrigger::Always);
}

/// A patch with an audible sound and nothing routed.
fn patch() -> Patch {
    Patch {
        cutoff_hz: 2_000.0,
        sustain: 1.0,
        attack_s: 0.001,
        output_gain: 1.0,
        ..Patch::default()
    }
}

fn render(v: &mut Voice, p: &Patch, r: &Routing, n: usize) -> Vec<f32> {
    v.set_topology(r);
    (0..n).map(|_| v.process(p, r)).collect()
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

/// **Nothing routed changes nothing.** The init patch renders exactly as it did before routing
/// existed, which is the property the whole conversion is held to.
#[test]
fn an_unrouted_patch_is_bit_identical_to_one_with_no_routing_at_all() {
    let p = patch();
    let empty = Routing::new();
    let mut a = voice();
    note(&mut a, 48);
    let with_graph = render(&mut a, &p, &empty, 4_800);

    // Every route absent is the same as every amount zero, and both are the same as the machine's
    // own path: the sums contribute nothing and the arithmetic is untouched.
    let q = patch();
    let mut depths = Routing::new();
    for target_routes in depths.amounts.iter_mut() {
        for amount in target_routes.iter_mut() {
            *amount = 0.5;
        }
    }
    let mut b = voice();
    note(&mut b, 48);
    let with_amounts_but_absent = render(&mut b, &q, &depths, 4_800);

    assert_eq!(
        with_graph, with_amounts_but_absent,
        "an absent route must contribute nothing whatever its amount holds"
    );
    assert!(peak(&with_graph) > 0.0, "and the patch should sound at all");
}

/// A route sounds once it is present, and stops when it is removed — the owner's requirement, at
/// the instrument.
#[test]
fn a_route_sounds_when_present_and_stops_when_removed() {
    let mut p = patch();
    p.lfo_rate_hz = 8.0;
    let mut r = Routing::new();
    r.present[target::CUTOFF][source::LFO] = true;
    r.amounts[target::CUTOFF][source::LFO] = 1.0;

    let mut v = voice();
    note(&mut v, 48);
    let routed = render(&mut v, &p, &r, 24_000);

    let mut off = r;
    off.present[target::CUTOFF][source::LFO] = false;
    let mut w = voice();
    note(&mut w, 48);
    let unrouted = render(&mut w, &p, &off, 24_000);

    assert_ne!(
        routed, unrouted,
        "a present route into the cutoff must change the sound"
    );

    // And the amount is untouched by the removal, so putting it back restores exactly the same
    // sound rather than an approximation of it.
    let mut back = off;
    back.present[target::CUTOFF][source::LFO] = true;
    let mut x = voice();
    note(&mut x, 48);
    assert_eq!(
        render(&mut x, &p, &back, 24_000),
        routed,
        "re-adding a removed source restores the depth it had"
    );
}

/// **The amplitude target scales the envelope rather than adding to it**, so a decayed voice is
/// exact silence whatever is routed. This is the circular-lifetime finding: adding would leave a
/// latched source holding the voice audible after its envelope ended.
#[test]
fn a_decayed_voice_is_exactly_silent_however_much_reaches_its_amplifier() {
    let mut p = patch();
    p.attack_s = 0.001;
    p.decay_s = 0.01;
    p.sustain = 0.0;
    p.release_s = 0.01;
    // Key is latched: it keeps its value through and after the release, which is what makes this
    // the dangerous case.
    let mut r = Routing::new();
    r.present[target::AMPLITUDE][source::KEY] = true;
    r.amounts[target::AMPLITUDE][source::KEY] = 1.0;

    let mut v = voice();
    note(&mut v, 96); // well above middle C, so the key source is large and positive
    v.set_topology(&r);
    for _ in 0..2_400 {
        v.process(&p, &r);
    }
    release(&mut v, 96);

    let tail: Vec<f32> = (0..FS as usize).map(|_| v.process(&p, &r)).collect();
    let last = &tail[tail.len() - 4_800..];
    assert_eq!(
        peak(last),
        0.0,
        "a latched source routed to amplitude must not hold the voice open"
    );
}

/// Audio-rate sources are what make FM reachable, and they are the instrument's own oscillator
/// rather than a new generator.
#[test]
fn an_audio_rate_source_reaches_pitch_and_changes_the_sound() {
    let mut p = patch();
    p.levels.saw = 0.8;
    let none = Routing::new();
    let mut plain = voice();
    note(&mut plain, 48);
    let unmodulated = render(&mut plain, &p, &none, 12_000);

    let mut r = Routing::new();
    r.present[target::PITCH][source::SAW] = true;
    r.amounts[target::PITCH][source::SAW] = 1.0;
    let mut fm = voice();
    note(&mut fm, 48);
    let modulated = render(&mut fm, &p, &r, 12_000);

    assert_ne!(
        unmodulated, modulated,
        "the saw should be able to modulate pitch"
    );
    assert!(
        modulated.iter().all(|s| s.is_finite()),
        "and audio-rate feedback must stay finite"
    );
}

/// Every source into every target, at extreme depths: nothing produces a non-finite sample and
/// nothing runs away. Any-to-any reaches pairs no designer chose, so this is the sweep that matters.
#[test]
fn no_source_target_pair_produces_a_non_finite_or_runaway_sample() {
    for t in 0..mxm_mono_01_dsp::routing::TARGETS {
        for src in 0..mxm_mono_01_dsp::routing::SOURCES {
            for &depth in &[-1.0f32, 1.0] {
                let mut p = patch();
                p.levels.saw = 0.7;
                p.levels.noise = 0.3;
                p.resonance = 0.9;
                let mut r = Routing::new();
                r.present[t][src] = true;
                r.amounts[t][src] = depth;

                let mut v = voice();
                note(&mut v, 60);
                let out = render(&mut v, &p, &r, 4_800);
                assert!(
                    out.iter().all(|s| s.is_finite()),
                    "target {t} from source {src} at {depth} produced a non-finite sample"
                );
                assert!(
                    peak(&out) < 100.0,
                    "target {t} from source {src} at {depth} ran away: {}",
                    peak(&out)
                );
            }
        }
    }
}

/// Silence in, exact zero out, with everything routed: the denormal-flush contract survives.
#[test]
fn a_fully_routed_voice_still_reaches_exact_silence() {
    let mut p = patch();
    p.decay_s = 0.01;
    p.sustain = 0.0;
    p.release_s = 0.01;
    let mut r = Routing::new();
    for t in 0..mxm_mono_01_dsp::routing::TARGETS {
        for src in 0..mxm_mono_01_dsp::routing::SOURCES {
            r.present[t][src] = true;
            r.amounts[t][src] = 0.5;
        }
    }

    let mut v = voice();
    note(&mut v, 60);
    v.set_topology(&r);
    for _ in 0..2_400 {
        v.process(&p, &r);
    }
    release(&mut v, 60);
    let tail: Vec<f32> = (0..(FS as usize * 2)).map(|_| v.process(&p, &r)).collect();
    assert_eq!(
        peak(&tail[tail.len() - 4_800..]),
        0.0,
        "everything routed must still decay to exact zero"
    );
}

/// `reset` leaves no tail in the frame, so one render cannot leak a sample into the next.
#[test]
fn reset_clears_the_routing_frame() {
    let mut p = patch();
    p.levels.saw = 0.9;
    let mut r = Routing::new();
    r.present[target::CUTOFF][source::SAW] = true;
    r.amounts[target::CUTOFF][source::SAW] = 1.0;

    let mut v = voice();
    note(&mut v, 60);
    let first = render(&mut v, &p, &r, 2_400);

    v.reset();
    note(&mut v, 60);
    let second = render(&mut v, &p, &r, 2_400);

    assert_eq!(
        first, second,
        "a reset voice renders identically to a fresh one"
    );
}

/// **A source that becomes needed starts from silence, not from an old phrase.**
///
/// A source nothing reads is not published, so its slot keeps whatever it held the last time
/// something did. Adding a backward route to it later would read that value for one sample — and
/// how old it is depends on how long the source went unread, which is the host's buffer sizes
/// deciding a sound. `Graph::set_topology` clears the slot as the source becomes needed.
///
/// Two gaps, because the voice has two ways of not publishing: another route keeps the frame
/// ticking while the saw's value is offered and dropped, or nothing is routed and the frame is not
/// opened at all.
///
/// **Falsified before trusted**: with the clear removed from `Graph::set_topology`, the first read
/// after re-adding is `6.3` in both gaps — the saw's `0.9` from before the gap, at the pitch route's
/// seven semitones — instead of `0.0`.
#[test]
fn a_source_that_becomes_needed_starts_from_silence_not_from_an_old_phrase() {
    let mut on = Routing::new();
    on.present[target::PITCH][source::SAW] = true;
    on.amounts[target::PITCH][source::SAW] = 1.0;

    let mut ticking = Routing::new();
    ticking.present[target::PITCH][source::LFO] = true;
    ticking.amounts[target::PITCH][source::LFO] = 1.0;

    for (name, gap) in [("ticking", ticking), ("unrouted", Routing::new())] {
        let mut g = Graph::new();
        g.set_topology(&on);
        g.begin_sample();
        g.write(source::SAW, 0.9);
        assert!(
            g.sum(target::PITCH, &on) > 0.0,
            "{name}: the old phrase must be readable before the gap, or this proves nothing"
        );

        g.set_topology(&gap);
        if g.any_live() {
            for _ in 0..FS as usize {
                g.begin_sample();
                g.write(source::LFO, 0.25);
                // The voice offers its audio every sample; nothing reads the saw, so it is dropped.
                g.write(source::SAW, -0.7);
            }
        }

        // Re-added. A route from the instrument's audio into pitch is backward — this sample's saw
        // is not published until after the pitch sum — so the sum reads the slot as it stands.
        g.set_topology(&on);
        g.begin_sample();
        assert_eq!(
            g.sum(target::PITCH, &on),
            0.0,
            "{name}: a newly read source must start from silence, not from before the gap"
        );

        // And publication resumes from there.
        g.write(source::SAW, 0.5);
        g.begin_sample();
        assert_eq!(
            g.sum(target::PITCH, &on),
            0.5 * FULL_SCALE[target::PITCH][source::SAW],
            "{name}: the next sample reads what was published"
        );
    }
}

/// **The declared evaluation order is what the voice runs**, not a table that agrees with itself.
///
/// `routing::source` declares the instrument's audio last, so a route from it into pitch reads
/// *last* sample and a route into cutoff reads *this* one. Noise carries the proof because it does
/// not depend on pitch: two fresh voices draw the same noise, so on the first sample a backward
/// route has nothing to read and the render must be bit-identical to an unrouted one, while a
/// forward route already reads this sample's noise and must not be.
///
/// **Falsified before trusted**: with the four audio `write`s removed from `Voice::process`, the
/// backward route renders identically to the unrouted voice from the second sample on; with them
/// moved below the cutoff sum, the forward route's first sample equals the unrouted one.
#[test]
fn a_route_from_the_instruments_audio_is_a_sample_late_into_pitch_and_on_time_into_cutoff() {
    let mut p = patch();
    p.levels.saw = 0.8;
    p.levels.noise = 0.5;
    let first = |r: &Routing| {
        let mut v = voice();
        note(&mut v, 60);
        render(&mut v, &p, r, 64)
    };

    let unrouted = first(&Routing::new());
    let mut backward = Routing::new();
    backward.present[target::PITCH][source::NOISE] = true;
    backward.amounts[target::PITCH][source::NOISE] = 1.0;
    let mut forward = Routing::new();
    forward.present[target::CUTOFF][source::NOISE] = true;
    forward.amounts[target::CUTOFF][source::NOISE] = 1.0;
    let late = first(&backward);
    let on_time = first(&forward);

    assert_ne!(
        unrouted[0], 0.0,
        "the first sample must sound, or neither comparison below means anything"
    );
    assert_eq!(
        late[0].to_bits(),
        unrouted[0].to_bits(),
        "a route from noise into pitch is backward: its first sample has nothing to read"
    );
    assert_ne!(
        late[1..],
        unrouted[1..],
        "and from the next sample on it reads what the noise published"
    );
    assert_ne!(
        on_time[0], unrouted[0],
        "a route from noise into cutoff is forward: it reads this sample's noise"
    );
}
