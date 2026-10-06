//! Presets: this instrument's factory set, and what the collection's preset crate needs of it.
//!
//! The format, the library on disk, favourites, the loaded identity and the app-bar controls are
//! `mxm-preset`'s — one crate for every instrument and effect, extracted from the five verbatim
//! copies this file used to be one of (`plugins/AGENTS.md`, *A preset is parameter values*). What
//! is left here is what only this instrument knows: its id, its parameters, and its sounds.

use std::sync::RwLock;

pub use mxm_preset::{
    Category, Entry, INIT_NAME, Library, Loaded, Origin, Preset, PresetIdentity, Refused, Value,
    factory, loaded, mark_loaded, mark_none, read_favourites, snapshot, write_favourites,
};

use crate::params::MxmMono01Params;

/// **The tempo syncs this plugin gained on 2026-09-25** (`plans/plan-tempo-sync-controls.md`). A
/// preset file written before them was written unsynced, so each loads off rather than keeping the
/// instance's sync, and without reporting a missing control.
pub(crate) const TEMPO_SYNC_IDS: &[&str] = &["lfosync"];

impl mxm_preset::Instrument for MxmMono01Params {
    fn clap_id(&self) -> &'static str {
        crate::CLAP_ID
    }

    /// In declaration order, from the one list the editor draws from.
    fn parameters(&self) -> Vec<(&'static str, &dyn mxm_preset::ErasedParam)> {
        // **The routes are parameters of this instrument like any other**, and a preset that did not
        // name them would leave the previous patch's modulation in place — which is precisely the
        // hazard `plugins/AGENTS.md`'s coverage rule exists for, and it bites hardest here because
        // the machine's own filter envelope, PWM and key tracking *are* routes now.
        crate::editor::sections::all_parameters(self)
            .into_iter()
            .map(|bound| (bound.id, bound.param))
            .chain(self.routes.parameters())
            .collect()
    }

    fn identity(&self) -> &RwLock<PresetIdentity> {
        &self.preset
    }

    fn factory_files(&self) -> &'static [(&'static str, &'static str)] {
        FACTORY_FILES
    }

    fn default_missing_legacy_parameter(&self, id: &str) -> bool {
        TEMPO_SYNC_IDS.contains(&id)
    }
}

/// The factory set, compiled in.
///
/// **Five files, and Init is not one of them** — see [`Preset::init`]. These five are *content*: a
/// sound nobody can read is a sound nobody can learn from, so they are files rather than code.
pub const FACTORY_FILES: &[(&str, &str)] = &[
    ("Plucked bass", include_str!("../presets/plucked-bass.json")),
    ("Sub bass", include_str!("../presets/sub-bass.json")),
    ("Deep sub", include_str!("../presets/deep-sub.json")),
    ("Rubber bass", include_str!("../presets/rubber-bass.json")),
    ("Growl bass", include_str!("../presets/growl-bass.json")),
    ("Round bass", include_str!("../presets/round-bass.json")),
    ("Acid line", include_str!("../presets/acid-line.json")),
    ("Pick bass", include_str!("../presets/pick-bass.json")),
    ("Hollow lead", include_str!("../presets/hollow-lead.json")),
    ("Bright lead", include_str!("../presets/bright-lead.json")),
    ("Square lead", include_str!("../presets/square-lead.json")),
    ("Glide lead", include_str!("../presets/glide-lead.json")),
    ("Octave lead", include_str!("../presets/octave-lead.json")),
    ("Vibrato lead", include_str!("../presets/vibrato-lead.json")),
    ("Reed lead", include_str!("../presets/reed-lead.json")),
    ("Whistle lead", include_str!("../presets/whistle-lead.json")),
    (
        "Screaming lead",
        include_str!("../presets/screaming-lead.json"),
    ),
    ("Soft pad", include_str!("../presets/soft-pad.json")),
    ("Warm pad", include_str!("../presets/warm-pad.json")),
    ("Glass pad", include_str!("../presets/glass-pad.json")),
    ("Breath pad", include_str!("../presets/breath-pad.json")),
    ("Sweep pad", include_str!("../presets/sweep-pad.json")),
    ("Dark pad", include_str!("../presets/dark-pad.json")),
    ("String pad", include_str!("../presets/string-pad.json")),
    ("Choir pad", include_str!("../presets/choir-pad.json")),
    ("Drone", include_str!("../presets/drone.json")),
    ("Swell", include_str!("../presets/swell.json")),
    ("Wood pluck", include_str!("../presets/wood-pluck.json")),
    ("Harp", include_str!("../presets/harp.json")),
    ("Marimba", include_str!("../presets/marimba.json")),
    ("Clav", include_str!("../presets/clav.json")),
    ("Bell", include_str!("../presets/bell.json")),
    (
        "Electric piano",
        include_str!("../presets/electric-piano.json"),
    ),
    ("Organ", include_str!("../presets/organ.json")),
    ("Music box", include_str!("../presets/music-box.json")),
    ("Brass", include_str!("../presets/brass.json")),
    ("Cello", include_str!("../presets/cello.json")),
    ("Violin", include_str!("../presets/violin.json")),
    ("Noise sweep", include_str!("../presets/noise-sweep.json")),
    ("Wind", include_str!("../presets/wind.json")),
    ("Rain", include_str!("../presets/rain.json")),
    ("Siren", include_str!("../presets/siren.json")),
    ("Laser", include_str!("../presets/laser.json")),
    ("Kick", include_str!("../presets/kick.json")),
    ("Snare", include_str!("../presets/snare.json")),
    ("Hat", include_str!("../presets/hat.json")),
    ("Tom", include_str!("../presets/tom.json")),
    ("Blip", include_str!("../presets/blip.json")),
    ("Pulse gate", include_str!("../presets/pulse-gate.json")),
    ("Ramp sweep", include_str!("../presets/ramp-sweep.json")),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// **A project saved before the tempo syncs restores them Off** (`mxm_preset::add_switches_off`),
    /// whatever this instance had.
    #[test]
    fn an_older_state_restores_the_tempo_syncs_off() {
        use nice_plug::prelude::Plugin as _;
        let mut state = nice_plug::prelude::PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        };
        crate::MxmMono01::filter_state(&mut state);
        for id in TEMPO_SYNC_IDS {
            assert!(
                matches!(
                    state.params.get(*id),
                    Some(nice_plug::plugin::ParamValue::Bool(false))
                ),
                "{{id}} was not restored off"
            );
        }
    }

    /// **A preset saved before the tempo syncs loads them off, and cleanly** ([`TEMPO_SYNC_IDS`]).
    #[test]
    fn a_preset_from_before_the_tempo_syncs_loads_them_off() {
        let params = crate::params::MxmMono01Params::default();
        let mut old = mxm_preset::Preset::init(&params);
        for id in TEMPO_SYNC_IDS {
            old.params.remove(*id);
        }
        let (writes, problems) = old.resolve(&params);
        assert!(problems.is_empty(), "{{problems:?}}");
        for id in TEMPO_SYNC_IDS {
            assert!(
                writes.iter().any(|(w, _, v)| w == id && *v == 0.0),
                "{{id}} was not written off"
            );
        }
    }

    use mxm_preset::user_root;

    fn params() -> MxmMono01Params {
        MxmMono01Params::default()
    }

    /// Prints every parameter as `id | name | default normalised | default text`.
    ///
    /// A facility, not a test: designing a factory preset means choosing normalised values, and
    /// choosing them blind is how a preset ends up with a filter at 0.5 that nobody meant.
    ///
    /// ```text
    /// cargo test -p mxm-mono-01 --lib the_parameter_table -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "prints what each normalised value means, for preset design"]
    fn the_mapping_table() {
        let params = params();
        for bound in crate::editor::sections::all_parameters(&params) {
            let steps: Vec<String> = (0..=10)
                .map(|i| {
                    let v = i as f32 / 10.0;
                    format!("{v:.1}={}", bound.param.format(v))
                })
                .collect();
            eprintln!("{:<12} {}", bound.id, steps.join("  "));
        }
    }

    #[test]
    #[ignore = "prints the parameter table for preset design"]
    fn the_parameter_table() {
        let params = params();
        for bound in crate::editor::sections::all_parameters(&params) {
            eprintln!(
                "{:<12} | {:<22} | {:.4} | {}",
                bound.id,
                bound.param.name(),
                bound.param.default_normalised(),
                bound.param.text(),
            );
        }
    }

    /// The five factory sounds, as **overrides on the defaults**.
    ///
    /// Written here rather than as twenty-seven numbers each: what a preset *is* is the handful of
    /// values that make it that sound, and a file full of defaults hides them. The generator below
    /// turns each into a complete file, because the format takes no sparse overlays — an overlay's
    /// meaning would change the day the defaults are retuned.
    /// The factory sounds, as **overrides on the defaults**.
    ///
    /// Written as the handful of values that make each sound rather than as twenty-seven numbers
    /// apiece: a file full of defaults hides the three that matter. `write_the_factory_presets`
    /// turns each into a complete file, because the format takes no sparse overlays — an overlay's
    /// meaning would change the day the defaults were retuned.
    ///
    /// **Fifty, and Init is not one of them.** Init is generated from the parameter defaults and has
    /// no file at all; see [`Preset::init`].
    const FACTORY_DESIGN: &[Design] = &[
        (
            "Plucked bass",
            Category::Bass,
            // the pluck: a short filter envelope and nothing held
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.8),
                ("sublevel", 0.6),
                ("cutoff", 0.3),
                ("resonance", 0.35),
                ("mod_cutoff_env", 0.85),
                ("attack", 0.1),
                ("decay", 0.29),
                ("sustain", 0.0),
                ("release", 0.3),
                ("mod_cutoff_key", 0.75),
            ],
        ),
        (
            "Sub bass",
            Category::Bass,
            // one octave below the key, with a little saw left to hear it by
            &[
                ("oscrange", 0.0),
                ("subtype", 0.0),
                ("sublevel", 1.0),
                ("sawlevel", 0.35),
                ("cutoff", 0.3),
                ("mod_cutoff_env", 0.5),
                ("attack", 0.1),
                ("sustain", 1.0),
                ("release", 0.28),
                ("glide", 0.2),
            ],
        ),
        (
            "Deep sub",
            Category::Bass,
            // as low as this instrument goes and still a note
            &[
                ("oscrange", 0.0),
                ("subtype", 0.0),
                ("sublevel", 1.0),
                ("sawlevel", 0.0),
                ("cutoff", 0.26),
                ("resonance", 0.1),
                ("mod_cutoff_env", 0.5),
                ("attack", 0.12),
                ("sustain", 1.0),
                ("release", 0.35),
            ],
        ),
        (
            "Rubber bass",
            Category::Bass,
            // narrow pulse and a fast envelope: it bounces
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.0),
                ("pulselevel", 0.9),
                ("pulsewidth", 0.25),
                ("sublevel", 0.4),
                ("cutoff", 0.33),
                ("resonance", 0.45),
                ("mod_cutoff_env", 0.8),
                ("attack", 0.05),
                ("decay", 0.25),
                ("sustain", 0.15),
                ("release", 0.25),
                ("mod_cutoff_key", 0.7),
            ],
        ),
        (
            "Growl bass",
            Category::Bass,
            // resonance high enough to be part of the tone
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.9),
                ("pulselevel", 0.5),
                ("pulsewidth", 0.35),
                ("cutoff", 0.31),
                ("resonance", 0.68),
                ("mod_cutoff_env", 0.78),
                ("attack", 0.08),
                ("decay", 0.34),
                ("sustain", 0.3),
                ("release", 0.28),
                ("mod_cutoff_key", 0.725),
            ],
        ),
        (
            "Round bass",
            Category::Bass,
            // no resonance at all, so it sits under everything
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.85),
                ("sublevel", 0.45),
                ("cutoff", 0.34),
                ("resonance", 0.0),
                ("mod_cutoff_env", 0.62),
                ("attack", 0.12),
                ("decay", 0.42),
                ("sustain", 0.55),
                ("release", 0.3),
            ],
        ),
        (
            "Acid line",
            Category::Bass,
            // the one everybody reaches for: high resonance, short envelope, glide
            &[
                ("oscrange", 0.0),
                ("sawlevel", 1.0),
                ("cutoff", 0.3),
                ("resonance", 0.88),
                ("mod_cutoff_env", 0.88),
                ("attack", 0.0),
                ("decay", 0.27),
                ("sustain", 0.0),
                ("release", 0.22),
                ("mod_cutoff_key", 0.775),
                ("glide", 0.3),
            ],
        ),
        (
            "Pick bass",
            Category::Bass,
            // the opposite: all attack
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.6),
                ("pulselevel", 0.7),
                ("pulsewidth", 0.2),
                ("noiselevel", 0.12),
                ("cutoff", 0.38),
                ("resonance", 0.4),
                ("mod_cutoff_env", 0.82),
                ("attack", 0.0),
                ("decay", 0.24),
                ("sustain", 0.1),
                ("release", 0.24),
            ],
        ),
        (
            "Hollow lead",
            Category::Lead,
            // a narrow pulse, which is where hollow comes from
            &[
                ("oscrange", 0.6667),
                ("sawlevel", 0.0),
                ("pulselevel", 0.85),
                ("sublevel", 0.2),
                ("pulsewidth", 0.18),
                ("cutoff", 0.62),
                ("resonance", 0.55),
                ("mod_cutoff_env", 0.7),
                ("attack", 0.13),
                ("decay", 0.42),
                ("sustain", 0.55),
                ("release", 0.33),
                ("mod_cutoff_key", 0.8),
                ("mod_pitch_lfo", 0.53),
                ("lforate", 0.5),
            ],
        ),
        (
            "Bright lead",
            Category::Lead,
            // open filter and full key tracking, so it stays bright up the keyboard
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 1.0),
                ("cutoff", 0.72),
                ("resonance", 0.3),
                ("mod_cutoff_env", 0.62),
                ("attack", 0.06),
                ("decay", 0.45),
                ("sustain", 0.7),
                ("release", 0.3),
                ("mod_cutoff_key", 0.95),
            ],
        ),
        (
            "Square lead",
            Category::Lead,
            // the plain one, and none the worse for it
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.0),
                ("pulselevel", 1.0),
                ("pulsewidth", 0.5),
                ("cutoff", 0.6),
                ("resonance", 0.35),
                ("mod_cutoff_env", 0.6),
                ("attack", 0.08),
                ("sustain", 0.75),
                ("release", 0.28),
                ("mod_cutoff_key", 0.85),
            ],
        ),
        (
            "Glide lead",
            Category::Lead,
            // long portamento and legato priority: one line, never restarted
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.9),
                ("sublevel", 0.3),
                ("cutoff", 0.58),
                ("resonance", 0.4),
                ("mod_cutoff_env", 0.62),
                ("attack", 0.1),
                ("sustain", 0.8),
                ("release", 0.35),
                ("glide", 0.52),
                ("mod_cutoff_key", 0.8),
            ],
        ),
        (
            "Octave lead",
            Category::Lead,
            // the sub an octave down, which thickens it without detuning anything
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.8),
                ("sublevel", 0.7),
                ("subtype", 0.0),
                ("cutoff", 0.62),
                ("resonance", 0.28),
                ("mod_cutoff_env", 0.6),
                ("attack", 0.07),
                ("sustain", 0.75),
                ("release", 0.3),
                ("mod_cutoff_key", 0.825),
            ],
        ),
        (
            "Vibrato lead",
            Category::Lead,
            // the LFO on pitch, gently, which is what it is there for
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.95),
                ("cutoff", 0.64),
                ("resonance", 0.3),
                ("mod_cutoff_env", 0.58),
                ("attack", 0.12),
                ("sustain", 0.8),
                ("release", 0.32),
                ("mod_pitch_lfo", 0.57),
                ("lforate", 0.48),
                ("mod_cutoff_key", 0.85),
            ],
        ),
        (
            "Reed lead",
            Category::Lead,
            // noise in the attack, like breath before tone
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.5),
                ("pulselevel", 0.6),
                ("pulsewidth", 0.3),
                ("noiselevel", 0.18),
                ("cutoff", 0.55),
                ("resonance", 0.42),
                ("mod_cutoff_env", 0.66),
                ("attack", 0.16),
                ("decay", 0.4),
                ("sustain", 0.65),
                ("release", 0.3),
            ],
        ),
        (
            "Whistle lead",
            Category::Lead,
            // almost no harmonics left: a filtered pulse near a sine
            &[
                ("oscrange", 0.6667),
                ("sawlevel", 0.0),
                ("pulselevel", 0.9),
                ("pulsewidth", 0.5),
                ("cutoff", 0.45),
                ("resonance", 0.2),
                ("mod_cutoff_env", 0.5),
                ("attack", 0.14),
                ("sustain", 0.85),
                ("release", 0.3),
                ("mod_cutoff_key", 0.925),
                ("mod_pitch_lfo", 0.54),
                ("lforate", 0.45),
            ],
        ),
        (
            "Screaming lead",
            Category::Lead,
            // resonance past the point of politeness
            &[
                ("oscrange", 0.6667),
                ("sawlevel", 1.0),
                ("cutoff", 0.55),
                ("resonance", 0.92),
                ("mod_cutoff_env", 0.72),
                ("attack", 0.04),
                ("decay", 0.44),
                ("sustain", 0.6),
                ("release", 0.28),
                ("mod_cutoff_key", 0.9),
            ],
        ),
        (
            "Soft pad",
            Category::Pad,
            // slow in, slow out, and the LFO moving the pulse width
            &[
                ("sawlevel", 0.6),
                ("pulselevel", 0.5),
                ("sublevel", 0.3),
                ("mod_width_lfo", 0.7),
                ("lforate", 0.15),
                ("cutoff", 0.55),
                ("resonance", 0.15),
                ("mod_cutoff_env", 0.62),
                ("mod_cutoff_lfo", 0.625),
                ("attack", 0.46),
                ("sustain", 0.75),
                ("release", 0.52),
                ("mod_cutoff_key", 0.675),
            ],
        ),
        (
            "Warm pad",
            Category::Pad,
            // no resonance and a low corner: nothing to catch the ear
            &[
                ("sawlevel", 0.75),
                ("sublevel", 0.45),
                ("cutoff", 0.48),
                ("resonance", 0.08),
                ("mod_cutoff_env", 0.58),
                ("attack", 0.5),
                ("decay", 0.6),
                ("sustain", 0.85),
                ("release", 0.58),
            ],
        ),
        (
            "Glass pad",
            Category::Pad,
            // a narrow pulse held open, which is where the glassiness is
            &[
                ("oscrange", 0.6667),
                ("sawlevel", 0.0),
                ("pulselevel", 0.9),
                ("pulsewidth", 0.22),
                ("mod_width_lfo", 0.65),
                ("lforate", 0.12),
                ("cutoff", 0.66),
                ("resonance", 0.35),
                ("mod_cutoff_env", 0.55),
                ("attack", 0.44),
                ("sustain", 0.8),
                ("release", 0.55),
            ],
        ),
        (
            "Breath pad",
            Category::Pad,
            // noise mixed in far enough to hear, not far enough to hiss
            &[
                ("sawlevel", 0.55),
                ("noiselevel", 0.3),
                ("cutoff", 0.52),
                ("resonance", 0.18),
                ("mod_cutoff_env", 0.6),
                ("mod_cutoff_lfo", 0.59),
                ("lforate", 0.1),
                ("attack", 0.52),
                ("sustain", 0.75),
                ("release", 0.6),
            ],
        ),
        (
            "Sweep pad",
            Category::Pad,
            // the LFO on the filter, slow enough to be a shape rather than a wobble
            &[
                ("sawlevel", 0.85),
                ("sublevel", 0.25),
                ("cutoff", 0.44),
                ("resonance", 0.45),
                ("mod_cutoff_env", 0.55),
                ("mod_cutoff_lfo", 0.775),
                ("lforate", 0.08),
                ("attack", 0.42),
                ("sustain", 0.8),
                ("release", 0.55),
            ],
        ),
        (
            "Dark pad",
            Category::Pad,
            // the corner low and the release long: it stays behind everything
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.7),
                ("sublevel", 0.5),
                ("cutoff", 0.38),
                ("resonance", 0.12),
                ("mod_cutoff_env", 0.56),
                ("attack", 0.48),
                ("sustain", 0.8),
                ("release", 0.62),
            ],
        ),
        (
            "String pad",
            Category::Strings,
            // a fast enough attack to be played, a slow enough one to be a section
            &[
                ("sawlevel", 1.0),
                ("cutoff", 0.58),
                ("resonance", 0.2),
                ("mod_cutoff_env", 0.6),
                ("attack", 0.36),
                ("decay", 0.55),
                ("sustain", 0.78),
                ("release", 0.5),
                ("mod_cutoff_key", 0.7),
            ],
        ),
        (
            "Choir pad",
            Category::Pad,
            // pulse width moving slowly, which is the closest this gets to voices
            &[
                ("sawlevel", 0.3),
                ("pulselevel", 0.8),
                ("pulsewidth", 0.45),
                ("mod_width_lfo", 0.775),
                ("lforate", 0.1),
                ("cutoff", 0.5),
                ("resonance", 0.22),
                ("mod_cutoff_env", 0.55),
                ("attack", 0.46),
                ("sustain", 0.82),
                ("release", 0.56),
            ],
        ),
        (
            "Drone",
            Category::Drone,
            // no envelope movement at all: it arrives and stays
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.8),
                ("sublevel", 0.6),
                ("cutoff", 0.42),
                ("resonance", 0.3),
                ("mod_cutoff_env", 0.5),
                ("attack", 0.3),
                ("decay", 0.5),
                ("sustain", 1.0),
                ("release", 0.45),
            ],
        ),
        (
            "Swell",
            Category::Pad,
            // eight seconds in and twelve out, which is the whole instrument's range
            &[
                ("sawlevel", 0.9),
                ("sublevel", 0.3),
                ("cutoff", 0.5),
                ("resonance", 0.25),
                ("mod_cutoff_env", 0.7),
                ("attack", 0.72),
                ("sustain", 0.9),
                ("release", 0.75),
            ],
        ),
        (
            "Wood pluck",
            Category::Pluck,
            // short, narrow, and gone
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.0),
                ("pulselevel", 0.9),
                ("pulsewidth", 0.2),
                ("cutoff", 0.52),
                ("resonance", 0.4),
                ("mod_cutoff_env", 0.75),
                ("attack", 0.0),
                ("decay", 0.26),
                ("sustain", 0.0),
                ("release", 0.22),
                ("mod_cutoff_key", 0.8),
            ],
        ),
        (
            "Harp",
            Category::Pluck,
            // a longer decay on the same idea
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.85),
                ("cutoff", 0.6),
                ("resonance", 0.25),
                ("mod_cutoff_env", 0.7),
                ("attack", 0.0),
                ("decay", 0.42),
                ("sustain", 0.0),
                ("release", 0.38),
                ("mod_cutoff_key", 0.85),
            ],
        ),
        (
            "Marimba",
            Category::Percussion,
            // so short it is almost a click, which is the point
            &[
                ("oscrange", 0.6667),
                ("sawlevel", 0.0),
                ("pulselevel", 0.85),
                ("pulsewidth", 0.5),
                ("cutoff", 0.58),
                ("resonance", 0.2),
                ("mod_cutoff_env", 0.72),
                ("attack", 0.0),
                ("decay", 0.22),
                ("sustain", 0.0),
                ("release", 0.18),
                ("mod_cutoff_key", 0.875),
            ],
        ),
        (
            "Clav",
            Category::Keys,
            // narrow pulse, hard envelope, resonance for the bite
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.0),
                ("pulselevel", 1.0),
                ("pulsewidth", 0.14),
                ("cutoff", 0.55),
                ("resonance", 0.58),
                ("mod_cutoff_env", 0.8),
                ("attack", 0.0),
                ("decay", 0.28),
                ("sustain", 0.05),
                ("release", 0.2),
                ("mod_cutoff_key", 0.825),
            ],
        ),
        (
            "Bell",
            Category::Keys,
            // high, brief, and resonant enough to ring
            &[
                ("oscrange", 1.0),
                ("sawlevel", 0.0),
                ("pulselevel", 0.8),
                ("pulsewidth", 0.3),
                ("cutoff", 0.7),
                ("resonance", 0.72),
                ("mod_cutoff_env", 0.6),
                ("attack", 0.0),
                ("decay", 0.4),
                ("sustain", 0.0),
                ("release", 0.42),
                ("mod_cutoff_key", 0.925),
            ],
        ),
        (
            "Electric piano",
            Category::Keys,
            // a soft attack over a fast decay: the shape, if not the sound
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.4),
                ("pulselevel", 0.7),
                ("pulsewidth", 0.42),
                ("sublevel", 0.25),
                ("cutoff", 0.56),
                ("resonance", 0.18),
                ("mod_cutoff_env", 0.68),
                ("attack", 0.06),
                ("decay", 0.44),
                ("sustain", 0.25),
                ("release", 0.34),
                ("mod_cutoff_key", 0.775),
            ],
        ),
        (
            "Organ",
            Category::Keys,
            // the VCA on the gate, so it is on or off and never in between
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.0),
                ("pulselevel", 0.9),
                ("pulsewidth", 0.5),
                ("sublevel", 0.7),
                ("cutoff", 0.62),
                ("resonance", 0.1),
                ("mod_cutoff_env", 0.5),
                ("vcasource", 1.0),
                ("mod_cutoff_key", 0.75),
            ],
        ),
        (
            "Music box",
            Category::Keys,
            // two octaves up and almost no body
            &[
                ("oscrange", 1.0),
                ("sawlevel", 0.0),
                ("pulselevel", 0.75),
                ("pulsewidth", 0.5),
                ("cutoff", 0.74),
                ("resonance", 0.45),
                ("mod_cutoff_env", 0.55),
                ("attack", 0.0),
                ("decay", 0.34),
                ("sustain", 0.0),
                ("release", 0.32),
                ("mod_cutoff_key", 0.95),
            ],
        ),
        (
            "Brass",
            Category::Brass,
            // the filter envelope doing the swell that brass does
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 1.0),
                ("cutoff", 0.46),
                ("resonance", 0.28),
                ("mod_cutoff_env", 0.78),
                ("attack", 0.22),
                ("decay", 0.42),
                ("sustain", 0.7),
                ("release", 0.32),
                ("mod_cutoff_key", 0.725),
            ],
        ),
        (
            "Cello",
            Category::Strings,
            // an octave down and bowed rather than struck
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.95),
                ("cutoff", 0.46),
                ("resonance", 0.22),
                ("mod_cutoff_env", 0.62),
                ("attack", 0.28),
                ("decay", 0.5),
                ("sustain", 0.8),
                ("release", 0.44),
                ("mod_pitch_lfo", 0.535),
                ("lforate", 0.42),
            ],
        ),
        (
            "Violin",
            Category::Strings,
            // up two octaves, with the vibrato faster
            &[
                ("oscrange", 0.6667),
                ("sawlevel", 0.95),
                ("cutoff", 0.62),
                ("resonance", 0.3),
                ("mod_cutoff_env", 0.58),
                ("attack", 0.26),
                ("sustain", 0.82),
                ("release", 0.38),
                ("mod_pitch_lfo", 0.555),
                ("lforate", 0.52),
                ("mod_cutoff_key", 0.8),
            ],
        ),
        (
            "Noise sweep",
            Category::Fx,
            // the filter opening across a long envelope, on noise alone
            &[
                ("sawlevel", 0.0),
                ("noiselevel", 1.0),
                ("cutoff", 0.28),
                ("resonance", 0.8),
                ("mod_cutoff_env", 0.95),
                ("mod_cutoff_lfo", 0.575),
                ("lforate", 0.1),
                ("attack", 0.3),
                ("decay", 0.67),
                ("sustain", 0.0),
                ("release", 0.5),
            ],
        ),
        (
            "Wind",
            Category::Fx,
            // noise held open, with the filter drifting
            &[
                ("sawlevel", 0.0),
                ("noiselevel", 1.0),
                ("cutoff", 0.4),
                ("resonance", 0.55),
                ("mod_cutoff_env", 0.5),
                ("mod_cutoff_lfo", 0.725),
                ("lforate", 0.06),
                ("attack", 0.45),
                ("sustain", 0.85),
                ("release", 0.55),
            ],
        ),
        (
            "Rain",
            Category::Fx,
            // the random LFO on the filter, which is what it is for
            &[
                ("sawlevel", 0.0),
                ("noiselevel", 1.0),
                ("lfowave", 1.0),
                ("lforate", 0.62),
                ("cutoff", 0.52),
                ("resonance", 0.62),
                ("mod_cutoff_env", 0.5),
                ("mod_cutoff_lfo", 0.8),
                ("attack", 0.3),
                ("sustain", 0.8),
                ("release", 0.4),
            ],
        ),
        (
            "Siren",
            Category::Fx,
            // the LFO on pitch, deep and slow
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 1.0),
                ("mod_pitch_lfo", 0.925),
                ("lforate", 0.18),
                ("cutoff", 0.62),
                ("resonance", 0.35),
                ("mod_cutoff_env", 0.5),
                ("attack", 0.1),
                ("sustain", 0.9),
                ("release", 0.3),
            ],
        ),
        (
            "Laser",
            Category::Fx,
            // a filter envelope that closes instead of opening
            &[
                ("oscrange", 1.0),
                ("sawlevel", 1.0),
                ("cutoff", 0.78),
                ("resonance", 0.7),
                ("mod_cutoff_env", 0.05),
                ("attack", 0.0),
                ("decay", 0.26),
                ("sustain", 0.0),
                ("release", 0.2),
            ],
        ),
        (
            "Kick",
            Category::Percussion,
            // the sub, a hard envelope, and nothing else
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.0),
                ("sublevel", 1.0),
                ("subtype", 0.0),
                ("cutoff", 0.34),
                ("resonance", 0.3),
                ("mod_cutoff_env", 0.72),
                ("attack", 0.0),
                ("decay", 0.2),
                ("sustain", 0.0),
                ("release", 0.16),
            ],
        ),
        (
            "Snare",
            Category::Percussion,
            // noise with a short resonant envelope
            &[
                ("sawlevel", 0.0),
                ("noiselevel", 1.0),
                ("sublevel", 0.25),
                ("cutoff", 0.52),
                ("resonance", 0.62),
                ("mod_cutoff_env", 0.68),
                ("attack", 0.0),
                ("decay", 0.24),
                ("sustain", 0.0),
                ("release", 0.2),
            ],
        ),
        (
            "Hat",
            Category::Percussion,
            // the same, shorter and much brighter
            &[
                ("sawlevel", 0.0),
                ("noiselevel", 1.0),
                ("cutoff", 0.8),
                ("resonance", 0.4),
                ("mod_cutoff_env", 0.55),
                ("attack", 0.0),
                ("decay", 0.16),
                ("sustain", 0.0),
                ("release", 0.14),
            ],
        ),
        (
            "Tom",
            Category::Percussion,
            // sub and noise together, tuned by the key
            &[
                ("oscrange", 0.0),
                ("sawlevel", 0.0),
                ("sublevel", 0.85),
                ("noiselevel", 0.35),
                ("cutoff", 0.4),
                ("resonance", 0.42),
                ("mod_cutoff_env", 0.74),
                ("attack", 0.0),
                ("decay", 0.26),
                ("sustain", 0.0),
                ("release", 0.22),
                ("mod_cutoff_key", 0.9),
            ],
        ),
        (
            "Blip",
            Category::Pluck,
            // one millisecond of envelope, which is the shortest this instrument has
            &[
                ("oscrange", 0.6667),
                ("sawlevel", 0.0),
                ("pulselevel", 1.0),
                ("pulsewidth", 0.5),
                ("cutoff", 0.68),
                ("resonance", 0.5),
                ("mod_cutoff_env", 0.6),
                ("attack", 0.0),
                ("decay", 0.14),
                ("sustain", 0.0),
                ("release", 0.1),
                ("mod_cutoff_key", 0.925),
            ],
        ),
        (
            "Pulse gate",
            Category::Sequence,
            // the LFO chopping the filter in square steps
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 1.0),
                ("lfowave", 0.25),
                ("lforate", 0.55),
                ("cutoff", 0.4),
                ("resonance", 0.55),
                ("mod_cutoff_env", 0.5),
                ("mod_cutoff_lfo", 0.875),
                ("attack", 0.05),
                ("sustain", 0.9),
                ("release", 0.24),
            ],
        ),
        (
            "Ramp sweep",
            Category::Fx,
            // a sawtooth LFO on the filter, which resets rather than returns
            &[
                ("oscrange", 0.3333),
                ("sawlevel", 0.95),
                ("lfowave", 0.5),
                ("lforate", 0.22),
                ("cutoff", 0.38),
                ("resonance", 0.58),
                ("mod_cutoff_env", 0.5),
                ("mod_cutoff_lfo", 0.84),
                ("attack", 0.08),
                ("sustain", 0.88),
                ("release", 0.3),
            ],
        ),
    ];

    /// One designed sound: its name, its category, and the values that make it.
    type Design = (&'static str, Category, &'static [(&'static str, f32)]);

    /// Writes the five factory presets to `plugins/mxm-mono-01/presets/`.
    ///
    /// A facility, not a test — and the *only* thing that writes those files, so the numbers in
    /// `FACTORY_DESIGN` stay the readable statement of each sound and the JSON stays generated
    /// output. `every_factory_preset_covers_every_parameter` is what catches a file that has fallen
    /// behind a new parameter.
    ///
    /// ```text
    /// cargo test -p mxm-mono-01 --lib write_the_factory_presets -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "writes the factory preset files"]
    fn write_the_factory_presets() {
        let params = params();
        let bindings = mxm_preset::Instrument::parameters(&params);

        for (name, category, overrides) in FACTORY_DESIGN {
            let mut preset = Preset::init(&params);
            preset.name = (*name).to_owned();
            preset.category = *category;

            for (id, v) in *overrides {
                let bound = bindings
                    .iter()
                    .find(|(known_id, _)| *known_id == *id)
                    .unwrap_or_else(|| panic!("{name:?} names `{id}`, which is not a parameter"));
                preset.params.insert(
                    (*id).to_owned(),
                    Value {
                        v: *v,
                        text: bound.1.format(*v),
                    },
                );
            }

            // From the manifest directory, not the working one: a test's cwd is the crate root
            // and not the workspace root, which is the sort of thing that only says so once.
            let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("presets")
                .join(format!("{}.json", name.to_lowercase().replace(' ', "-")));
            std::fs::write(&file, preset.to_json()).expect("write the preset");
            eprintln!("wrote {}", file.display());
        }
    }

    #[test]
    fn every_designed_preset_names_real_parameters() {
        // Runs by default, unlike the generator: a typo in `FACTORY_DESIGN` would otherwise only
        // surface the next time somebody regenerated the files, and silently leave that value at
        // its default in the meantime.
        let params = params();
        let known = mxm_preset::Instrument::parameters(&params);
        for (name, _category, overrides) in FACTORY_DESIGN {
            for (id, v) in *overrides {
                assert!(
                    known.iter().any(|(known_id, _)| *known_id == *id),
                    "{name:?} names `{id}`, which is not a parameter of this instrument"
                );
                assert!(
                    (0.0..=1.0).contains(v),
                    "{name:?} sets `{id}` to {v}, which is not a normalised value"
                );
            }
        }
    }

    #[test]
    fn the_factory_files_match_the_design_they_were_generated_from() {
        // The generator is `#[ignore]`d, so nothing forces it to have been run. This is what says
        // the shipped files are the current design rather than a stale one — the same class of
        // mistake as a stale `.clap` bundle, and just as quiet.
        for (name, _category, overrides) in FACTORY_DESIGN {
            let (_, text) = FACTORY_FILES
                .iter()
                .find(|(file_name, _)| file_name == name)
                .unwrap_or_else(|| panic!("no factory file for {name:?}"));
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");

            for (id, v) in *overrides {
                let value = preset
                    .params
                    .get(*id)
                    .unwrap_or_else(|| panic!("{name:?} is missing `{id}`"));
                assert!(
                    (value.v - v).abs() < 1e-6,
                    "{name:?} ships `{id}` at {} but is designed at {v} — regenerate the files",
                    value.v
                );
            }
        }
    }

    #[test]
    fn the_user_root_is_under_this_instruments_own_id() {
        // Namespaced by CLAP id so a second instrument's presets cannot appear in this one's list.
        let Some(root) = user_root(crate::CLAP_ID) else {
            return;
        };
        assert!(root.ends_with("presets"));
        assert!(root.to_string_lossy().contains(crate::CLAP_ID));
    }

    /// How many octaves below the played note a preset's **lowest audible source** sits.
    ///
    /// The range switch and the sub shape both shift octaves, and it is easy to add them without
    /// noticing: Sub bass shipped at 16' *with* a two-octave sub and the sub as its only voice, so
    /// C3 sounded at 32 Hz and C2 at 16 Hz — at, and then below, the bottom of hearing. It was
    /// reported as clicking, because a square wave that low is a train of separate pulses rather
    /// than a note.
    fn octaves_below_the_key(preset: &Preset) -> f32 {
        let v = |id: &str| preset.params.get(id).map_or(0.0, |value| value.v);

        // 16' / 8' / 4' / 2' is one octave down, at pitch, one up, two up.
        let range = match v("oscrange") {
            r if r < 0.17 => -1.0,
            r if r < 0.5 => 0.0,
            r if r < 0.84 => 1.0,
            _ => 2.0,
        };

        // The sub is one or two octaves below the range, and only counts if it can be heard at all.
        let sub_audible = v("sublevel") > 0.05;
        let sub = if v("subtype") < 0.25 { 1.0 } else { 2.0 };

        // Saw, pulse and noise all sound at the range's own pitch.
        let main_audible = v("sawlevel") > 0.05 || v("pulselevel") > 0.05 || v("noiselevel") > 0.05;

        match (main_audible, sub_audible) {
            // The main is the highest thing sounding, so it sets the floor.
            (true, _) => -range,
            (false, true) => -range + sub,
            // Silent. Caught by its own test rather than by this one.
            (false, false) => 0.0,
        }
    }

    #[test]
    fn no_factory_preset_sounds_below_hearing() {
        // **Two octaves below the key is the limit**, which puts C2 at 32 Hz and C3 at 65 Hz — low,
        // and still a pitch. Three octaves put C2 at 16 Hz, which is not a note at all.
        //
        // The rule is about the *lowest audible source*: a preset whose sub is buried under a saw
        // still has the saw to be heard by, and one whose sub is the only voice does not.
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            let below = octaves_below_the_key(&preset);
            assert!(
                below <= 2.0,
                "{name:?} sounds {below} octaves below the key, which puts C2 at {:.1} Hz",
                65.4 / 2f32.powf(below - 1.0)
            );
        }
    }

    #[test]
    fn every_factory_preset_has_something_that_sounds() {
        // A preset with every level at zero loads, reports nothing wrong, and is silent — which
        // reads as the instrument being broken rather than the preset being empty.
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            let v = |id: &str| preset.params.get(id).map_or(0.0, |value| value.v);
            assert!(
                v("sawlevel") > 0.05
                    || v("pulselevel") > 0.05
                    || v("sublevel") > 0.05
                    || v("noiselevel") > 0.05,
                "{name:?} has every source at zero"
            );
            assert!(v("outgain") > 0.05, "{name:?} is turned down to nothing");
        }
    }

    #[test]
    fn no_two_factory_presets_are_the_same_sound() {
        // Fifty is enough that a copied-and-edited design could lose its edit unnoticed.
        for (index, (name, text)) in FACTORY_FILES.iter().enumerate() {
            let a = Preset::parse(text, crate::CLAP_ID).expect("parses");
            for (other, text) in &FACTORY_FILES[index + 1..] {
                let b = Preset::parse(text, crate::CLAP_ID).expect("parses");
                assert_ne!(a.params, b.params, "{name:?} and {other:?} are identical");
            }
        }
    }

    #[test]
    fn every_factory_preset_has_a_category() {
        // A sound is saved with its category (the owner's rule, 2026-09-04), and the factory set
        // is where a person first sees what the categories mean. *Uncategorised* is for files
        // written before the field existed, not for sounds this instrument ships.
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            assert_ne!(
                preset.category,
                Category::Uncategorised,
                "factory preset {name:?} has no category"
            );
        }
    }

    #[test]
    fn every_factory_preset_parses_and_is_for_this_instrument() {
        // A malformed factory preset is a build mistake, not a user's, so it is caught here rather
        // than skipped quietly in the browser.
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID)
                .unwrap_or_else(|e| panic!("factory preset {name:?} does not parse: {e}"));
            assert_eq!(preset.name, *name, "the file's name must match its listing");
        }
    }

    #[test]
    fn every_factory_preset_covers_every_parameter() {
        // The one that catches a factory preset written before a parameter existed: it would load
        // and quietly leave that parameter wherever the last patch left it.
        let params = params();
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            let (_, problems) = preset.resolve(&params);
            assert!(
                problems.is_empty(),
                "factory preset {name:?} is incomplete: {problems:?}"
            );
        }
    }

    #[test]
    fn the_factory_list_begins_with_init() {
        let params = params();
        let all = factory(&params);
        assert_eq!(all[0].name, INIT_NAME);
        assert_eq!(all.len(), FACTORY_FILES.len() + 1);
    }
}
