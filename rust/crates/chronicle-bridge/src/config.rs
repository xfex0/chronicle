use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

pub type Weights = BTreeMap<String, f64>;

#[derive(Debug, Clone, Deserialize)]
pub struct Drift {
    pub technology: f64,
    pub industrialization: f64,
    pub space_from_technology: f64,
    pub space_race_bonus: f64,
    pub space_cooperation_bonus: f64,
    pub mean_reversion: f64,
    pub noise: f64,
    pub wars_relaxation: f64,
    pub wars_target_militarism: f64,
    pub unification_from_cooperation: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Chance {
    pub base: f64,
    #[serde(default)]
    pub effects: Weights,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClimateCrisis {
    pub min_industrialization: f64,
    pub base: f64,
    pub collapse_below_environment: f64,
    pub effects: Weights,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Conquest {
    pub min_militarism: f64,
    pub base: f64,
    pub effects: Weights,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PeacefulMerge {
    pub min_cooperation: f64,
    pub base: f64,
    pub effects: Weights,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Events {
    pub nuclear_war: Chance,
    pub climate_crisis: ClimateCrisis,
    pub conquest: Conquest,
    pub peaceful_merge: PeacefulMerge,
    pub unified_at: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Milestone {
    pub name: String,
    pub at: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MappingReliability {
    pub authority: f64,
    pub ethics: f64,
    pub civics: f64,
    pub origin: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StellarisRules {
    pub fanatic_threshold: f64,
    pub democratic_min: f64,
    pub dictatorial_min: f64,
    pub imperial_min: f64,
    pub mechanists_automation_min: f64,
    pub close_call_margin: f64,
    pub mapping_reliability: MappingReliability,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BridgeConfig {
    pub version: u32,
    pub start_year: i32,
    pub end_year: i32,
    pub step_years: i32,
    pub drift: Drift,
    pub events: Events,
    pub space_milestones: Vec<Milestone>,
    pub stellaris: StellarisRules,
    pub axes: BTreeMap<String, Weights>,
    pub civic_scores: BTreeMap<String, Weights>,
}

pub const BUILTIN_CONFIG: &str = include_str!("../../../../config/bridge.yaml");
pub const BUILTIN_VOCABULARY: &str = include_str!("../../../../games/stellaris/vocabulary.yaml");

impl BridgeConfig {
    pub fn from_yaml(src: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(src)
    }

    /// Parsed once; the built-in file is validated by tests, so a panic here is a build bug.
    pub fn builtin() -> &'static BridgeConfig {
        static CFG: OnceLock<BridgeConfig> = OnceLock::new();
        CFG.get_or_init(|| Self::from_yaml(BUILTIN_CONFIG).expect("config/bridge.yaml is valid"))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Authority {
    pub name: String,
    pub key: String,
    #[serde(default)]
    pub forbids_ethics: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ethic {
    pub name: String,
    pub opposite: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Civic {
    pub name: String,
    #[serde(default)]
    pub authorities: Vec<String>,
    #[serde(default)]
    pub requires_ethics: Vec<String>,
    #[serde(default)]
    pub forbids_ethics: Vec<String>,
    #[serde(default)]
    pub dlc: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Origin {
    pub name: String,
    pub key: String,
    #[serde(default)]
    pub requires_ethics: Vec<String>,
    #[serde(default)]
    pub dlc: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vocabulary {
    pub version: u32,
    pub verified: bool,
    pub authorities: BTreeMap<String, Authority>,
    pub ethics: BTreeMap<String, Ethic>,
    pub civics: BTreeMap<String, Civic>,
    pub origins: BTreeMap<String, Origin>,
}

impl Vocabulary {
    pub fn from_yaml(src: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(src)
    }

    pub fn builtin() -> &'static Vocabulary {
        static VOCAB: OnceLock<Vocabulary> = OnceLock::new();
        VOCAB.get_or_init(|| Self::from_yaml(BUILTIN_VOCABULARY).expect("games/stellaris/vocabulary.yaml is valid"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_files_parse_and_are_consistent() {
        let cfg = BridgeConfig::from_yaml(BUILTIN_CONFIG).unwrap();
        let vocab = Vocabulary::from_yaml(BUILTIN_VOCABULARY).unwrap();
        for civic in cfg.civic_scores.keys() {
            assert!(vocab.civics.contains_key(civic), "civic {civic} has a score but no vocabulary entry");
        }
        for (k, e) in &vocab.ethics {
            assert_eq!(vocab.ethics[&e.opposite].opposite, *k, "ethic opposites must be symmetric");
        }
        assert_eq!(cfg.axes.len(), 4);
    }
}
