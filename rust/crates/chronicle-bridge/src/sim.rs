//! 1948 → 2200 in steps. Draw order is part of the contract: it matches the research
//! prototype exactly, so both produce the same world for the same seed.

use std::collections::BTreeSet;

use chronicle_rng::DetRng;
use serde::Serialize;

use crate::config::{BridgeConfig, Weights};
use crate::state::{CivilizationState, SOCIAL};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BridgeEvent {
    pub year: i32,
    /// nuclear_war | climate_crisis | ecological_collapse | conquest | blocs_merged |
    /// earth_unified | space_orbit | space_moon | space_mars | space_interstellar
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BridgeRun {
    pub final_state: CivilizationState,
    pub events: Vec<BridgeEvent>,
    pub flags: BTreeSet<String>,
    /// Indicator snapshots at the start of each step (for charts).
    pub history: Vec<(i32, CivilizationState)>,
}

fn apply(c: &mut CivilizationState, effects: &Weights) {
    for (k, v) in effects {
        c.add(k, *v);
    }
}

pub fn simulate(start: &CivilizationState, seed: u64, cfg: &BridgeConfig) -> BridgeRun {
    let mut c = start.clone();
    let d = &cfg.drift;
    let ev = &cfg.events;
    let mut events = Vec::new();
    let mut flags = BTreeSet::new();
    let mut history = Vec::new();
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut unified = false;
    let mut year = cfg.start_year;
    let push = |events: &mut Vec<BridgeEvent>, year: i32, kind: &str| {
        events.push(BridgeEvent { year, kind: kind.to_string() });
    };

    while year < cfg.end_year {
        history.push((year, c.clone()));
        let mut rng = DetRng::for_stream(seed, &format!("bridge/{year}"));

        let tech = c.get("technology");
        c.add("technology", d.technology * (1.0 - tech));
        let ind = c.get("industrialization");
        c.add("industrialization", d.industrialization * (1.0 - ind));
        let boost = 1.0
            + (if c.blocs > 1 { d.space_race_bonus } else { 0.0 })
            + d.space_cooperation_bonus * c.get("international_cooperation");
        let space = c.get("space_program");
        c.add("space_program", d.space_from_technology * c.get("technology") * boost * (1.0 - space));
        for k in SOCIAL {
            let delta = d.mean_reversion * (0.5 - c.get(k)) + (rng.next_f64() * 2.0 - 1.0) * d.noise;
            c.add(k, delta);
        }
        let target = c.get("militarism") * (1.0 - c.get("international_cooperation")) * d.wars_target_militarism;
        let wars = c.get("global_wars");
        c.add("global_wars", d.wars_relaxation * (target - wars));
        let uni = c.get("planetary_unification");
        c.add(
            "planetary_unification",
            d.unification_from_cooperation * c.get("international_cooperation") * (1.0 - uni),
        );

        let tension = ((c.blocs as f64 - 1.0) / 2.0).min(1.0) * (0.5 + 0.5 * c.get("militarism"));
        let p_nuke =
            ev.nuclear_war.base * c.get("nuclear_weapons") * (1.0 - c.get("international_cooperation")) * tension;
        if rng.chance(p_nuke) {
            apply(&mut c, &ev.nuclear_war.effects);
            push(&mut events, year, "nuclear_war");
            flags.insert("nuclear_war".to_string());
        }

        let cc = &ev.climate_crisis;
        if c.get("industrialization") >= cc.min_industrialization
            && rng.chance(cc.base * c.get("industrialization") * (1.0 - c.get("environmental_policy")))
        {
            let kind = if c.get("environmental_policy") < cc.collapse_below_environment {
                "ecological_collapse"
            } else {
                "climate_crisis"
            };
            apply(&mut c, &cc.effects);
            push(&mut events, year, kind);
            flags.insert(kind.to_string());
        }

        if c.blocs > 1 {
            let q = &ev.conquest;
            let m = &ev.peaceful_merge;
            if c.get("militarism") >= q.min_militarism
                && rng.chance(q.base * c.get("militarism") * (1.0 - c.get("international_cooperation")))
            {
                c.blocs -= 1;
                apply(&mut c, &q.effects);
                push(&mut events, year, "conquest");
                flags.insert("unified_by_conquest".to_string());
            } else if c.get("international_cooperation") >= m.min_cooperation
                && rng.chance(m.base * c.get("international_cooperation"))
            {
                c.blocs -= 1;
                apply(&mut c, &m.effects);
                push(&mut events, year, "blocs_merged");
            }
        }

        for ms in &cfg.space_milestones {
            if !reached.contains(&ms.name) && c.get("space_program") >= ms.at {
                reached.insert(ms.name.clone());
                push(&mut events, year, &format!("space_{}", ms.name));
            }
        }
        if !unified && c.get("planetary_unification") >= ev.unified_at {
            unified = true;
            push(&mut events, year, "earth_unified");
            flags.insert("earth_unified".to_string());
        }
        year += cfg.step_years;
    }
    history.push((year.min(cfg.end_year), c.clone()));
    BridgeRun { final_state: c, events, flags, history }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::presets;

    const SEED: u64 = 45_819_283;

    fn events(name: &str) -> Vec<(i32, String)> {
        let start = presets().into_iter().find(|(n, _)| *n == name).unwrap().1;
        simulate(&start, SEED, BridgeConfig::builtin()).events.into_iter().map(|e| (e.year, e.kind)).collect()
    }

    fn ev(v: &[(i32, &str)]) -> Vec<(i32, String)> {
        v.iter().map(|(y, k)| (*y, k.to_string())).collect()
    }

    /// Same events as tools/research (era_bridge.py) for the same seed.
    #[test]
    fn matches_research_prototype() {
        assert_eq!(
            events("global_democracy"),
            ev(&[(1978, "space_orbit"), (2018, "space_moon"), (2028, "climate_crisis"), (2078, "space_mars"),
                 (2088, "earth_unified"), (2188, "space_interstellar")])
        );
        assert_eq!(
            events("military_dictatorship"),
            ev(&[(1968, "conquest"), (1978, "space_orbit"), (2008, "conquest"), (2028, "climate_crisis"),
                 (2028, "space_moon"), (2108, "space_mars")])
        );
        assert_eq!(
            events("planned_technocracy"),
            ev(&[(1948, "space_orbit"), (1968, "blocs_merged"), (1988, "space_moon"), (2028, "climate_crisis"),
                 (2058, "space_mars"), (2108, "earth_unified"), (2168, "space_interstellar")])
        );
    }

    #[test]
    fn deterministic_and_seed_sensitive() {
        let start = presets()[2].1.clone();
        let cfg = BridgeConfig::builtin();
        let a = simulate(&start, 1, cfg);
        let b = simulate(&start, 1, cfg);
        assert_eq!(a.events, b.events);
        assert_eq!(a.final_state, b.final_state);
        let differs = (2..30).any(|s| simulate(&start, s, cfg).events != a.events);
        assert!(differs, "different seeds should produce different histories");
    }

    #[test]
    fn indicators_stay_in_range() {
        let cfg = BridgeConfig::builtin();
        for (_, p) in presets() {
            for seed in 0..25 {
                let r = simulate(&p, seed, cfg);
                assert!(r.final_state.validate().is_ok());
                assert_eq!(r.history.len(), 27);
            }
        }
    }
}
