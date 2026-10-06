//! Render a short demo of the mxm-mono-01 voice to a WAV file.
//!
//! This is the check a unit test cannot make: whether the thing actually sounds
//! like a synthesizer. Tests catch NaN, instability and silence; they say nothing
//! about character.
//!
//! Run with: `cargo run -p mxm-mono-01-dsp --release --example mono_01_render_demo`

use mxm_mono_01_dsp::lfo::LfoShape;
use mxm_mono_01_dsp::oscillator::{MixLevels, SubShape};
use mxm_mono_01_dsp::routing::{Routing, source, target};
use mxm_mono_01_dsp::voice::{NoteId, Patch, Retrigger, Voice};

const FS: f32 = 48_000.0;

/// One step of a little sequence: MIDI note, length in beats, and whether it
/// overlaps the next note (which is what makes legato slides happen).
struct Step {
    note: u8,
    beats: f32,
    slide: bool,
}

fn step(note: u8, beats: f32) -> Step {
    Step {
        note,
        beats,
        slide: false,
    }
}

fn slide(note: u8, beats: f32) -> Step {
    Step {
        note,
        beats,
        slide: true,
    }
}

struct Section {
    name: &'static str,
    patch: Patch,
    /// The section's modulation, which is where its filter envelope, PWM and filter LFO live now.
    ///
    /// They used to be `Patch` fields. They are the machine's own routes, present in the init patch
    /// at zero depth, so a demo that wants them turns them **up** rather than switching them on.
    routing: Routing,
    retrigger: Retrigger,
    bpm: f32,
    steps: Vec<Step>,
}

/// The machine's own wiring with some of it turned up: `(target, source, depth)`.
fn wired(depths: &[(usize, usize, f32)]) -> Routing {
    let mut routing = Routing::init();
    for &(t, s, depth) in depths {
        routing.present[t][s] = true;
        routing.amounts[t][s] = depth;
    }
    routing
}

fn render(section: &Section, out: &mut Vec<f32>) {
    let mut voice = Voice::new();
    voice.set_sample_rate(FS);
    let routing = &section.routing;
    voice.set_topology(routing);

    let beat_samples = (60.0 / section.bpm * FS) as usize;
    let mut held: Option<u8> = None;

    for (i, s) in section.steps.iter().enumerate() {
        let len = (beat_samples as f32 * s.beats) as usize;
        // A slide keeps the previous note held past the new note-on, which is what
        // triggers legato behaviour in the voice.
        let gate_len = if s.slide { len } else { (len * 4) / 5 };

        let id = NoteId {
            voice_id: Some(i as i32),
            channel: 0,
            note: s.note,
        };
        voice.note_on(id, section.retrigger);

        if let Some(prev) = held.take() {
            // Release the overlapping previous note *after* the new one started.
            voice.note_off(Some(i as i32 - 1), 0, prev, section.retrigger);
        }

        for n in 0..len {
            if n == gate_len && !s.slide {
                voice.note_off(Some(i as i32), 0, s.note, section.retrigger);
            }
            out.push(voice.process(&section.patch, routing));
        }

        if s.slide {
            held = Some(s.note);
        } else {
            voice.note_off(Some(i as i32), 0, s.note, section.retrigger);
        }
    }

    // Let the tail ring out.
    for _ in 0..(FS as usize) {
        out.push(voice.process(&section.patch, routing));
    }
}

/// Writes the demo through `mxm-measure`'s encoder.
///
/// **The seventh hand-written WAV writer, and the one the first census missed** — it lived here
/// rather than in `examples/common/`, so a search for the shared module did not find it. Unlike the
/// six, it applied no headroom at all and clamped to full scale, so nothing moves to the call site;
/// what changes is that `as i16` truncation becomes rounding, one LSB, away from zero.
fn write_wav(path: &str, samples: &[f32]) -> std::io::Result<()> {
    mxm_audio_file::write(
        path,
        samples,
        1,
        FS as u32,
        mxm_audio_file::Target::Wav(mxm_audio_file::Bits::Sixteen),
    )
    .map(|_| ())
    .map_err(std::io::Error::other)
}

fn main() -> std::io::Result<()> {
    let sections = vec![
        // 1. The classic: saw through a filter envelope. Plucky, and the sound the
        //    instrument is best known for.
        Section {
            name: "saw bass",
            retrigger: Retrigger::Always,
            bpm: 120.0,
            steps: vec![
                step(36, 0.5),
                step(36, 0.5),
                step(43, 0.5),
                step(36, 0.5),
                step(41, 0.5),
                step(36, 0.5),
                step(39, 0.5),
                step(34, 0.5),
            ],
            patch: Patch {
                levels: MixLevels {
                    saw: 0.9,
                    pulse: 0.0,
                    sub: 0.0,
                    noise: 0.0,
                },
                cutoff_hz: 240.0,
                resonance: 0.55,
                attack_s: 0.002,
                decay_s: 0.18,
                sustain: 0.0,
                release_s: 0.08,
                ..Patch::default()
            },
            routing: wired(&[(target::CUTOFF, source::ENVELOPE, 0.55)]),
        },
        // 2. Sub plus pulse: the weight the sub-oscillator adds is the whole reason
        //    it exists.
        Section {
            name: "sub and pulse",
            retrigger: Retrigger::Always,
            bpm: 120.0,
            steps: vec![
                step(31, 1.0),
                step(31, 0.5),
                step(38, 0.5),
                step(36, 1.0),
                step(29, 1.0),
            ],
            patch: Patch {
                levels: MixLevels {
                    saw: 0.0,
                    pulse: 0.55,
                    sub: 0.9,
                    noise: 0.0,
                },
                sub_shape: SubShape::Oct1Square,
                pulse_width: 0.35,
                cutoff_hz: 400.0,
                resonance: 0.3,
                attack_s: 0.003,
                decay_s: 0.3,
                sustain: 0.25,
                release_s: 0.12,
                ..Patch::default()
            },
            routing: wired(&[(target::CUTOFF, source::ENVELOPE, 0.45)]),
        },
        // 3. Legato slides with glide: this is what the note stack's fallback rule and
        //    the free-running phasors exist to make possible.
        Section {
            name: "legato slides",
            retrigger: Retrigger::Legato,
            bpm: 110.0,
            steps: vec![
                step(36, 0.5),
                slide(48, 0.5),
                step(36, 0.5),
                slide(43, 0.5),
                step(34, 0.5),
                slide(46, 1.0),
            ],
            patch: Patch {
                levels: MixLevels {
                    saw: 0.8,
                    pulse: 0.0,
                    sub: 0.5,
                    noise: 0.0,
                },
                glide_time_s: 0.09,
                cutoff_hz: 300.0,
                resonance: 0.62,
                attack_s: 0.004,
                decay_s: 0.35,
                sustain: 0.3,
                release_s: 0.2,
                ..Patch::default()
            },
            routing: wired(&[(target::CUTOFF, source::ENVELOPE, 0.5)]),
        },
        // 4. High resonance with PWM: shows the filter near self-oscillation and the
        //    LFO doing something audible.
        Section {
            name: "resonant sweep",
            retrigger: Retrigger::Always,
            bpm: 100.0,
            steps: vec![step(40, 2.0), step(45, 2.0)],
            patch: Patch {
                levels: MixLevels {
                    saw: 0.5,
                    pulse: 0.5,
                    sub: 0.3,
                    noise: 0.05,
                },
                lfo_rate_hz: 0.8,
                lfo_shape: LfoShape::Triangle,
                cutoff_hz: 180.0,
                resonance: 0.85,
                attack_s: 0.15,
                decay_s: 1.2,
                sustain: 0.35,
                release_s: 0.5,
                ..Patch::default()
            },
            routing: wired(&[
                (target::PULSE_WIDTH, source::LFO, 0.8),
                (target::CUTOFF, source::ENVELOPE, 0.75),
                (target::CUTOFF, source::LFO, 0.15),
            ]),
        },
    ];

    let mut samples = Vec::new();
    for section in &sections {
        println!("rendering: {}", section.name);
        render(section, &mut samples);
        // A breath between sections.
        samples.resize(samples.len() + FS as usize / 2, 0.0);
    }

    let peak = samples.iter().fold(0.0f32, |a, s| a.max(s.abs()));
    println!(
        "\n{} samples, {:.1} s, peak {:.3} ({:.1} dBFS)",
        samples.len(),
        samples.len() as f32 / FS,
        peak,
        20.0 * peak.max(1e-9).log10()
    );

    // Normalise to a comfortable level rather than clipping or being inaudible.
    if peak > 0.0 {
        let target = 0.89f32;
        for s in samples.iter_mut() {
            *s *= target / peak;
        }
    }

    let path = "target/mxm-mono-01-demo.wav";
    write_wav(path, &samples)?;
    println!("wrote {path}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This demo's write path, which is **not** like the other seven.
    ///
    /// `write_wav` applies no headroom law at all — it hands the samples straight to the encoder,
    /// which clamps at full scale. That difference is the thing worth catching if it ever changes
    /// silently, so the assertion is the opposite of the others': a loud source must arrive **at**
    /// full scale rather than just under it.
    #[test]
    fn the_demo_write_path_clamps_rather_than_leaving_headroom() {
        let frames = 256;
        let samples: Vec<f32> = (0..frames)
            .map(|i| {
                let t = i as f32 / FS;
                1.6 * (std::f32::consts::TAU * 220.0 * t).sin()
            })
            .collect();

        let mut path = std::env::temp_dir();
        path.push(format!("mono-01-render-demo-{}.wav", std::process::id()));
        write_wav(path.to_str().expect("a utf-8 path"), &samples).expect("the demo is written");

        let read = mxm_audio_file_decode::decode_file(
            &path,
            &mxm_audio_file_decode::Limits::new(
                usize::MAX,
                mxm_audio_file_decode::AtLimit::Refuse,
                mxm_audio_file_decode::Keep::AllUpTo(2),
            ),
        )
        .expect("the demo file parses");
        assert_eq!(read.channels, 1, "the demo wrote the wrong channel count");
        assert_eq!(
            read.sample_rate, 48_000,
            "the demo wrote the wrong sample rate"
        );
        assert_eq!(read.frames(), frames, "the demo dropped or invented frames");

        let peak = mxm_measure::level::peak(&read.interleaved).expect("a finite file");
        assert!(
            peak > 0.999,
            "this writer clamps rather than leaving headroom, so a loud source should reach full \
             scale: peak {peak}"
        );
        std::fs::remove_file(&path).ok();
    }
}
