//! Civilization of Earth at the end of the HoI4 era.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The 14 indicators, in the order the simulation applies social drift to the first 8.
pub const INDICATORS: [&str; 14] = [
    "authoritarianism",
    "social_equality",
    "xenophobia",
    "international_cooperation",
    "religious_influence",
    "militarism",
    "global_wars",
    "nuclear_weapons",
    "industrialization",
    "technology",
    "space_program",
    "economic_planning",
    "environmental_policy",
    "planetary_unification",
];

/// Indicators that drift socially (mean reversion + noise), in simulation order.
pub const SOCIAL: [&str; 8] = [
    "authoritarianism",
    "social_equality",
    "xenophobia",
    "international_cooperation",
    "religious_influence",
    "militarism",
    "economic_planning",
    "environmental_policy",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ideology {
    Democracy,
    Communism,
    Fascism,
    Monarchy,
    NonAligned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CivilizationState {
    pub values: BTreeMap<String, f64>,
    /// Competing power blocs (1 = one dominant bloc).
    pub blocs: u32,
    pub ideology: Ideology,
    /// How many indicators came from real data (the rest are neutral defaults).
    #[serde(default = "all_known")]
    pub known_indicators: usize,
}

fn all_known() -> usize {
    INDICATORS.len()
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StateError {
    #[error("indicator '{0}' is missing")]
    Missing(String),
    #[error("indicator '{0}' = {1} is outside 0..1")]
    OutOfRange(String, f64),
    #[error("unknown indicator '{0}'")]
    Unknown(String),
    #[error("blocs must be between 1 and 6")]
    Blocs,
}

impl CivilizationState {
    pub fn get(&self, key: &str) -> f64 {
        self.values.get(key).copied().unwrap_or(0.5)
    }

    pub fn add(&mut self, key: &str, dv: f64) {
        let v = (self.get(key) + dv).max(0.0).min(1.0);
        self.values.insert(key.to_string(), v);
    }

    pub fn validate(&self) -> Result<(), StateError> {
        for k in INDICATORS {
            let v = *self.values.get(k).ok_or_else(|| StateError::Missing(k.into()))?;
            if !(0.0..=1.0).contains(&v) {
                return Err(StateError::OutOfRange(k.into(), v));
            }
        }
        if let Some(k) = self.values.keys().find(|k| !INDICATORS.contains(&k.as_str())) {
            return Err(StateError::Unknown(k.clone()));
        }
        if !(1..=6).contains(&self.blocs) {
            return Err(StateError::Blocs);
        }
        Ok(())
    }

    /// Share of indicators backed by data (feeds confidence).
    pub fn coverage(&self) -> (usize, usize) {
        (self.known_indicators.min(INDICATORS.len()), INDICATORS.len())
    }

    /// Build from Semantic Registry values (key → 0..1). Missing indicators get a neutral 0.5
    /// and lower the data coverage — that is how a partial HoI4 import stays honest.
    pub fn from_semantics(values: &BTreeMap<String, f64>, blocs: u32, ideology: Ideology) -> Self {
        let mut out = BTreeMap::new();
        let mut known = 0;
        for k in INDICATORS {
            match values.get(k) {
                Some(v) if v.is_finite() => {
                    out.insert(k.to_string(), v.clamp(0.0, 1.0));
                    known += 1;
                }
                _ => {
                    out.insert(k.to_string(), 0.5);
                }
            }
        }
        Self { values: out, blocs, ideology, known_indicators: known }
    }
}

fn civ(blocs: u32, ideology: Ideology, v: &[(&str, f64)]) -> CivilizationState {
    CivilizationState {
        values: v.iter().map(|(k, x)| (k.to_string(), *x)).collect(),
        blocs,
        ideology,
        known_indicators: INDICATORS.len(),
    }
}

/// Example worlds (same numbers as the research prototype).
pub fn presets() -> Vec<(&'static str, CivilizationState)> {
    vec![
        (
            "global_democracy",
            civ(1, Ideology::Democracy, &[
                ("authoritarianism", 0.1), ("social_equality", 0.9), ("xenophobia", 0.1),
                ("international_cooperation", 0.9), ("religious_influence", 0.2), ("militarism", 0.2),
                ("global_wars", 0.05), ("nuclear_weapons", 0.4), ("industrialization", 0.7), ("technology", 0.6),
                ("space_program", 0.1), ("economic_planning", 0.3), ("environmental_policy", 0.6),
                ("planetary_unification", 0.5),
            ]),
        ),
        (
            "military_dictatorship",
            civ(3, Ideology::Fascism, &[
                ("authoritarianism", 0.9), ("social_equality", 0.1), ("xenophobia", 0.85),
                ("international_cooperation", 0.1), ("religious_influence", 0.4), ("militarism", 0.9),
                ("global_wars", 0.8), ("nuclear_weapons", 0.8), ("industrialization", 0.6), ("technology", 0.5),
                ("space_program", 0.1), ("economic_planning", 0.6), ("environmental_policy", 0.2),
                ("planetary_unification", 0.2),
            ]),
        ),
        (
            "cold_war",
            civ(2, Ideology::NonAligned, &[
                ("authoritarianism", 0.5), ("social_equality", 0.5), ("xenophobia", 0.5),
                ("international_cooperation", 0.3), ("religious_influence", 0.4), ("militarism", 0.6),
                ("global_wars", 0.3), ("nuclear_weapons", 0.9), ("industrialization", 0.6), ("technology", 0.55),
                ("space_program", 0.15), ("economic_planning", 0.5), ("environmental_policy", 0.3),
                ("planetary_unification", 0.2),
            ]),
        ),
        (
            "planned_technocracy",
            civ(2, Ideology::Communism, &[
                ("authoritarianism", 0.45), ("social_equality", 0.75), ("xenophobia", 0.3),
                ("international_cooperation", 0.7), ("religious_influence", 0.1), ("militarism", 0.3),
                ("global_wars", 0.1), ("nuclear_weapons", 0.3), ("industrialization", 0.8), ("technology", 0.7),
                ("space_program", 0.2), ("economic_planning", 0.9), ("environmental_policy", 0.5),
                ("planetary_unification", 0.4),
            ]),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_valid() {
        for (name, p) in presets() {
            assert_eq!(p.validate(), Ok(()), "{name}");
        }
    }

    #[test]
    fn partial_semantics_lower_coverage() {
        let mut v = BTreeMap::new();
        v.insert("militarism".to_string(), 0.8);
        v.insert("technology".to_string(), 1.4);
        let s = CivilizationState::from_semantics(&v, 2, Ideology::NonAligned);
        assert_eq!(s.coverage(), (2, 14));
        assert_eq!(s.get("technology"), 1.0);
        assert_eq!(s.get("xenophobia"), 0.5);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn rejects_bad_input() {
        let mut s = presets()[0].1.clone();
        s.values.insert("militarism".into(), 1.5);
        assert!(matches!(s.validate(), Err(StateError::OutOfRange(..))));
        s.values.remove("militarism");
        assert!(matches!(s.validate(), Err(StateError::Missing(..))));
    }
}
