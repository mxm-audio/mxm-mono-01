//! Does each mxm-mono-01 control do what its label says?
//!
//! Measured through the real player, on rendered audio — not by reading the DSP and agreeing with
//! it. Every assertion here is a property of the sound: a frequency, an amplitude, a tail length.
//!
//! This is the check the plan called "the recurring manual one": does it click on note-on, does the
//! filter close, does the envelope have the shape the labels promise.

use mxm_player_harness::app_harness;

use mxm_player::events::input::Payload;
use mxm_player::session::{FRAMES_PER_BLOCK, Session};
use std::path::PathBuf;

const PLUGIN: &str = "dk.mxm.mxm-mono-01";
const SAMPLE_RATE: f64 = 48_000.0;

fn bundle() -> Option<(PathBuf, PathBuf)> {
    let dir = app_harness::bundled_dir()?;
    let file = dir.join("mxm-mono-01.clap");
    Some((dir, file))
}

fn session(name: &str) -> Option<Session> {
    let (dir, file) = bundle()?;
    let mut s = Session::scratch(name, vec![dir]);
    s.load(&file, PLUGIN);
    Some(s)
}

// --- measurement --------------------------------------------------------------------------------

/// The left channel of an interleaved stereo render.
use mxm_measure::channels::left;

/// Peak magnitude of a capture.
///
/// **A shim over `mxm-measure`, and the `expect` is the point.** The shared ruler reports absence for
/// a **non-finite** buffer rather than the largest number in it, because `f32::max` would otherwise
/// let a render that is half NaN measure as perfectly healthy — and then pass every "is it quiet?"
/// assertion below. Panicking here is the loud failure that behaviour deserves.
fn peak(samples: &[f32]) -> f32 {
    mxm_measure::level::peak(samples).expect("the capture is finite")
}

/// RMS via `mxm-measure`, narrowed to `f32` for these call sites.
///
/// **Absence panics rather than reading as zero.** The shared ruler declines for an empty buffer and
/// for a **non-finite** one; turning the second into `0.0` would let a broken render pass a silence
/// assertion, which is exactly what the result-form contract is for.
fn rms(samples: &[f32]) -> f32 {
    mxm_measure::level::rms(samples).expect("the capture is non-empty and finite") as f32
}

/// Fundamental frequency, from rising zero crossings.
///
/// Good enough for a filtered saw, which crosses zero once per cycle in each direction. It would
/// be wrong for noise, so the noise level is kept at zero wherever this is used.
fn fundamental_hz(samples: &[f32]) -> f64 {
    let crossings = samples
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    crossings as f64 * SAMPLE_RATE / samples.len() as f64
}

/// A crude high-frequency measure: mean absolute sample-to-sample difference.
///
/// Rises with brightness, falls as a lowpass closes. Relative comparisons only.
fn brightness(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let total: f32 = samples.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
    total / (samples.len() - 1) as f32
}

// --- driving ------------------------------------------------------------------------------------

/// Sets a parameter by name, in its declared (normalised) range, and lets it settle.
///
/// nice-plug reports every parameter on 0..1 whatever its own units, so `fraction` is a position
/// in the range rather than a value in Hz or seconds. What it *means* is read back from the
/// plugin's own formatting.
fn set_param(session: &mut Session, name: &str, fraction: f64) -> String {
    let param = session
        .state()
        .param(name)
        .unwrap_or_else(|| panic!("`{name}` is not a parameter"))
        .clone();
    let value = param.min + fraction * (param.max - param.min);

    session
        .app()
        .engine_mut()
        .push_gui_event(Payload::ParamValue {
            param_id: param.id,
            value,
        });
    session.advance_blocks(4).expect("the session advances");

    session
        .state()
        .param(name)
        .map(|p| p.text.clone())
        .unwrap_or_default()
}

/// Plays a note, renders, and returns the left channel of the sustained portion.
fn sustained(session: &mut Session, note: u8, blocks: u64) -> Vec<f32> {
    session.clear_capture();
    session.app().note_on(note, 100.0 / 127.0);
    session.advance_blocks(blocks).expect("advances");
    let audio = left(&session.captured());

    session.app().note_off(note);
    session.advance_blocks(40).expect("advances");

    // Skip the attack so measurements see the steady state.
    let skip = (FRAMES_PER_BLOCK * 3).min(audio.len());
    audio[skip..].to_vec()
}

// --- the tests ----------------------------------------------------------------------------------

#[test]
fn at_rest_it_is_exactly_silent() {
    let Some(mut s) = session("rest") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };
    s.advance_blocks(20).expect("advances");
    assert_eq!(
        peak(&s.captured()),
        0.0,
        "an idle synth must render exact zeros, not merely something quiet"
    );
}

#[test]
fn a_note_sounds_at_the_pitch_it_was_asked_for() {
    let Some(mut s) = session("pitch") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    // MIDI 60 is C4, 261.63 Hz, at the default 8' footage.
    let audio = sustained(&mut s, 60, 12);
    let hz = fundamental_hz(&audio);

    assert!(
        (hz - 261.63).abs() < 6.0,
        "MIDI 60 should sound at about 261.6 Hz, measured {hz:.1} Hz"
    );
}

#[test]
fn the_octave_footage_switch_moves_the_pitch_by_octaves() {
    let Some(mut s) = session("footage") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    // Range is a four-way switch: 16', 8', 4', 2'. Position 0 is 16'.
    let mut pitches = Vec::new();
    for (index, fraction) in [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0].into_iter().enumerate() {
        let label = set_param(&mut s, "Range", fraction);
        let hz = fundamental_hz(&sustained(&mut s, 60, 12));
        pitches.push((index, label, hz));
    }

    for (index, label, hz) in &pitches {
        println!("  range {index} ({label}): {hz:.1} Hz");
    }

    // Each step up is an octave: consecutive ratios of two.
    for pair in pitches.windows(2) {
        let ratio = pair[1].2 / pair[0].2;
        assert!(
            (ratio - 2.0).abs() < 0.1,
            "each footage step must double the pitch: {} -> {} is {ratio:.2}x",
            pair[0].1,
            pair[1].1
        );
    }
}

#[test]
fn closing_the_filter_makes_the_sound_darker() {
    let Some(mut s) = session("cutoff") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    // Take the envelope out of the filter so cutoff is the only thing moving. **A route now**,
    // and its centre is no modulation, so 0.5 of a signed range is the same "off" the old
    // `Filter envelope` knob meant at 0.5 — the depth moved, its meaning did not.
    set_param(&mut s, "Cutoff from Envelope", 0.5);

    let open_label = set_param(&mut s, "Cutoff", 1.0);
    let open = brightness(&sustained(&mut s, 60, 12));

    let closed_label = set_param(&mut s, "Cutoff", 0.25);
    let closed = brightness(&sustained(&mut s, 60, 12));

    println!("  open ({open_label}): {open:.5}   closed ({closed_label}): {closed:.5}");
    assert!(
        closed < open * 0.5,
        "closing the cutoff must remove high-frequency content: {open:.5} -> {closed:.5}"
    );
}

#[test]
fn the_mixer_levels_each_contribute_and_silence_together() {
    let Some(mut s) = session("mixer") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    // Everything down: the oscillator section contributes nothing.
    for source in ["Saw", "Pulse", "Sub", "Noise"] {
        set_param(&mut s, source, 0.0);
    }
    let muted = peak(&sustained(&mut s, 60, 10));
    assert!(
        muted < 1e-6,
        "with every mixer level at zero there is nothing to hear, got {muted:e}"
    );

    // Each source on its own makes sound.
    for source in ["Saw", "Pulse", "Sub", "Noise"] {
        set_param(&mut s, source, 0.9);
        let alone = peak(&sustained(&mut s, 60, 10));
        assert!(
            alone > 1e-3,
            "`{source}` alone should be audible, got {alone:e}"
        );
        set_param(&mut s, source, 0.0);
    }
}

#[test]
fn the_output_level_scales_the_sound() {
    let Some(mut s) = session("output") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    let loud_label = set_param(&mut s, "Output", 1.0);
    let loud = rms(&sustained(&mut s, 60, 10));

    let quiet_label = set_param(&mut s, "Output", 0.35);
    let quiet = rms(&sustained(&mut s, 60, 10));

    println!("  {loud_label}: rms {loud:.5}   {quiet_label}: rms {quiet:.5}");
    assert!(
        quiet < loud * 0.6,
        "lowering the output must make it quieter: {loud:.5} -> {quiet:.5}"
    );
    assert!(quiet > 0.0, "and not silence it entirely");
}

#[test]
fn a_long_attack_takes_longer_to_reach_full_volume() {
    let Some(mut s) = session("attack") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    let measure_early_peak = |s: &mut Session| -> f32 {
        s.clear_capture();
        s.app().note_on(60, 1.0);
        // Two blocks ≈ 21 ms.
        s.advance_blocks(2).expect("advances");
        let early = peak(&left(&s.captured()));
        s.app().note_off(60);
        s.advance_blocks(40).expect("advances");
        early
    };

    let short_label = set_param(&mut s, "Attack", 0.0);
    let short = measure_early_peak(&mut s);

    let long_label = set_param(&mut s, "Attack", 0.6);
    let long = measure_early_peak(&mut s);

    println!("  attack {short_label}: {short:.4}   attack {long_label}: {long:.4}");
    assert!(
        long < short * 0.5,
        "a longer attack must still be rising when a short one has arrived: {short:.4} vs {long:.4}"
    );
}

#[test]
fn a_long_release_keeps_sounding_after_the_key_is_lifted() {
    let Some(mut s) = session("release") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    /// Milliseconds from the key lifting until the sound falls below an audible threshold.
    ///
    /// Measuring the *peak* of a window starting at note-off would be wrong: the loudest sample
    /// in it is the one right after the key lifts, which is the sustain level whatever the
    /// release is set to. The decay time is the thing the label actually promises.
    fn decay_ms(s: &mut Session) -> f64 {
        s.app().note_on(60, 1.0);
        s.advance_blocks(10).expect("advances");
        s.app().note_off(60);
        s.clear_capture();
        s.advance_blocks(200).expect("advances");

        let audio = left(&s.captured());
        let quiet = 1e-3;
        let last_loud = audio.iter().rposition(|v| v.abs() > quiet).unwrap_or(0);
        last_loud as f64 / SAMPLE_RATE * 1000.0
    }

    let short_label = set_param(&mut s, "Release", 0.0);
    let short = decay_ms(&mut s);

    let long_label = set_param(&mut s, "Release", 0.55);
    let long = decay_ms(&mut s);

    println!(
        "  release {short_label}: decays in {short:.0} ms   release {long_label}: {long:.0} ms"
    );
    assert!(
        long > short * 3.0,
        "a longer release must take longer to fall silent: {short:.0} ms vs {long:.0} ms"
    );
    assert!(
        short < 60.0,
        "the shortest release should be nearly immediate, took {short:.0} ms"
    );
}

#[test]
fn pitch_bend_moves_the_pitch_and_returns_to_centre() {
    let Some(mut s) = session("bend") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    let centred = fundamental_hz(&sustained(&mut s, 60, 12));

    s.app().engine_mut().push_gui_event(Payload::PitchBend {
        channel: 0,
        value: 1.0,
    });
    let bent_up = fundamental_hz(&sustained(&mut s, 60, 12));

    s.app().engine_mut().push_gui_event(Payload::PitchBend {
        channel: 0,
        value: 0.0,
    });
    let bent_down = fundamental_hz(&sustained(&mut s, 60, 12));

    s.app().engine_mut().push_gui_event(Payload::PitchBend {
        channel: 0,
        value: 0.5,
    });
    let recentred = fundamental_hz(&sustained(&mut s, 60, 12));

    println!(
        "  centre {centred:.1} Hz, up {bent_up:.1} Hz, down {bent_down:.1} Hz, back {recentred:.1} Hz"
    );

    // The default bend range is 2 semitones: a whole tone is a ratio of about 1.122.
    assert!(
        bent_up > centred * 1.05,
        "bending up must raise the pitch: {centred:.1} -> {bent_up:.1}"
    );
    assert!(
        bent_down < centred * 0.95,
        "bending down must lower it: {centred:.1} -> {bent_down:.1}"
    );
    assert!(
        (recentred - centred).abs() < 6.0,
        "returning the wheel to centre must return the pitch: {centred:.1} vs {recentred:.1}"
    );
}

#[test]
fn all_sound_off_silences_immediately_and_all_notes_off_releases() {
    let Some(mut s) = session("panic-ccs") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };
    // A long release, so "immediate" and "released" are clearly different outcomes.
    set_param(&mut s, "Release", 0.7);

    // CC 120: all sound off. No release tail at all.
    s.app().note_on(60, 1.0);
    s.advance_blocks(10).expect("advances");
    s.app().engine_mut().push_gui_event(Payload::ControlChange {
        channel: 0,
        controller: 120,
        value: 0,
    });
    s.advance_blocks(2).expect("advances");
    s.clear_capture();
    s.advance_blocks(8).expect("advances");
    let after_sound_off = peak(&left(&s.captured()));

    // CC 123: all notes off. Releases normally, so a tail remains.
    s.app().note_on(64, 1.0);
    s.advance_blocks(10).expect("advances");
    s.app().engine_mut().push_gui_event(Payload::ControlChange {
        channel: 0,
        controller: 123,
        value: 0,
    });
    s.advance_blocks(2).expect("advances");
    s.clear_capture();
    s.advance_blocks(8).expect("advances");
    let after_notes_off = peak(&left(&s.captured()));

    println!("  after CC120: {after_sound_off:e}   after CC123: {after_notes_off:e}");
    assert!(
        after_sound_off < 1e-6,
        "CC 120 must cut the sound immediately, got {after_sound_off:e}"
    );
    assert!(
        after_notes_off > 1e-4,
        "CC 123 must release rather than cut, so a tail should remain, got {after_notes_off:e}"
    );
}

#[test]
fn it_is_monophonic_and_the_note_stack_returns_to_the_held_note() {
    let Some(mut s) = session("stack") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    // Hold a low note, add a higher one, then release the higher one: the low one comes back.
    s.app().note_on(60, 1.0);
    s.advance_blocks(8).expect("advances");

    s.app().note_on(72, 1.0);
    s.clear_capture();
    s.advance_blocks(8).expect("advances");
    let upper = fundamental_hz(&left(&s.captured()));

    s.app().note_off(72);
    s.clear_capture();
    s.advance_blocks(8).expect("advances");
    let back = fundamental_hz(&left(&s.captured()));

    s.app().note_off(60);
    s.advance_blocks(40).expect("advances");

    println!("  upper {upper:.1} Hz, back to {back:.1} Hz");
    assert!(
        (upper - 523.25).abs() < 12.0,
        "the newer note should take over: expected about 523 Hz, got {upper:.1}"
    );
    assert!(
        (back - 261.63).abs() < 8.0,
        "releasing it must fall back to the note still held: expected about 262 Hz, got {back:.1}"
    );
}

#[test]
fn the_filter_self_oscillation_guard_holds_at_extreme_settings() {
    // The DSP's stated numeric contract: bounded output however the filter is driven.
    let Some(mut s) = session("stability") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    set_param(&mut s, "Resonance", 1.0);
    set_param(&mut s, "Cutoff", 1.0);
    set_param(&mut s, "Saw", 1.0);
    set_param(&mut s, "Noise", 1.0);
    set_param(&mut s, "Output", 1.0);

    let audio = sustained(&mut s, 60, 40);

    let worst = peak(&audio);
    println!("  peak at maximum resonance and level: {worst:.3}");
    assert!(
        audio.iter().all(|s| s.is_finite()),
        "the filter must never produce NaN or infinity"
    );
    assert!(
        worst < 8.0,
        "the stated bound is |y| < 8.0, measured {worst:.3}"
    );
}

/// Renders audible demonstrations of the features the tests above measure.
///
/// Not an assertion — the tests are the assertions. This exists so a person can *hear* what the
/// numbers describe, which no amount of passing tests substitutes for on a musical instrument.
///
/// `cargo test -p mxm-mono-01-host-tests --test behaviour -- --ignored render_audio_demo --nocapture`
#[test]
#[ignore = "renders WAV files for listening rather than asserting; run explicitly"]
fn render_audio_demo() {
    let Some((dir, file)) = bundle() else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    let out = app_harness::workspace_root()
        .join("target")
        .join("mxm-mono-01-demo");
    let _ = std::fs::create_dir_all(&out);

    let render = |name: &str, body: &dyn Fn(&mut Session)| {
        let mut s = Session::scratch(&format!("demo-{name}"), vec![dir.clone()]);
        s.load(&file, PLUGIN);
        s.advance_blocks(4).expect("advances");
        s.clear_capture();
        body(&mut s);

        let samples = s.captured();
        let path = out.join(format!("{name}.wav"));
        // 16-bit PCM rather than 32-bit float: these are for listening, and every player opens
        // 16-bit. The measured tests work on the float samples, so nothing is lost.
        let scaled: Vec<f32> = samples.iter().map(|sample| sample * 0.9).collect();
        mxm_audio_file::write(
            &path,
            &scaled,
            2,
            SAMPLE_RATE as u32,
            mxm_audio_file::Target::Wav(mxm_audio_file::Bits::Sixteen),
        )
        .expect("wav");
        println!(
            "  {name}.wav  {:.1}s  peak {:.3}",
            samples.len() as f64 / 2.0 / SAMPLE_RATE,
            peak(&samples)
        );
    };

    // 1. The default patch, played as a phrase — what it sounds like out of the box.
    render("1-default-riff", &|s| {
        for note in [48u8, 48, 55, 48, 51, 48, 46, 48] {
            s.app().note_on(note, 100.0 / 127.0);
            s.advance_blocks(26).expect("advances");
            s.app().note_off(note);
            s.advance_blocks(12).expect("advances");
        }
        s.advance_blocks(40).expect("advances");
    });

    // 2. A filter sweep under a held note, with resonance up — the classic test of a ladder.
    render("2-filter-sweep", &|s| {
        set_param(s, "Resonance", 0.75);
        set_param(s, "Cutoff from Envelope", 0.5);
        set_param(s, "Cutoff", 0.05);
        s.clear_capture();

        s.app().note_on(45, 1.0);
        for step in 0..=60 {
            set_param(s, "Cutoff", 0.05 + 0.9 * f64::from(step) / 60.0);
            s.advance_blocks(4).expect("advances");
        }
        s.app().note_off(45);
        s.advance_blocks(40).expect("advances");
    });

    // 3. The four footages on one note, so the octave switch is audible.
    render("3-octaves", &|s| {
        for fraction in [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0] {
            set_param(s, "Range", fraction);
            s.app().note_on(60, 1.0);
            s.advance_blocks(28).expect("advances");
            s.app().note_off(60);
            s.advance_blocks(16).expect("advances");
        }
    });

    // 4. Legato and glide: the note stack and portamento, which a mouse cannot demonstrate.
    render("4-glide-legato", &|s| {
        set_param(s, "Glide", 0.45);
        s.clear_capture();

        s.app().note_on(48, 1.0);
        s.advance_blocks(30).expect("advances");
        s.app().note_on(60, 1.0); // legato: slides up, envelope does not retrigger
        s.advance_blocks(36).expect("advances");
        s.app().note_on(55, 1.0);
        s.advance_blocks(36).expect("advances");
        s.app().note_off(55);
        s.advance_blocks(30).expect("advances"); // falls back to 60, then 48
        s.app().note_off(60);
        s.advance_blocks(36).expect("advances");
        s.app().note_off(48);
        s.advance_blocks(50).expect("advances");
    });

    println!("  written to {}", out.display());
}
