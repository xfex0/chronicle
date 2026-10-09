//! Per-campaign settings chosen in the New Campaign wizard and editable later.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::confidence::ConfidencePolicy;
use crate::date::PartialDate;
use crate::registry::{GameRegistry, RegistryError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransitionMode {
    #[default]
    Manual,
    Suggested,
    Automatic,
}

/// Where the next game's world starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TargetStart {
    /// The target game's own start date (seamless, recommended).
    #[default]
    Native,
    /// The target world starts at the transition date (e.g. finish CK3 in 1453, EU5 from 1453).
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignSettings {
    pub start_game: String,
    pub transition_mode: TransitionMode,
    /// User overrides of transition dates, keyed `"ck3->eu5"`. Missing = registry default.
    #[serde(default)]
    pub transition_dates: BTreeMap<String, PartialDate>,
    /// Per transition `"ck3->eu5"`: native (default) or custom target start.
    #[serde(default)]
    pub transition_target_start: BTreeMap<String, TargetStart>,
    #[serde(default)]
    pub confidence: ConfidencePolicy,
    pub seed: u64,
    #[serde(default = "default_profile")]
    pub coefficient_profile: String,
}

fn default_profile() -> String {
    "balanced".into()
}

pub fn transition_key(from: &str, to: &str) -> String {
    format!("{from}->{to}")
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error(transparent)]
    Confidence(#[from] crate::confidence::PolicyError),
    #[error("start game '{0}' is not in the campaign chain")]
    StartGame(String),
    #[error("bad transition key '{0}' (expected 'from->to')")]
    Key(String),
}

impl CampaignSettings {
    pub fn new(start_game: &str, seed: u64) -> Self {
        Self {
            start_game: start_game.into(),
            transition_mode: TransitionMode::default(),
            transition_dates: BTreeMap::new(),
            transition_target_start: BTreeMap::new(),
            confidence: ConfidencePolicy::default(),
            seed,
            coefficient_profile: default_profile(),
        }
    }

    pub fn transition_date(&self, reg: &GameRegistry, from: &str, to: &str) -> Option<PartialDate> {
        self.transition_dates
            .get(&transition_key(from, to))
            .copied()
            .or_else(|| reg.transition(from, to).map(|t| t.default_date))
    }

    pub fn target_start(&self, from: &str, to: &str) -> TargetStart {
        self.transition_target_start.get(&transition_key(from, to)).copied().unwrap_or_default()
    }

    pub fn validate(&self, reg: &GameRegistry) -> Result<(), SettingsError> {
        if !reg.chain.contains(&self.start_game) {
            return Err(SettingsError::StartGame(self.start_game.clone()));
        }
        self.confidence.validate()?;
        for key in self.transition_target_start.keys() {
            key.split_once("->").ok_or_else(|| SettingsError::Key(key.clone()))?;
        }
        for (key, date) in &self.transition_dates {
            let (from, to) = key.split_once("->").ok_or_else(|| SettingsError::Key(key.clone()))?;
            reg.check_transition_date(from, to, *date, self.target_start(from, to))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_overrides() {
        let reg = GameRegistry::builtin().unwrap();
        let mut s = CampaignSettings::new("ck3", 45819283);
        assert_eq!(s.transition_date(&reg, "ck3", "eu5"), Some(PartialDate::ymd(1337, 4, 1)));
        s.transition_dates.insert(transition_key("ck3", "eu5"), PartialDate::year(1300));
        assert_eq!(s.transition_date(&reg, "ck3", "eu5"), Some(PartialDate::year(1300)));
        assert!(s.validate(&reg).is_ok());
        s.transition_dates.insert(transition_key("ck3", "eu5"), PartialDate::year(1450));
        assert!(s.validate(&reg).is_err(), "native EU5 start is 1337");
        s.transition_target_start.insert(transition_key("ck3", "eu5"), TargetStart::Custom);
        assert!(s.validate(&reg).is_ok(), "custom start: EU5 begins in 1450");
    }

    #[test]
    fn json_roundtrip() {
        let s = CampaignSettings::new("imperator", 1);
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<CampaignSettings>(&j).unwrap(), s);
    }
}
