//! mxm-mono-01's routing as the collection's modulation standard checks it
//! (`mxm_modulation::conformance`; `plans/plan-modulation-standard.md`).
//!
//! Behind the `conformance` feature, which only `[dev-dependencies]` enable — this crate's own
//! tests, and the plugin's, whose route readings are held to [`Declared::deliver`] — so no shipped
//! graph carries it. [`Declared`] answers every question through [`crate::routing`]'s own tables
//! and a real [`Graph`], never a copy of them.

use mxm_modulation::conformance::{Declaration, Kind};
use mxm_modulation::standard::{self, Offer, Performance};

use crate::routing::{
    self, Graph, KEY_UNIT_SEMITONES, Routing, SOURCE_NAMES, SOURCES, TARGET_NAMES, TARGETS, target,
};

/// What each target is, for the standard: a pitch, a symmetric width, a cutoff and the amplitude
/// factor.
const KINDS: [Kind; TARGETS] = [Kind::Pitch, Kind::Width, Kind::Cutoff, Kind::Amplitude];

/// mxm-mono-01's routing declaration.
#[derive(Debug, Clone, Copy, Default)]
pub struct Declared;

/// Exactly one route, at `amount`.
fn one_route(target: usize, source: usize, amount: f32) -> Routing {
    let mut routing = Routing::new();
    routing.present[target][source] = true;
    routing.amounts[target][source] = amount;
    routing.compact();
    routing
}

impl Declaration for Declared {
    fn sources(&self) -> usize {
        SOURCES
    }

    fn targets(&self) -> usize {
        TARGETS
    }

    fn performance(&self, source: usize) -> Option<Performance> {
        routing::PERFORMANCE[source]
    }

    fn kind(&self, target: usize) -> Kind {
        KINDS[target]
    }

    fn machine(&self, target: usize, source: usize) -> bool {
        routing::machine(target, source)
    }

    fn offered(&self, target: usize, source: usize) -> Offer {
        routing::offer(target, source)
    }

    fn key_unit(&self) -> f32 {
        KEY_UNIT_SEMITONES
    }

    /// One route alone through a voice's own [`Graph`], its source publishing `raw`; Amplitude
    /// through the factor the voice applies.
    fn deliver(&self, target: usize, source: usize, amount: f32, raw: f32) -> f32 {
        let routing = one_route(target, source, amount);
        let mut graph = Graph::new();
        graph.set_topology(&routing);
        graph.begin_sample();
        graph.write(source, raw);
        let sum = graph.sum(target, &routing);
        if target == target::AMPLITUDE {
            standard::amplitude_factor(sum) - 1.0
        } else {
            sum
        }
    }

    fn name(&self, target: usize, source: usize) -> String {
        format!("{} from {}", TARGET_NAMES[target], SOURCE_NAMES[source])
    }
}

#[cfg(test)]
mod tests {
    use mxm_modulation::conformance::{self, Case, Input};

    use super::*;
    use crate::routing::source;
    use crate::voice::{NoteId, Patch, Retrigger, VcaSource, Voice};

    const RATE: f32 = 48_000.0;

    fn report(result: Result<(), Vec<String>>) {
        if let Err(failures) = result {
            panic!("{} failure(s):\n{}", failures.len(), failures.join("\n"));
        }
    }

    fn id(note: u8) -> NoteId {
        NoteId {
            voice_id: None,
            channel: 0,
            note,
        }
    }

    /// **Every pair means what the standard says**: offered as `standard::offer` says, nothing at
    /// its source's rest, a meaningful move at full, and the standard reach for every pair the
    /// SH-101 did not have.
    ///
    /// Falsified before trusted: with the added cutoff reach left at the envelope's six octaves, it
    /// names every added cutoff pair from a performance source.
    #[test]
    fn every_pair_means_what_the_standard_says() {
        report(conformance::check_declaration(&Declared));
    }

    /// **A voice publishes what the standard says**: Key from middle C over its unit, Velocity as
    /// `v − 1`, the bender as its lever, each exactly zero at rest.
    ///
    /// Falsified before trusted: publishing the bend in semitones over twelve, as before, fails at
    /// every lever position but the centre.
    #[test]
    fn a_voice_publishes_what_the_standard_says() {
        report(conformance::check_publishers(&Declared, |from, input| {
            // A route reads the source, so the voice publishes it; at zero depth nothing moves.
            let routing = one_route(target::CUTOFF, from, 0.0);
            let mut voice = Voice::new();
            voice.set_sample_rate(RATE);
            voice.set_topology(&routing);
            let mut patch = Patch::default();
            let mut note = 60;
            match input {
                Input::Note(n) => note = n,
                Input::Normalised(value) if from == source::VELOCITY => patch.velocity = value,
                Input::Normalised(value) if from == source::WHEEL => patch.wheel = value,
                Input::Normalised(value) if from == source::PRESSURE => patch.pressure = value,
                Input::Lever(value) => {
                    patch.bend = value;
                    patch.bend_semitones = value * 2.0;
                }
                _ => {}
            }
            if !matches!(input, Input::Normalised(_)) || from != source::VELOCITY {
                patch.velocity = 1.0;
            }
            voice.note_on(id(note), Retrigger::Legato);
            voice.process(&patch, &routing);
            voice.published_for_test(from)
        }));
    }

    /// **Key is the note the voice is sounding, glide and all** — and not the range switch, which
    /// is the oscillator's: a glide's first sample reads where the voice was, and one time constant
    /// in it reads the lag's own value.
    ///
    /// Falsified before trusted: publishing the key before the glide reads 72 on the first sample.
    #[test]
    fn key_follows_the_glide_and_not_the_range_switch() {
        let routing = one_route(target::CUTOFF, source::KEY, 0.0);
        let mut voice = Voice::new();
        voice.set_sample_rate(RATE);
        voice.set_topology(&routing);
        let patch = Patch {
            glide_time_s: 0.1,
            range_offset: -12.0,
            ..Patch::default()
        };
        voice.note_on(id(48), Retrigger::Legato);
        voice.process(&patch, &routing);
        assert_eq!(
            voice.published_for_test(source::KEY) * KEY_UNIT_SEMITONES + 60.0,
            48.0,
            "the range switch is not the keyboard"
        );
        voice.note_on(id(72), Retrigger::Legato);
        voice.process(&patch, &routing);
        let started = voice.published_for_test(source::KEY) * KEY_UNIT_SEMITONES + 60.0;
        assert!(
            (48.0..48.1).contains(&started),
            "a glide starts where the voice was: {started}"
        );
        for _ in 1..(0.1 * RATE) as usize {
            voice.process(&patch, &routing);
        }
        let one_tau = voice.published_for_test(source::KEY) * KEY_UNIT_SEMITONES + 60.0;
        let expected = 72.0 - 24.0 * (-1.0f32).exp();
        assert!(
            (one_tau - expected).abs() < 0.01,
            "one time constant in, the key reads the lag: {one_tau}, not {expected}"
        );
    }

    /// **After a release, no performance route holds a note open** — every pair, both halves,
    /// the softest and hardest notes and the keyboard's ends, gestures held at full through the
    /// note and let go at the release, through the envelope and through the gate.
    #[test]
    fn after_a_release_no_performance_route_holds_a_note_open() {
        report(conformance::check_release_silence(
            &Declared,
            &[],
            |case: Case| {
                [VcaSource::Envelope, VcaSource::Gate]
                    .into_iter()
                    .all(|vca_source| {
                        let routing = one_route(case.target, case.source, case.amount);
                        let mut voice = Voice::new();
                        voice.set_sample_rate(RATE);
                        voice.set_topology(&routing);
                        let mut patch = Patch {
                            velocity: case.velocity,
                            wheel: 1.0,
                            pressure: 1.0,
                            bend: 1.0,
                            bend_semitones: 2.0,
                            release_s: 0.05,
                            vca_source,
                            ..Patch::default()
                        };
                        voice.note_on(id(case.key), Retrigger::Legato);
                        for _ in 0..(0.1 * RATE) as usize {
                            voice.process(&patch, &routing);
                        }
                        voice.note_off(None, 0, case.key, Retrigger::Legato);
                        patch.wheel = 0.0;
                        patch.pressure = 0.0;
                        patch.bend = 0.0;
                        patch.bend_semitones = 0.0;
                        (0..(2.0 * RATE) as usize)
                            .any(|_| voice.process(&patch, &routing) == 0.0 && !voice.is_active())
                    })
            },
        ));
    }
}
