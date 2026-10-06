//! mxm-mono-01's routing parameters: one presence and one amount per *(target, source)* pair.
//!
//! `plans/plan-modulation-routing.md` §4.3. The derive needs concrete fields and this instrument's
//! source list is its own, so the struct is declared here rather than generated — which is the shape
//! `mxm-mono-08` already uses for its 120 routing ids. What is shared is everything around these
//! fields: [`mxm_modulation_params`] reads them, and [`mxm_modulation`] evaluates them.
//!
//! # Permanent ids
//!
//! One `#[nested(id_prefix = …)]` per target, so a pair's id is `<target>_<source>` and
//! `<target>_<source>on`. Mechanically derived, never typed, so no instrument's routing ids can
//! drift from another's. **Permanent from here on**, like every id in this collection.

use mxm_modulation_params::Route;
use mxm_modulation_params::reading::{self, Fader, Reach};
use mxm_mono_01_dsp::routing::{
    FULL_SCALE, INIT_PRESENT, KEY_UNIT_SEMITONES, Routing, SOURCE_NAMES, SOURCES, TARGET_NAMES,
    TARGETS, offer, source, target,
};
use nice_plug::prelude::*;

/// Every routing pair's two permanent ids, `(amount, presence)`, in `[target][source]` order.
///
/// **Written out rather than derived at runtime**, because a preset's parameter list is
/// `&'static str` and because these are permanent ids: they belong in the source where they can be
/// read, grepped and diffed, exactly as `mxm-mono-08` writes out its own. They are still generated
/// by the `#[nested(id_prefix = …)]` groups in [`Routes`] — this table only names what the derive
/// produces, and `tests::the_id_table_is_what_the_derive_actually_produces` is what holds the two
/// together.
pub const ROUTE_IDS: [[(&str, &str); SOURCES]; TARGETS] = [
    [
        ("mod_pitch_lfo", "mod_pitch_lfoon"),
        ("mod_pitch_env", "mod_pitch_envon"),
        ("mod_pitch_key", "mod_pitch_keyon"),
        ("mod_pitch_vel", "mod_pitch_velon"),
        ("mod_pitch_wheel", "mod_pitch_wheelon"),
        ("mod_pitch_press", "mod_pitch_presson"),
        ("mod_pitch_bend", "mod_pitch_bendon"),
        ("mod_pitch_saw", "mod_pitch_sawon"),
        ("mod_pitch_pulse", "mod_pitch_pulseon"),
        ("mod_pitch_sub", "mod_pitch_subon"),
        ("mod_pitch_noise", "mod_pitch_noiseon"),
    ],
    [
        ("mod_width_lfo", "mod_width_lfoon"),
        ("mod_width_env", "mod_width_envon"),
        ("mod_width_key", "mod_width_keyon"),
        ("mod_width_vel", "mod_width_velon"),
        ("mod_width_wheel", "mod_width_wheelon"),
        ("mod_width_press", "mod_width_presson"),
        ("mod_width_bend", "mod_width_bendon"),
        ("mod_width_saw", "mod_width_sawon"),
        ("mod_width_pulse", "mod_width_pulseon"),
        ("mod_width_sub", "mod_width_subon"),
        ("mod_width_noise", "mod_width_noiseon"),
    ],
    [
        ("mod_cutoff_lfo", "mod_cutoff_lfoon"),
        ("mod_cutoff_env", "mod_cutoff_envon"),
        ("mod_cutoff_key", "mod_cutoff_keyon"),
        ("mod_cutoff_vel", "mod_cutoff_velon"),
        ("mod_cutoff_wheel", "mod_cutoff_wheelon"),
        ("mod_cutoff_press", "mod_cutoff_presson"),
        ("mod_cutoff_bend", "mod_cutoff_bendon"),
        ("mod_cutoff_saw", "mod_cutoff_sawon"),
        ("mod_cutoff_pulse", "mod_cutoff_pulseon"),
        ("mod_cutoff_sub", "mod_cutoff_subon"),
        ("mod_cutoff_noise", "mod_cutoff_noiseon"),
    ],
    [
        ("mod_amp_lfo", "mod_amp_lfoon"),
        ("mod_amp_env", "mod_amp_envon"),
        ("mod_amp_key", "mod_amp_keyon"),
        ("mod_amp_vel", "mod_amp_velon"),
        ("mod_amp_wheel", "mod_amp_wheelon"),
        ("mod_amp_press", "mod_amp_presson"),
        ("mod_amp_bend", "mod_amp_bendon"),
        ("mod_amp_saw", "mod_amp_sawon"),
        ("mod_amp_pulse", "mod_amp_pulseon"),
        ("mod_amp_sub", "mod_amp_subon"),
        ("mod_amp_noise", "mod_amp_noiseon"),
    ],
];

/// One target's routes: a presence and a signed amount for every source the instrument declares.
///
/// **Presence is the enable and the amount is the depth**, and nothing else. No selector, because a
/// pair *is* its source; no polarity switch, because the amount is signed; no "via" modifier,
/// because a gesture scaling a route is a multiplier module rather than a third parameter here.
#[derive(Params)]
pub struct TargetRoutes {
    #[id = "lfoon"]
    pub lfo_on: BoolParam,
    #[id = "lfo"]
    pub lfo: FloatParam,
    #[id = "envon"]
    pub env_on: BoolParam,
    #[id = "env"]
    pub env: FloatParam,
    #[id = "keyon"]
    pub key_on: BoolParam,
    #[id = "key"]
    pub key: FloatParam,
    #[id = "velon"]
    pub vel_on: BoolParam,
    #[id = "vel"]
    pub vel: FloatParam,
    #[id = "wheelon"]
    pub wheel_on: BoolParam,
    #[id = "wheel"]
    pub wheel: FloatParam,
    #[id = "presson"]
    pub press_on: BoolParam,
    #[id = "press"]
    pub press: FloatParam,
    #[id = "bendon"]
    pub bend_on: BoolParam,
    #[id = "bend"]
    pub bend: FloatParam,
    #[id = "sawon"]
    pub saw_on: BoolParam,
    #[id = "saw"]
    pub saw: FloatParam,
    #[id = "pulseon"]
    pub pulse_on: BoolParam,
    #[id = "pulse"]
    pub pulse: FloatParam,
    #[id = "subon"]
    pub sub_on: BoolParam,
    #[id = "sub"]
    pub sub: FloatParam,
    #[id = "noiseon"]
    pub noise_on: BoolParam,
    #[id = "noise"]
    pub noise: FloatParam,
}

/// A route amount: signed, centred on zero, and **an amount, so it starts there** — the
/// collection's one route parameter (`mxm_modulation_params::reading`), on the travel the pair's
/// offer allows, which on this instrument is always both halves.
///
/// Smoothed, because it multiplies a signal — `plugins/AGENTS.md`'s *smooth signals, not
/// coefficients*. Drawn bipolar, because its centre is *no modulation* and a half-filled track would
/// read as "half on" when it means "off". It reads as [`reach`] says.
fn amount(target: usize, source: usize) -> FloatParam {
    reading::amount_param(
        format!("{} from {}", TARGET_NAMES[target], SOURCE_NAMES[source]),
        reach(target, source),
        Fader::for_offer(offer(target, source), false),
        15.0,
    )
}

/// What a route reads: **what its pair delivers at this amount, in the target's own unit** —
/// semitones, a percentage of width, octaves, a percentage of the amplifier's level — and per octave
/// of keyboard for a Key route. So the machine's own routes read, at full, the numbers their retired
/// knobs meant: +7.00 st, +45 %, +6.00 and +4.00 oct, and +1.00 oct/oct; and a route the SH-101
/// never had reads the collection's standard reach, +12.00 st or +4.00 oct.
///
/// `FULL_SCALE` is per route rather than per target, so a bare percentage of the amount said `100`
/// for six octaves of filter envelope and for four of filter LFO — two rows meaning different octaves
/// at the same number, which `plans/plan-modulation-routing.md` decision 1.11 answers with the
/// target's unit.
fn reach(target: usize, source: usize) -> Reach {
    let unit = match target {
        target::PITCH => reading::SEMITONES,
        target::CUTOFF => reading::OCTAVES,
        _ => reading::PERCENT,
    };
    let full = FULL_SCALE[target][source];
    if source == source::KEY {
        Reach::per_octave(full * 12.0 / KEY_UNIT_SEMITONES, unit)
    } else {
        Reach::new(full, unit)
    }
}

/// Whether a route exists. **Configuration, not an amount**, so its default is the machine's own
/// wiring: the five paths the SH-101 itself has are present in the init patch, and everything else
/// is absent until a player asks for it.
///
/// That is the owner's first sentence — *the sources in the init patch should be the ones that
/// follow the original signal flow* — and it is what `plugins/AGENTS.md` already asks for. It is
/// safe because the depth beside it is an **amount** and starts at zero, so a revealed route is
/// audible as nothing and is one gesture from being the machine.
fn present(target: &str, source: &str, wired: bool) -> BoolParam {
    BoolParam::new(format!("{target} from {source} on"), wired)
}

impl TargetRoutes {
    /// Every pair for one target, at the init patch: the machine's own wiring present, the rest
    /// absent, and **every one of them at zero depth**.
    pub fn new(target: usize) -> Self {
        let n = SOURCE_NAMES;
        let wired = |s: usize| INIT_PRESENT.contains(&(target, s));
        let t = target;
        let target = TARGET_NAMES[t];
        Self {
            lfo_on: present(target, n[0], wired(0)),
            lfo: amount(t, 0),
            env_on: present(target, n[1], wired(1)),
            env: amount(t, 1),
            key_on: present(target, n[2], wired(2)),
            key: amount(t, 2),
            vel_on: present(target, n[3], wired(3)),
            vel: amount(t, 3),
            wheel_on: present(target, n[4], wired(4)),
            wheel: amount(t, 4),
            press_on: present(target, n[5], wired(5)),
            press: amount(t, 5),
            bend_on: present(target, n[6], wired(6)),
            bend: amount(t, 6),
            saw_on: present(target, n[7], wired(7)),
            saw: amount(t, 7),
            pulse_on: present(target, n[8], wired(8)),
            pulse: amount(t, 8),
            sub_on: present(target, n[9], wired(9)),
            sub: amount(t, 9),
            noise_on: present(target, n[10], wired(10)),
            noise: amount(t, 10),
        }
    }

    /// The pairs in **declared source order**, which is the order the frame and the interface use.
    /// This target's routes. **`target` is its index**, needed because each row's keyboard
    /// scope is its parameter's permanent id and those live in [`ROUTE_IDS`], keyed by target.
    pub fn routes(&self, target: usize) -> [Route<'_>; SOURCES] {
        [
            Route {
                source: SOURCE_NAMES[0],
                present: &self.lfo_on,
                amount: &self.lfo,
                present_id: ROUTE_IDS[target][0].1,
                amount_id: ROUTE_IDS[target][0].0,
            },
            Route {
                source: SOURCE_NAMES[1],
                present: &self.env_on,
                amount: &self.env,
                present_id: ROUTE_IDS[target][1].1,
                amount_id: ROUTE_IDS[target][1].0,
            },
            Route {
                source: SOURCE_NAMES[2],
                present: &self.key_on,
                amount: &self.key,
                present_id: ROUTE_IDS[target][2].1,
                amount_id: ROUTE_IDS[target][2].0,
            },
            Route {
                source: SOURCE_NAMES[3],
                present: &self.vel_on,
                amount: &self.vel,
                present_id: ROUTE_IDS[target][3].1,
                amount_id: ROUTE_IDS[target][3].0,
            },
            Route {
                source: SOURCE_NAMES[4],
                present: &self.wheel_on,
                amount: &self.wheel,
                present_id: ROUTE_IDS[target][4].1,
                amount_id: ROUTE_IDS[target][4].0,
            },
            Route {
                source: SOURCE_NAMES[5],
                present: &self.press_on,
                amount: &self.press,
                present_id: ROUTE_IDS[target][5].1,
                amount_id: ROUTE_IDS[target][5].0,
            },
            Route {
                source: SOURCE_NAMES[6],
                present: &self.bend_on,
                amount: &self.bend,
                present_id: ROUTE_IDS[target][6].1,
                amount_id: ROUTE_IDS[target][6].0,
            },
            Route {
                source: SOURCE_NAMES[7],
                present: &self.saw_on,
                amount: &self.saw,
                present_id: ROUTE_IDS[target][7].1,
                amount_id: ROUTE_IDS[target][7].0,
            },
            Route {
                source: SOURCE_NAMES[8],
                present: &self.pulse_on,
                amount: &self.pulse,
                present_id: ROUTE_IDS[target][8].1,
                amount_id: ROUTE_IDS[target][8].0,
            },
            Route {
                source: SOURCE_NAMES[9],
                present: &self.sub_on,
                amount: &self.sub,
                present_id: ROUTE_IDS[target][9].1,
                amount_id: ROUTE_IDS[target][9].0,
            },
            Route {
                source: SOURCE_NAMES[10],
                present: &self.noise_on,
                amount: &self.noise,
                present_id: ROUTE_IDS[target][10].1,
                amount_id: ROUTE_IDS[target][10].0,
            },
        ]
    }

    /// Whether each of this target's routes exists. Read **once per interval**, never per sample.
    pub fn presences(&self, target: usize) -> [bool; SOURCES] {
        mxm_modulation_params::presences(&self.routes(target))
    }

    /// One source's amount parameter, by index, in **declared source order**.
    ///
    /// A `match` rather than an array of references, and the reason is the cost gate: building an
    /// eleven-element array of `&FloatParam` cost forty-four pointer stores per sample across the
    /// four targets, for five routes that were actually live. This is branched past for a source
    /// nothing reads.
    #[inline]
    fn amount_param(&self, source: usize) -> &FloatParam {
        match source {
            0 => &self.lfo,
            1 => &self.env,
            2 => &self.key,
            3 => &self.vel,
            4 => &self.wheel,
            5 => &self.press,
            6 => &self.bend,
            7 => &self.saw,
            8 => &self.pulse,
            9 => &self.sub,
            _ => &self.noise,
        }
    }

    /// Snaps a newly present route's smoother to its stored value.
    ///
    /// **An absent route's smoother is not advanced, so it must not be resumed either.** While the
    /// pair was absent nothing called `next()`, but the parameter itself stayed editable: a host
    /// automating it, or a preset load, moves the *target* and leaves the smoother's current value
    /// wherever the last live sample left it. Resuming from there ramps the route in from a stale
    /// number over a span that depends on how long it was absent — the accumulation across skipped
    /// spans `plans/plan-modulation-routing.md` §6.2 forbids, because it makes a route's first
    /// audible value depend on the host's buffer sizes.
    ///
    /// Called from [`Routes::topology_from`] for the pairs that just became present, so a re-added
    /// route arrives at the depth the player last set rather than sliding up to it.
    pub fn arm(&self, newly_present: &[bool; SOURCES]) {
        for (source, &now) in newly_present.iter().enumerate() {
            if now {
                let param = self.amount_param(source);
                param.smoothed.reset(param.value());
            }
        }
    }

    /// Fills this target's amounts for the sample about to be rendered, as **fractions of each
    /// route's full scale** — the scale itself is applied by the sum, not here.
    ///
    /// `(amount × source) × scale` is the instruction sequence this voice executed before it had
    /// routing, and folding the scale in here instead would move every pinned digest for one saved
    /// instruction. `plans/plan-modulation-routing.md` §6.2 says so, and an earlier revision of this
    /// file did it anyway.
    ///
    /// **Only a live route's smoother is advanced**, which is the whole of the efficiency design: an
    /// absent pair costs nothing per sample beyond the branch that skips it, and its stored depth is
    /// left exactly where the player put it so re-adding the source restores it. An absent slot in
    /// `out` is left holding whatever it held — never cleared, because nothing reads it: the
    /// compacted list is what decides which slots the sum visits.
    #[inline]
    pub fn advance_into(&self, live: &[bool; SOURCES], out: &mut [f32; SOURCES]) {
        for (source, &on) in live.iter().enumerate() {
            if on {
                out[source] = self.amount_param(source).smoothed.next();
            }
        }
    }
}

/// All four targets' routes.
#[derive(Params)]
pub struct Routes {
    #[nested(id_prefix = "mod_pitch", group = "Modulation - Pitch")]
    pub pitch: TargetRoutes,
    #[nested(id_prefix = "mod_width", group = "Modulation - Pulse width")]
    pub width: TargetRoutes,
    #[nested(id_prefix = "mod_cutoff", group = "Modulation - Cutoff")]
    pub cutoff: TargetRoutes,
    #[nested(id_prefix = "mod_amp", group = "Modulation - Amplitude")]
    pub amp: TargetRoutes,
}

impl Default for Routes {
    fn default() -> Self {
        Self::new()
    }
}

impl Routes {
    /// The init patch: **the machine's own five routes present, every depth at zero.**
    ///
    /// `filterenv`, `filterlfo`, `keytrack`, `vcolfo` and the PWM path *are* these routes now. They
    /// were five knobs and a source switch, and the owner's answer on seeing the pilot was that they
    /// were no longer necessary and should just be init settings. So a fresh instance shows the
    /// SH-101's own signal flow, one row per path, each at zero; a player can turn one up, invert
    /// it, take it off, or put something the machine never had in its place.
    ///
    /// It is the same sound it always was, because a route at zero depth adds exactly nothing, and
    /// a route at depth `d` reaches exactly what the knob at `d` reached —
    /// `crates/mxm-mono-01-dsp/tests/equivalence.rs` is where both are held.
    pub fn new() -> Self {
        Self {
            pitch: TargetRoutes::new(target::PITCH),
            width: TargetRoutes::new(target::PULSE_WIDTH),
            cutoff: TargetRoutes::new(target::CUTOFF),
            amp: TargetRoutes::new(target::AMPLITUDE),
        }
    }

    /// The four targets, in declared target order.
    pub fn each(&self) -> [&TargetRoutes; TARGETS] {
        [&self.pitch, &self.width, &self.cutoff, &self.amp]
    }

    /// Which routes are live, for the whole instrument. Once per interval.
    pub fn topology(&self) -> Routing {
        let mut routing = Routing::new();
        for (index, (slot, group)) in routing.present.iter_mut().zip(self.each()).enumerate() {
            *slot = group.presences(index);
        }
        routing.compact();
        routing
    }

    /// The topology for this interval, with every **newly present** route's smoother snapped to
    /// its stored value.
    ///
    /// `previous` is the topology the last interval ran, so the caller keeps it across buffers.
    /// See [`TargetRoutes::arm`] for why resuming a skipped smoother is wrong.
    pub fn topology_from(&self, previous: &Routing) -> Routing {
        let routing = self.topology();
        for (index, group) in self.each().into_iter().enumerate() {
            let mut newly = [false; SOURCES];
            for (slot, (&now, &before)) in newly.iter_mut().zip(
                routing.present[index]
                    .iter()
                    .zip(previous.present[index].iter()),
            ) {
                *slot = now && !before;
            }
            group.arm(&newly);
        }
        routing
    }

    /// Fills this sample's amounts into an already-topologised [`Routing`].
    ///
    /// `wheel` is the mod wheel, and it is here rather than in a route because of what it does:
    /// **it scales the LFO-to-pitch depth**, which is the machine's own wheel path and is not a
    /// route into pitch at all. Routing the wheel to pitch gives a second bender; what the SH-101
    /// does is deepen the vibrato that is already there. `plans/plan-modulation-routing.md` §5.2
    /// keeps it as a named legacy path — the raw wheel is a routable source beside it, and the
    /// native way to say the same thing is a multiplier module this instrument does not have yet.
    ///
    /// It used to be added to `vcolfo` inside the plugin's per-sample patch. It is added to the pair
    /// that replaced `vcolfo`, which is the same arithmetic in the route's own home, and at a wheel
    /// of zero it is exactly the stored depth. **It adds, whichever way the route points**: the lever's vibrato is its own depth
    /// summed with the route's, as the machine's slider and lever depths summed, so a negative route is
    /// partly cancelled rather than deepened. Taking the route's sign instead would step the depth by
    /// twice the push as a swept amount crossed zero.
    pub fn advance(&self, routing: &mut Routing, wheel: f32) {
        let targets = self.each();
        for i in 0..routing.live().len() {
            let (t, source) = routing.live()[i];
            routing.amounts[t as usize][source as usize] = targets[t as usize]
                .amount_param(source as usize)
                .smoothed
                .next();
        }
        let vibrato = &mut routing.amounts[target::PITCH][source::LFO];
        *vibrato = (*vibrato + wheel).clamp(-1.0, 1.0);
    }
}

impl Routes {
    /// Every routing parameter, named by its permanent id, for the preset layer.
    ///
    /// **Presets carry routing.** They did not when the routing was new and every route was absent
    /// at init, and nothing noticed — which stopped being harmless the moment the machine's own
    /// modulation became routes: a preset that did not name them would load a sound with somebody
    /// else's filter envelope still in it. `plugins/AGENTS.md`'s coverage rule is what catches that,
    /// and it can only catch it if these are in the list.
    pub fn parameters(&self) -> Vec<(&'static str, &dyn mxm_preset::ErasedParam)> {
        let mut out = Vec::with_capacity(TARGETS * SOURCES * 2);
        for (index, (group, ids)) in self.each().into_iter().zip(ROUTE_IDS).enumerate() {
            for (route, (amount, presence)) in group.routes(index).into_iter().zip(ids) {
                out.push((amount, route.amount));
                out.push((presence, route.present));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::Params;

    /// [`ROUTE_IDS`] names exactly what the derive produces, and nothing else.
    ///
    /// The table is written out so a permanent id can be read in the source; this is what stops it
    /// becoming a second, quietly wrong opinion about what those ids are.
    #[test]
    fn the_id_table_is_what_the_derive_actually_produces() {
        let params = crate::params::MxmMono01Params::default();
        let real: std::collections::BTreeSet<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .filter(|id| id.starts_with("mod_"))
            .collect();

        let named: std::collections::BTreeSet<String> = ROUTE_IDS
            .iter()
            .flatten()
            .flat_map(|(amount, presence)| [(*amount).to_owned(), (*presence).to_owned()])
            .collect();

        assert_eq!(named, real, "ROUTE_IDS has drifted from the derived ids");
        assert_eq!(named.len(), TARGETS * SOURCES * 2);
    }

    /// **Remove, edit while absent, re-add: the route arrives at the depth the player set.**
    ///
    /// An absent route's smoother is never advanced, so it must not be resumed either. While the
    /// pair is absent the parameter itself stays editable — a host automating it, or a preset load
    /// — and that moves the *target* while the smoother's current value stays wherever the last
    /// live sample left it. Resuming would ramp the route in from that stale number over a span set
    /// by how long it was absent.
    ///
    /// **Falsified before trusted**: with `arm`'s body removed, the first sample after re-adding
    /// reads `0.898` — a point on the ramp down from `0.9` — instead of the stored `-0.4`.
    #[test]
    fn a_re_added_route_arrives_at_its_stored_depth_rather_than_ramping_from_a_stale_one() {
        use nice_plug::params::InternalParamMut;

        let routes = Routes::new();
        assert!(
            !routes.cutoff.saw_on.value(),
            "this pair starts absent, which is what the test needs"
        );

        // Present it, and let it settle at one depth. `_internal_update_smoother(_, false)` is what
        // the wrapper calls after every host parameter change, and it moves the *target* only.
        const RATE: f32 = 48_000.0;
        unsafe {
            routes.cutoff.saw_on._internal_set_plain_value(true);
            routes.cutoff.saw._internal_set_plain_value(0.9);
            routes.cutoff.saw._internal_update_smoother(RATE, true);
        }
        let routing = routes.topology_from(&Routing::new());
        let mut amounts = routing;
        routes.advance(&mut amounts, 0.0);
        assert_eq!(amounts.amounts[target::CUTOFF][source::SAW], 0.9);

        // Remove it, then edit the depth while it is absent.
        unsafe {
            routes.cutoff.saw_on._internal_set_plain_value(false);
        }
        let absent = routes.topology_from(&routing);
        unsafe {
            // Nothing advances the smoother while the pair is absent, so its current value is still
            // 0.9 and it is now mid-ramp toward the new target.
            routes.cutoff.saw._internal_set_plain_value(-0.4);
            routes.cutoff.saw._internal_update_smoother(RATE, false);
        }
        assert!(
            routes.cutoff.saw.smoothed.is_smoothing(),
            "the edit must leave the smoother mid-ramp, or this proves nothing"
        );

        // Re-add it. The very first sample must be the stored depth, not a ramp from 0.9.
        unsafe {
            routes.cutoff.saw_on._internal_set_plain_value(true);
        }
        let mut back = routes.topology_from(&absent);
        routes.advance(&mut back, 0.0);
        assert_eq!(
            back.amounts[target::CUTOFF][source::SAW],
            -0.4,
            "a re-added route must arrive at its stored depth"
        );
    }

    /// **The wheel's push adds, whichever way the route points**, so the depth is continuous as a swept
    /// amount crosses zero. A push that took the route's sign — tried on 2026-09-15, after a review read
    /// *deepens* as *further from zero* — stepped the depth by twice the push at the crossing, and the
    /// next review caught it.
    #[test]
    fn the_wheel_push_adds_the_same_whichever_way_the_route_points() {
        use nice_plug::params::InternalParamMut;

        let routes = Routes::new();
        let depth_at = |amount: f32| {
            unsafe {
                routes.pitch.lfo._internal_set_plain_value(amount);
                routes.pitch.lfo._internal_update_smoother(48_000.0, true);
            }
            let stored = routes.pitch.lfo.value();
            let mut routing = routes.topology_from(&Routing::new());
            routes.advance(&mut routing, 0.5);
            (stored, routing.amounts[target::PITCH][source::LFO])
        };
        for amount in [-0.3, -0.001, 0.0, 0.001, 0.3] {
            let (stored, depth) = depth_at(amount);
            assert_eq!(
                depth,
                stored + 0.5,
                "the push at a stored depth of {stored}"
            );
        }
    }

    /// **A control-map role may take a route's amount only where Init wires that route.**
    ///
    /// An absent pair contributes nothing whatever its amount holds, so a controller knob bound to
    /// one would be dead on a fresh instance. A role bound to a *presence* is never dead, because
    /// turning it on is what creates the route. `mxm-mono-00`'s check of the same name is the shape.
    ///
    /// **Falsified before trusted**: with `osc1.pwm_depth` pointed at `mod_width_env`, a route the
    /// init patch leaves absent, this fails naming that id.
    #[test]
    fn a_control_map_role_never_points_at_a_dead_route() {
        let text = include_str!("../control-map.json");
        let params = crate::params::MxmMono01Params::default();

        let mut named = 0;
        for (t, group) in params.routes.each().into_iter().enumerate() {
            for (route, (amount, presence)) in group.routes(t).into_iter().zip(ROUTE_IDS[t]) {
                if text.contains(&format!("\"{presence}\"")) {
                    named += 1;
                }
                if text.contains(&format!("\"{amount}\"")) {
                    named += 1;
                    assert!(
                        route.is_present(),
                        "the control map binds a role to {amount}, which Init does not wire: a \
                         knob on it would do nothing"
                    );
                }
            }
        }
        // **Five ids, six roles**: `filter.lfo_amount` and `lfo1.to_filter` both name the filter's
        // LFO route, which is the same control reached two ways.
        assert_eq!(
            named, 5,
            "the map names {named} routing ids; it named five when this was written, so either the \
             map or this check has drifted"
        );
    }

    /// **A route reads what its pair delivers**, in the target's own unit — the machine's own routes
    /// at full read the numbers their retired knobs meant — and a reading typed back in lands on the
    /// amount it came from.
    ///
    /// A bare percentage of the amount said `100` for six octaves of filter envelope and for four of
    /// filter LFO, and for seven semitones of vibrato: nothing false and nothing a player could use
    /// (`plans/plan-modulation-routing.md` decision 1.11). Each full-scale reading is built from the
    /// DSP's own constant, so a scale that moves moves the reading with it.
    #[test]
    fn a_route_reads_what_its_pair_delivers_and_reads_back() {
        use mxm_mono_01_dsp::voice::{
            FILTER_ENV_OCTAVES, FILTER_LFO_OCTAVES, PWM_WIDTH_SWING, VCO_LFO_SEMITONES,
        };
        use mxm_preset::ErasedParam;
        let r = Routes::new();
        let cases: [(&FloatParam, f32, String); 15] = [
            (&r.pitch.lfo, 1.0, format!("{VCO_LFO_SEMITONES:+.2} st")),
            (
                &r.width.lfo,
                1.0,
                format!("{:+.0} %", PWM_WIDTH_SWING * 100.0),
            ),
            (
                &r.width.env,
                0.0,
                format!("{:+.0} %", -PWM_WIDTH_SWING * 100.0),
            ),
            (&r.cutoff.env, 1.0, format!("{FILTER_ENV_OCTAVES:+.2} oct")),
            (
                &r.cutoff.env,
                0.0,
                format!("{:+.2} oct", -FILTER_ENV_OCTAVES),
            ),
            (&r.cutoff.env, 0.5, "+0.00 oct".to_owned()),
            (&r.cutoff.lfo, 1.0, format!("{FILTER_LFO_OCTAVES:+.2} oct")),
            // The key source spans five octaves either side of middle C, so one octave of cutoff
            // per octave of keyboard — `keytrack` at full.
            (&r.cutoff.key, 1.0, "+1.00 oct/oct".to_owned()),
            (&r.amp.lfo, 1.0, "+100 %".to_owned()),
            // Routes the SH-101 never had, at the collection's standard reach.
            (&r.pitch.key, 1.0, "+12.00 st/oct".to_owned()),
            (&r.pitch.wheel, 1.0, "+12.00 st".to_owned()),
            (&r.pitch.env, 0.0, "-12.00 st".to_owned()),
            (&r.cutoff.vel, 1.0, "+4.00 oct".to_owned()),
            (&r.amp.vel, 1.0, "+100 %".to_owned()),
            (&r.amp.key, 1.0, "+20 %/oct".to_owned()),
        ];
        for (param, normalised, expected) in cases {
            let text = ErasedParam::format(param, normalised);
            assert_eq!(
                text,
                expected,
                "{} at {normalised}",
                ErasedParam::name(param)
            );
            let back = param
                .string_to_normalized_value(&text)
                .expect("the reading parses");
            assert!(
                (back - normalised).abs() < 1e-3,
                "{}: {text} read back as {back}",
                ErasedParam::name(param)
            );
        }
    }

    use mxm_plugin_test::routing_checks;

    /// **Every route parameter says what the DSP does** — the modulation standard's plugin half:
    /// each pair's travel is its offer's, its reading carries its target's unit and states what
    /// `mxm_mono_01_dsp::conformance` measures the voice's own graph delivering, and every reading
    /// survives the host's round trip.
    ///
    /// Falsified before trusted: with the pitch reading's reach left at the LFO's seven semitones,
    /// it names every added pitch pair from a performance source.
    #[test]
    fn every_route_parameter_says_what_the_dsp_does() {
        let routes = Routes::new();
        let groups = routes.each();
        if let Err(failures) =
            routing_checks::amounts(&mxm_mono_01_dsp::conformance::Declared, |target, source| {
                Some(groups[target].amount_param(source))
            })
        {
            panic!(
                "{} failure(s):
{}",
                failures.len(),
                failures.join(
                    "
"
                )
            );
        }
    }

    /// **Every reading survives the host's round trip, a rounded zero included**: printed, parsed and
    /// printed again, it is the same text — at amounts either side of zero, not only the round numbers.
    /// The bare percentage printed `-0` there, which parses to zero and prints `0`, and
    /// `clap-validator`'s `param-conversions` fails on that whenever its values land in the sliver
    /// (`mxm_modulation_params::signed`).
    #[test]
    fn every_reading_survives_the_hosts_round_trip_a_rounded_zero_included() {
        let routes = Routes::new();
        let check = |param: &FloatParam| {
            for normalised in [
                0.0f32, 0.25, 0.4999, 0.49999, 0.5, 0.50001, 0.5001, 0.75, 1.0,
            ] {
                let text = param.normalized_value_to_string(normalised, true);
                let back = param
                    .string_to_normalized_value(&text)
                    .unwrap_or_else(|| panic!("{}: {text} does not parse", param.name()));
                assert_eq!(
                    text,
                    param.normalized_value_to_string(back, true),
                    "{} at {normalised}",
                    param.name()
                );
            }
        };
        for group in routes.each() {
            for source in 0..SOURCES {
                check(group.amount_param(source));
            }
        }
    }
}
