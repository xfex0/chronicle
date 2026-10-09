//! Final civilization → Stellaris empire design, with reasons and confidence per decision.

use std::collections::{BTreeMap, BTreeSet};

use chronicle_core::{ConfidenceComponents, ConfidencePolicy};
use serde::Serialize;

use crate::config::{BridgeConfig, Vocabulary, Weights};
use crate::state::{CivilizationState, Ideology};

/// Localisable explanation: the UI translates `code` and fills in `value` / `detail`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reason {
    pub code: String,
    pub value: Option<f64>,
    pub detail: Option<String>,
}

fn reason(code: &str, value: Option<f64>, detail: Option<&str>) -> Reason {
    Reason { code: code.to_string(), value, detail: detail.map(str::to_string) }
}

#[derive(Debug, Clone, Serialize)]
pub struct Decision {
    /// authority | ethics | civics | origin
    pub part: String,
    pub choice: Vec<String>,
    pub reasons: Vec<Reason>,
    /// Runner-up when the margin was small (shown as "close call").
    pub alternative: Option<String>,
    pub components: ConfidenceComponents,
    pub confidence: f64,
    /// auto | warning | pending_review under the default policy (the UI re-applies the campaign policy).
    pub review_state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmpireDesign {
    pub authority: String,
    /// e.g. ["fanatic_materialist", "xenophile"] or three single ethics; always 3 points.
    pub ethics: Vec<String>,
    pub civics: Vec<String>,
    pub origin: String,
    /// Axis scores in [-1, 1] (positive = first ethic of the axis name).
    pub axes: BTreeMap<String, f64>,
    pub decisions: Vec<Decision>,
    /// True when the vocabulary keys are confirmed against the game files.
    pub vocabulary_verified: bool,
}

impl EmpireDesign {
    pub fn ethic_points(&self) -> u32 {
        self.ethics.iter().map(|e| if e.starts_with("fanatic_") { 2 } else { 1 }).sum()
    }

    pub fn base_ethics(&self) -> BTreeSet<String> {
        self.ethics.iter().map(|e| e.trim_start_matches("fanatic_").to_string()).collect()
    }

    /// Close the validation gate on every decision (called after `validate_design`).
    pub fn set_validation(&mut self, passed: bool) {
        let policy = ConfidencePolicy::default();
        for d in &mut self.decisions {
            d.components.validation_passed = passed;
            d.confidence = d.components.confidence();
            d.review_state = policy.decide(&d.components).review_state().to_string();
        }
    }
}

fn weighted(w: &Weights, c: &CivilizationState) -> f64 {
    w.iter().fold(0.0, |acc, (k, v)| acc + v * if k == "constant" { 1.0 } else { c.get(k) })
}

/// (axis name, positive ethic, negative ethic)
const AXES: [(&str, &str, &str); 4] = [
    ("authoritarian_egalitarian", "authoritarian", "egalitarian"),
    ("xenophobe_xenophile", "xenophobe", "xenophile"),
    ("militarist_pacifist", "militarist", "pacifist"),
    ("spiritualist_materialist", "spiritualist", "materialist"),
];

fn decision(part: &str, choice: Vec<String>, reasons: Vec<Reason>, alternative: Option<String>, coverage: (usize, usize), reliability: f64) -> Decision {
    let components = ConfidenceComponents::new(coverage.0, coverage.1, reliability, true, true);
    let policy = ConfidencePolicy::default();
    Decision {
        part: part.to_string(),
        choice,
        reasons,
        alternative,
        confidence: components.confidence(),
        review_state: policy.decide(&components).review_state().to_string(),
        components,
    }
}

pub fn design_empire(
    c: &CivilizationState,
    flags: &BTreeSet<String>,
    cfg: &BridgeConfig,
    vocab: &Vocabulary,
    coverage: (usize, usize),
) -> EmpireDesign {
    let s = &cfg.stellaris;
    let rel = &s.mapping_reliability;
    let mut decisions = Vec::new();

    // ---- ethics: exactly 3 points
    let scores: BTreeMap<String, f64> = AXES
        .iter()
        .map(|(name, _, _)| (name.to_string(), weighted(&cfg.axes[*name], c).clamp(-1.0, 1.0)))
        .collect();
    let mut ranked: Vec<&(&str, &str, &str)> = AXES.iter().collect();
    ranked.sort_by(|a, b| {
        scores[b.0].abs().partial_cmp(&scores[a.0].abs()).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(b.0))
    });
    let pick = |a: &(&str, &str, &str)| -> String {
        if scores[a.0] >= 0.0 { a.1.to_string() } else { a.2.to_string() }
    };
    let fanatic = scores[ranked[0].0].abs() >= s.fanatic_threshold;
    let ethics: Vec<String> = if fanatic {
        vec![format!("fanatic_{}", pick(ranked[0])), pick(ranked[1])]
    } else {
        ranked[..3].iter().map(|a| pick(*a)).collect()
    };
    let used = if fanatic { 2 } else { 3 };
    let mut ethic_reasons: Vec<Reason> =
        ranked[..used].iter().map(|a| reason("ethic.axis", Some(scores[a.0]), Some(a.0))).collect();
    if fanatic {
        ethic_reasons.insert(0, reason("ethic.fanatic", Some(scores[ranked[0].0]), Some(ranked[0].0)));
    }
    let margin = scores[ranked[used - 1].0].abs() - scores[ranked[used].0].abs();
    let ethic_alt = (margin < s.close_call_margin).then(|| pick(ranked[used]));
    let base: BTreeSet<String> = ethics.iter().map(|e| e.trim_start_matches("fanatic_").to_string()).collect();

    // ---- authority
    let a = c.get("authoritarianism");
    let mut authority = if c.ideology == Ideology::Monarchy && a >= s.imperial_min {
        "imperial"
    } else if 1.0 - a >= s.democratic_min {
        "democratic"
    } else if a >= s.dictatorial_min {
        "dictatorial"
    } else {
        "oligarchic"
    }
    .to_string();
    let mut auth_reasons = vec![reason(&format!("authority.{authority}"), Some(a), None)];
    let forbidden: BTreeSet<String> = vocab.authorities[&authority]
        .forbids_ethics
        .iter()
        .map(|e| e.trim_start_matches("fanatic_").to_string())
        .collect();
    if base.intersection(&forbidden).next().is_some() {
        auth_reasons.push(reason("authority.adjusted_for_ethics", None, Some(&authority)));
        authority = "oligarchic".to_string();
    }

    // ---- civics: best two allowed
    let allowed = |key: &str| -> bool {
        let Some(cv) = vocab.civics.get(key) else { return false };
        if !cv.authorities.is_empty() && !cv.authorities.contains(&authority) {
            return false;
        }
        if cv.requires_ethics.iter().any(|r| !base.contains(r)) {
            return false;
        }
        !cv.forbids_ethics.iter().any(|f| base.contains(f))
    };
    let mut civic_scores: Vec<(f64, String)> = cfg
        .civic_scores
        .iter()
        .filter(|(k, _)| allowed(k.as_str()))
        .map(|(k, w)| (weighted(w, c), k.clone()))
        .collect();
    civic_scores.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal).then(x.1.cmp(&y.1)));
    let civics: Vec<String> = civic_scores.iter().take(2).map(|(_, k)| k.clone()).collect();
    let civic_reasons: Vec<Reason> =
        civic_scores.iter().take(2).map(|(sc, k)| reason("civic.score", Some(*sc), Some(k))).collect();
    let civic_alt = match (civic_scores.get(1), civic_scores.get(2)) {
        (Some(second), Some(third)) if second.0 - third.0 < s.close_call_margin => Some(third.1.clone()),
        _ => None,
    };

    // ---- origin
    let automation = c.get("technology") * c.get("industrialization") * (0.5 + 0.5 * c.get("economic_planning"));
    let (origin, origin_reason) = if flags.contains("nuclear_war") {
        ("post_apocalyptic", reason("origin.nuclear_war", None, None))
    } else if flags.contains("ecological_collapse") {
        ("doomsday", reason("origin.ecological_collapse", None, None))
    } else if base.contains("materialist") && automation >= s.mechanists_automation_min {
        ("mechanists", reason("origin.automation", Some(automation), None))
    } else {
        ("prosperous_unification", reason("origin.default", None, None))
    };
    let mut origin_reasons = vec![origin_reason];
    if !flags.contains("earth_unified") {
        origin_reasons.push(reason("origin.unification_incomplete", Some(c.get("planetary_unification")), None));
    }

    decisions.push(decision("authority", vec![authority.clone()], auth_reasons, None, coverage, rel.authority));
    decisions.push(decision("ethics", ethics.clone(), ethic_reasons, ethic_alt, coverage, rel.ethics));
    decisions.push(decision("civics", civics.clone(), civic_reasons, civic_alt, coverage, rel.civics));
    decisions.push(decision("origin", vec![origin.to_string()], origin_reasons, None, coverage, rel.origin));

    EmpireDesign {
        authority,
        ethics,
        civics,
        origin: origin.to_string(),
        axes: scores,
        decisions,
        vocabulary_verified: vocab.verified,
    }
}

/// Check a design against the vocabulary rules (the in-game designer remains the final judge).
pub fn validate_design(d: &EmpireDesign, vocab: &Vocabulary) -> Vec<String> {
    let mut problems = Vec::new();
    if d.ethic_points() != 3 {
        problems.push(format!("ethics use {} points, expected 3", d.ethic_points()));
    }
    let base = d.base_ethics();
    for e in &base {
        match vocab.ethics.get(e) {
            None => problems.push(format!("unknown ethic {e}")),
            Some(def) if base.contains(&def.opposite) => problems.push(format!("{e} conflicts with {}", def.opposite)),
            _ => {}
        }
    }
    match vocab.authorities.get(&d.authority) {
        None => problems.push(format!("unknown authority {}", d.authority)),
        Some(auth) => {
            for f in &auth.forbids_ethics {
                if base.contains(f.trim_start_matches("fanatic_")) {
                    problems.push(format!("authority {} forbids {f}", d.authority));
                }
            }
        }
    }
    if d.civics.len() != 2 || d.civics[0] == d.civics.get(1).cloned().unwrap_or_default() {
        problems.push("exactly two different civics are required".to_string());
    }
    for k in &d.civics {
        let Some(cv) = vocab.civics.get(k) else {
            problems.push(format!("unknown civic {k}"));
            continue;
        };
        if !cv.authorities.is_empty() && !cv.authorities.contains(&d.authority) {
            problems.push(format!("civic {k} needs authority {:?}", cv.authorities));
        }
        for r in &cv.requires_ethics {
            if !base.contains(r) {
                problems.push(format!("civic {k} needs ethic {r}"));
            }
        }
        for f in &cv.forbids_ethics {
            if base.contains(f) {
                problems.push(format!("civic {k} forbids ethic {f}"));
            }
        }
    }
    match vocab.origins.get(&d.origin) {
        None => problems.push(format!("unknown origin {}", d.origin)),
        Some(o) => {
            for r in &o.requires_ethics {
                if !base.contains(r) {
                    problems.push(format!("origin {} needs ethic {r}", d.origin));
                }
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use crate::state::presets;
    use crate::run;

    const SEED: u64 = 45_819_283;

    fn design(name: &str) -> crate::EmpireDesign {
        let start = presets().into_iter().find(|(n, _)| *n == name).unwrap().1;
        let r = run(&start, SEED).unwrap();
        assert!(r.problems.is_empty(), "{name}: {:?}", r.problems);
        r.design
    }

    fn sorted(v: &[String]) -> Vec<String> {
        let mut v = v.to_vec();
        v.sort();
        v
    }

    /// Spec example: global democracy, high tech, cooperation, equality.
    #[test]
    fn global_democracy() {
        let d = design("global_democracy");
        assert_eq!(d.authority, "democratic");
        assert_eq!(sorted(&d.ethics), ["egalitarian", "materialist", "xenophile"]);
        assert_eq!(d.civics, ["beacon_of_liberty", "technocracy"]);
        assert_eq!(d.origin, "prosperous_unification");
    }

    /// Spec example: military dictatorship, nationalism, constant wars.
    #[test]
    fn military_dictatorship() {
        let d = design("military_dictatorship");
        assert_eq!(d.authority, "dictatorial");
        assert_eq!(sorted(&d.ethics), ["authoritarian", "militarist", "xenophobe"]);
        assert_eq!(d.civics, ["distinguished_admiralty", "nationalistic_zeal"]);
    }

    #[test]
    fn planned_technocracy_becomes_mechanists() {
        let d = design("planned_technocracy");
        assert_eq!(d.origin, "mechanists");
        assert_eq!(d.ethics[0], "fanatic_materialist");
        assert_eq!(d.civics, ["technocracy", "free_haven"]);
    }

    #[test]
    fn every_seed_gives_a_valid_design() {
        for (name, p) in presets() {
            for seed in 0..60 {
                let r = run(&p, seed).unwrap();
                assert!(r.problems.is_empty(), "{name} seed {seed}: {:?}", r.problems);
                assert_eq!(r.design.ethic_points(), 3);
                assert_eq!(r.design.decisions.len(), 4);
            }
        }
    }

    #[test]
    fn partial_data_lowers_confidence() {
        use crate::state::{CivilizationState, Ideology};
        let mut v = std::collections::BTreeMap::new();
        for k in ["authoritarianism", "militarism", "technology"] {
            v.insert(k.to_string(), 0.7);
        }
        let s = CivilizationState::from_semantics(&v, 2, Ideology::NonAligned);
        let r = run(&s, 1).unwrap();
        for d in &r.design.decisions {
            assert!(d.confidence < 0.2, "3 of 14 indicators known: {}", d.confidence);
            assert_eq!(d.review_state, "pending_review");
        }
    }
}
