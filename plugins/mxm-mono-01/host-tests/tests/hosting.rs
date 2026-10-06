//! P0 — prove the hosting API, offline.
//!
//! The comparison is against a direct `mxm-mono-01-dsp` render driven by the same events, **as the
//! in-memory `f32` channels, bit-exact**. No format conversion is involved in a 32-bit envelope,
//! so there is no reason to weaken it. A WAV is written alongside purely as a diagnostic.
//!
//! Requires `cargo xtask bundle mxm-mono-01 --release` and `cargo xtask fixtures --release` to have
//! run; both tests skip with an explanatory message rather than failing if the artifact is
//! missing, because a missing build is not a hosting bug.

use mxm_mono_01_dsp::lfo::LfoShape;
use mxm_mono_01_dsp::oscillator::{MixLevels, SubShape};
use mxm_mono_01_dsp::voice::{NoteId, Patch, Retrigger, VcaSource, Voice};
use mxm_player::envelope::Dialect;
use mxm_player::offline::{EventKind, RenderConfig, ScheduledEvent, render};
use std::path::PathBuf;

const SAMPLE_RATE: f64 = 48_000.0;
const BLOCK: u32 = 512;
const TOTAL_FRAMES: u64 = 96_000;
const NOTE: u16 = 60;
/// CLAP velocity is normalised, not a MIDI integer. 100/127 is MIDI velocity 100.
const VELOCITY: f64 = 100.0 / 127.0;
/// Note-off at frame 0 of block 40.
const NOTE_OFF_FRAME: u64 = 40 * BLOCK as u64;
const VOICE_ID: u32 = 1;

fn bundle(name: &str, hint: &str) -> Option<PathBuf> {
    let path = mxm_player_harness::workspace_root().join(name);
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipping: {} is missing — run `{hint}`", path.display());
        None
    }
}

fn events() -> Vec<ScheduledEvent> {
    vec![
        ScheduledEvent {
            frame: 0,
            kind: EventKind::NoteOn {
                channel: 0,
                key: NOTE,
                velocity: VELOCITY,
                note_id: VOICE_ID,
            },
        },
        ScheduledEvent {
            frame: NOTE_OFF_FRAME,
            kind: EventKind::NoteOff {
                channel: 0,
                key: NOTE,
                velocity: 0.0,
                note_id: VOICE_ID,
            },
        },
    ]
}

/// The plugin's default patch, reproduced from `plugins/mxm-mono-01/src/params.rs`.
///
/// Every smoother is at its default target with nothing in flight, so the patch is constant for
/// the whole render — which is what makes a bit-exact comparison meaningful in the first place.
///
/// **Hand-copied, and it has to be.** The DSP crate is framework-free and knows nothing about
/// `MxmMono01Params`, so there is no way to read the defaults from the plugin without dragging
/// nice-plug into this comparison and losing the independence that gives it its value. The cost is
/// that a defaults change breaks this test, loudly, at frame 1 — which is the right failure: it is
/// the only thing in the suite that notices the plugin and the DSP have drifted apart.
/// It last moved with the init-patch retune (`plugins/AGENTS.md`, *Every instrument has an init
/// patch*).
fn default_patch() -> Patch {
    Patch {
        range_offset: 0.0, // OscRange::Eight
        tune_cents: 0.0,
        glide_time_s: 0.0,
        bend_semitones: 0.0,
        bend: 0.0,
        expression_semitones: 0.0,
        // The routable performance sources, at rest: this test is the machine's own default patch,
        // and nothing is routed in it.
        velocity: 0.0,
        wheel: 0.0,
        pressure: 0.0,

        levels: MixLevels {
            saw: 0.8,
            pulse: 0.0,
            sub: 0.0,
            noise: 0.0,
        },
        pulse_width: 0.5,
        sub_shape: SubShape::Oct1Square,

        cutoff_hz: 16_000.0,
        resonance: 0.0,

        attack_s: 0.002,
        decay_s: 1.2,
        sustain: 0.2,
        release_s: 0.25,

        lfo_rate_hz: 5.5,
        lfo_shape: LfoShape::Triangle,

        vca_source: VcaSource::Envelope,
        output_gain: 1.0, // db_to_gain(0.0)
    }
}

/// Renders the same events straight through the DSP crate, with no host involved.
fn reference_render() -> Vec<f32> {
    let patch = default_patch();
    // Nothing routed: this is the instrument's own default patch, which is the point of the test.
    let routing = mxm_mono_01_dsp::routing::Routing::new();
    let mut voice = Voice::new();
    voice.set_sample_rate(SAMPLE_RATE as f32);
    voice.reset();

    let mut out = Vec::with_capacity(TOTAL_FRAMES as usize);
    for frame in 0..TOTAL_FRAMES {
        if frame == 0 {
            voice.note_on(
                NoteId {
                    voice_id: Some(VOICE_ID as i32),
                    channel: 0,
                    note: NOTE as u8,
                },
                Retrigger::Legato,
            );
        }
        if frame == NOTE_OFF_FRAME {
            voice.note_off(Some(VOICE_ID as i32), 0, NOTE as u8, Retrigger::Legato);
        }
        out.push(voice.process(&patch, &routing));
    }
    out
}

#[test]
fn mxm_mono_01_renders_bit_exactly_through_the_host() {
    let Some(path) = bundle(
        "target/bundled/mxm-mono-01.clap",
        "cargo xtask bundle mxm-mono-01 --release",
    ) else {
        return;
    };

    let result = render(
        &path,
        "dk.mxm.mxm-mono-01",
        RenderConfig {
            state: None,
            sample_rate: SAMPLE_RATE,
            block_size: BLOCK,
            total_frames: TOTAL_FRAMES,
        },
        &events(),
    )
    .expect("mxm-mono-01 should host cleanly");

    // The selection policy itself is asserted, not assumed: mxm-mono-01 lists stereo first, and the
    // player takes the first compatible configuration.
    assert_eq!(
        result.envelope.audio.channel_count, 2,
        "the first compatible configuration is the stereo one ({})",
        result.envelope.selection
    );
    assert_eq!(
        result.envelope.note_input.map(|p| p.dialect),
        Some(Dialect::Clap),
        "mxm-mono-01 advertises the CLAP note dialect, which is what carries voice IDs"
    );
    assert_eq!(result.channels.len(), 2);

    // mxm-mono-01's stereo is dual mono.
    assert_eq!(
        result.channels[0], result.channels[1],
        "the two channels must be identical, sample for sample"
    );

    let reference = reference_render();
    assert_eq!(result.channels[0].len(), reference.len());

    write_diagnostic_wav(&result.channels[0]);

    if let Some((frame, (hosted, direct))) = result.channels[0]
        .iter()
        .zip(reference.iter())
        .enumerate()
        .find(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| (i, (*a, *b)))
    {
        panic!(
            "hosted render diverges from the direct DSP render at frame {frame}: \
             hosted {hosted:?} vs direct {direct:?}"
        );
    }

    // A synth that renders pure silence would pass the comparison for the wrong reason.
    assert!(
        result.channels[0].iter().any(|s| s.abs() > 1e-4),
        "the render should contain audible signal"
    );
}

#[test]
fn repeated_load_and_teardown_cycles_stay_clean() {
    let Some(path) = bundle(
        "target/bundled/mxm-mono-01.clap",
        "cargo xtask bundle mxm-mono-01 --release",
    ) else {
        return;
    };

    // Short renders, repeated: this is the deadlock-and-leak check, not an audio check.
    for _ in 0..8 {
        let result = render(
            &path,
            "dk.mxm.mxm-mono-01",
            RenderConfig {
                state: None,
                sample_rate: SAMPLE_RATE,
                block_size: BLOCK,
                total_frames: BLOCK as u64 * 4,
            },
            &events(),
        )
        .expect("every cycle should instantiate, activate, process and deactivate cleanly");
        assert_eq!(result.channels.len(), 2);
    }
}

/// A diagnostic artifact only: the assertion above compares the `f32` channels directly.
fn write_diagnostic_wav(samples: &[f32]) {
    let path = mxm_player_harness::workspace_root()
        .join("target")
        .join("p0-mxm-mono-01.wav");
    let _ = mxm_audio_file::write(
        &path,
        samples,
        1,
        SAMPLE_RATE as u32,
        mxm_audio_file::Target::WavFloat32,
    );
}
