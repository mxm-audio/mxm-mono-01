//! T4 — golden audio: a fixed score through the real application path.
//!
//! The other layers prove the *player* behaves. This one proves the **plugin still sounds the
//! same**, and it does so through `PlayerApp`, not through a harness that bypasses it — the
//! difference matters, because everything between the keyboard and the DSP is player code.
//!
//! The reference is a committed hash rather than a committed WAV: a 32-bit float render of a few
//! seconds is megabytes, and the thing being asserted is "did this change", for which a digest is
//! exactly as good and reviewable in a diff.
//!
//! # When this fails
//!
//! A failure is not automatically a bug — a deliberate DSP change should fail it. The fix is to
//! listen to the artifact it writes, decide whether the change was intended, and update the
//! reference in the same commit as the change that caused it. A silent update is the one thing
//! that makes this test worthless.

use mxm_player_harness::app_harness;

use mxm_player::session::Session;
use std::path::PathBuf;

const PLUGIN: &str = "dk.mxm.mxm-mono-01";

/// The committed reference for [`score`], at mxm-mono-01's default parameters.
///
/// Update **only** together with the change that moved it, and record *why* here — not only in a
/// commit message, which is not where somebody reading this constant will look.
///
/// **Moved by the init-patch retune** (`plugins/AGENTS.md`, *Every instrument has an init patch*).
/// Seven defaults changed, and the audible ones are the envelope: attack 5 → 2 ms, decay 0.3 → 1.2 s,
/// sustain 0.7 → 0.2, release 0.2 → 0.25 s, plus the filter-envelope depth 0.4 → 0 — `filterenv` then, the (cutoff, envelope) route now — and the cutoff opening to
/// 16 kHz. The render was listened to before this line was changed: peak 0.412, RMS 0.120, a fast
/// attack falling to a low sustain and then a release tail — a plucked shape where the previous
/// reference was flatter and more sustained. That is the change the retune was for.
///
/// **Not moved by the diode clamp** (2026-09-03, `crates/mxm-mono-01-dsp/src/filter.rs`): the
/// resonance feedback saturator changed from `tanh` to a diode clamp, and this digest stayed
/// bit-identical — because the score plays the init patch, whose resonance is zero, so the
/// feedback path contributes nothing to the render. Worth knowing before trusting a green run
/// here as evidence about the filter's resonance behaviour: it is not.
const GOLDEN_DIGEST: &str = "b0791ab43fe7c079";

/// How many samples the score renders. Guards against a change that alters length rather than
/// content — a shorter render with a matching prefix would otherwise slip through.
const GOLDEN_SAMPLES: usize = 45 * mxm_player::session::FRAMES_PER_BLOCK * 2;

fn bundle() -> Option<(PathBuf, PathBuf)> {
    let dir = app_harness::bundled_dir()?;
    let file = dir.join("mxm-mono-01.clap");
    Some((dir, file))
}

/// The score: two notes, one released under sustain, one plain. Fixed forever.
///
/// It deliberately exercises the paths that are easy to break silently — the press stack, the
/// pedal, and the release tail — rather than a single note that would still pass if sustain
/// stopped working entirely.
fn score(session: &mut Session) -> Result<(), String> {
    session.advance_blocks(2)?;

    session.app().note_on(60, 100.0 / 127.0);
    session.advance_blocks(6)?;

    session.app().set_sustain(true);
    session.app().note_off(60);
    session.advance_blocks(6)?;

    session.app().note_on(67, 90.0 / 127.0);
    session.advance_blocks(6)?;

    session.app().set_sustain(false);
    session.advance_blocks(6)?;

    session.app().note_off(67);
    session.advance_blocks(19)?;
    Ok(())
}

/// A stable digest of the rendered samples, on their exact bit patterns.
fn digest(samples: &[f32]) -> String {
    // FNV-1a over the raw bits: no dependency, and exact — this is a bit-for-bit comparison
    // expressed compactly, not a perceptual one.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for sample in samples {
        for byte in sample.to_bits().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

fn render_score(name: &str) -> Option<(Vec<f32>, PathBuf)> {
    let (dir, file) = bundle()?;
    let mut session = Session::scratch(name, vec![dir]);
    session.load(&file, PLUGIN);
    score(&mut session).expect("the session advances");

    let samples = session.captured();
    let (wav, _json) = session
        .write_artifacts(name)
        .expect("the artifacts are written");
    Some((samples, wav))
}

#[test]
fn the_score_still_sounds_the_same() {
    let Some((samples, wav)) = render_score("golden") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    assert_eq!(
        samples.len(),
        GOLDEN_SAMPLES,
        "the score's length changed; the reference covers content, so length is checked separately"
    );
    assert!(
        samples.iter().any(|s| s.abs() > 1e-4),
        "the score rendered silence, which no reference should ever match"
    );

    let actual = digest(&samples);
    assert_eq!(
        actual,
        GOLDEN_DIGEST,
        "mxm-mono-01 renders differently through the player than the committed reference.\n\
         If the change was deliberate, listen to {} and update GOLDEN_DIGEST to {actual} in the \
         same commit as the change that caused it.",
        wav.display()
    );
}

#[test]
fn the_golden_test_would_catch_a_change_in_the_sound() {
    // A reference test that cannot fail is decoration. This proves the digest is sensitive to the
    // audio rather than to something incidental, by changing one parameter and asserting the
    // render moves away from the reference.
    let Some((dir, file)) = bundle() else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-01 --release`");
        return;
    };

    let mut session = Session::scratch("golden-sensitivity", vec![dir]);
    session.load(&file, PLUGIN);

    // Move the filter cutoff well away from its default, then play the same score.
    let cutoff = session
        .state()
        .param("Cutoff")
        .expect("Cutoff is a parameter")
        .clone();
    session
        .app()
        .engine_mut()
        .push_gui_event(mxm_player::events::input::Payload::ParamValue {
            param_id: cutoff.id,
            value: cutoff.min + 0.05 * (cutoff.max - cutoff.min),
        });

    score(&mut session).expect("the session advances");

    assert_ne!(
        digest(&session.captured()),
        GOLDEN_DIGEST,
        "closing the filter must change the render; if it does not, the digest is not measuring \
         the audio"
    );
}
