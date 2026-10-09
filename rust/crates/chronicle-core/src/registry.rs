//! games_registry.yaml — every game fact Chronicle relies on, with `verified` flags.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::date::PartialDate;
use crate::settings::TargetStart;

/// The registry shipped with the app (compiled in, so it cannot go missing).
pub const BUILTIN_REGISTRY: &str = include_str!("../../../../config/games_registry.yaml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GameKind {
    #[default]
    Game,
    /// Simulated by Chronicle itself (Modern Era Bridge); nothing to install.
    Virtual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportStatus {
    Mvp,
    Planned,
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Era {
    pub start: PartialDate,
    #[serde(default)]
    pub end: Option<PartialDate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// Paths relative to the install dir; any one existing counts.
    pub candidates: Vec<String>,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveDirHint {
    /// Relative to the user's Documents folder (Windows/macOS) or ~/.local/share (Linux).
    pub documents_subpath: Vec<String>,
    pub verified: bool,
}

/// Kinds of save files, as detected from the container (see paradox-parser).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveFormat {
    /// Uncompressed text.
    Plaintext,
    /// Text inside a ZIP container.
    Compressed,
    /// Token-encoded binary (ironman, and some autosaves). Needs per-patch token data.
    Binary,
}

impl SaveFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plaintext => "plaintext",
            Self::Compressed => "compressed",
            Self::Binary => "binary",
        }
    }
}

/// What the CURRENT adapter version can do with a save format. Never a global "turn off
/// ironman": the answer depends on adapter and game version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatSupport {
    Supported,
    Planned,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameDef {
    #[serde(skip_deserializing)]
    pub key: String,
    pub display_name: String,
    #[serde(default)]
    pub kind: GameKind,
    #[serde(default)]
    pub steam_app_id: Option<u32>,
    pub status: SupportStatus,
    pub era: Era,
    #[serde(default)]
    pub era_verified: bool,
    #[serde(default)]
    pub install_fingerprint: Option<Fingerprint>,
    #[serde(default)]
    pub save_dir: Option<SaveDirHint>,
    /// Save-format capabilities of this Chronicle version's adapter.
    #[serde(default)]
    pub save_formats: BTreeMap<SaveFormat, FormatSupport>,
    /// File extensions of this game's saves, e.g. [".ck3"].
    #[serde(default)]
    pub save_extensions: Vec<String>,
}

impl GameDef {
    pub fn format_support(&self, f: SaveFormat) -> FormatSupport {
        self.save_formats.get(&f).copied().unwrap_or(FormatSupport::Planned)
    }

    pub fn steam_launch_url(&self) -> Option<String> {
        self.steam_app_id.map(|id| format!("steam://run/{id}"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionDef {
    pub from: String,
    pub to: String,
    pub default_date: PartialDate,
    /// Other suggested dates shown in the UI (e.g. 1453.1.1 for "finish CK3").
    #[serde(default)]
    pub alternatives: Vec<PartialDate>,
    /// Can the target adapter build a world at a non-native start date?
    #[serde(default)]
    pub custom_start_supported: bool,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawRegistry {
    version: u32,
    games: BTreeMap<String, GameDef>,
    chain: Vec<String>,
    transitions: Vec<TransitionDef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameRegistry {
    pub version: u32,
    pub games: BTreeMap<String, GameDef>,
    pub chain: Vec<String>,
    pub transitions: Vec<TransitionDef>,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("registry YAML is invalid: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("unknown game '{0}' referenced in {1}")]
    UnknownGame(String, &'static str),
    #[error("transition {from}→{to}: default date {date} is outside the {from} era")]
    DateOutsideEra { from: String, to: String, date: PartialDate },
    #[error("transition {from}→{to}: {date} is after the native start of {to} ({target_start}); choose an earlier date or start {to} at the transition date (custom start)")]
    Backwards { from: String, to: String, date: PartialDate, target_start: PartialDate },
}

impl GameRegistry {
    pub fn builtin() -> Result<Self, RegistryError> {
        Self::from_yaml(BUILTIN_REGISTRY)
    }

    pub fn from_yaml(src: &str) -> Result<Self, RegistryError> {
        let raw: RawRegistry = serde_yaml::from_str(src)?;
        let mut games = raw.games;
        for (k, g) in games.iter_mut() {
            g.key = k.clone();
        }
        let reg = Self { version: raw.version, games, chain: raw.chain, transitions: raw.transitions };
        reg.validate()?;
        Ok(reg)
    }

    pub fn game(&self, key: &str) -> Option<&GameDef> {
        self.games.get(key)
    }

    pub fn transition(&self, from: &str, to: &str) -> Option<&TransitionDef> {
        self.transitions.iter().find(|t| t.from == from && t.to == to)
    }

    /// Next game in the main chain.
    pub fn next_in_chain(&self, key: &str) -> Option<&str> {
        let i = self.chain.iter().position(|g| g == key)?;
        self.chain.get(i + 1).map(String::as_str)
    }

    /// Validates a (possibly user-chosen) transition date.
    ///
    /// - always: the date lies inside the source game's era;
    /// - `TargetStart::Native`: the date is not after the target's native start (otherwise the
    ///   target would begin earlier than the world we hand it — history would move backwards);
    /// - `TargetStart::Custom`: the target world starts AT the transition date, so any date in
    ///   the source era moves forward (needs adapter support — see `custom_start_supported`).
    pub fn check_transition_date(
        &self,
        from: &str,
        to: &str,
        date: PartialDate,
        target_start: TargetStart,
    ) -> Result<(), RegistryError> {
        let src = self.game(from).ok_or_else(|| RegistryError::UnknownGame(from.into(), "transition"))?;
        let dst = self.game(to).ok_or_else(|| RegistryError::UnknownGame(to.into(), "transition"))?;
        let within = date >= src.era.start && src.era.end.is_none_or(|end| date <= end);
        if !within {
            return Err(RegistryError::DateOutsideEra { from: from.into(), to: to.into(), date });
        }
        if target_start == TargetStart::Native && date > dst.era.start {
            return Err(RegistryError::Backwards {
                from: from.into(),
                to: to.into(),
                date,
                target_start: dst.era.start,
            });
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), RegistryError> {
        for g in &self.chain {
            if !self.games.contains_key(g) {
                return Err(RegistryError::UnknownGame(g.clone(), "chain"));
            }
        }
        for t in &self.transitions {
            self.check_transition_date(&t.from, &t.to, t.default_date, TargetStart::Native)?;
            for alt in &t.alternatives {
                self.check_transition_date(&t.from, &t.to, *alt, TargetStart::Custom)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_is_valid() {
        let r = GameRegistry::builtin().unwrap();
        assert_eq!(r.chain.first().map(String::as_str), Some("imperator"));
        assert_eq!(r.next_in_chain("ck3"), Some("eu5"));
        assert_eq!(r.game("eu5").unwrap().steam_app_id, Some(3450310));
        assert_eq!(r.game("ck3").unwrap().steam_launch_url().as_deref(), Some("steam://run/1158310"));
        assert!(r.game("modern_bridge").unwrap().steam_app_id.is_none());
        // CK3 text saves import (verified on 1.0.2); binary needs non-distributable tokens.
        let ck3 = r.game("ck3").unwrap();
        assert_eq!(ck3.format_support(SaveFormat::Plaintext), FormatSupport::Supported);
        assert_eq!(ck3.format_support(SaveFormat::Compressed), FormatSupport::Supported);
        assert_eq!(ck3.format_support(SaveFormat::Binary), FormatSupport::Unsupported);
        assert_eq!(r.game("hoi4").unwrap().format_support(SaveFormat::Binary), FormatSupport::Unsupported);
        assert_eq!(r.game("eu5").unwrap().format_support(SaveFormat::Binary), FormatSupport::Planned);
        assert_eq!(ck3.save_extensions, [".ck3"]);
    }

    #[test]
    fn ck3_to_eu5_defaults_to_eu5_start() {
        let r = GameRegistry::builtin().unwrap();
        let t = r.transition("ck3", "eu5").unwrap();
        assert_eq!(t.default_date, PartialDate::ymd(1337, 4, 1));
    }

    #[test]
    fn user_dates_are_checked() {
        let r = GameRegistry::builtin().unwrap();
        // Playing CK3 to 1400 then starting EU5 (1337) would move history backwards.
        let native = TargetStart::Native;
        assert!(matches!(r.check_transition_date("ck3", "eu5", PartialDate::year(1400), native),
                         Err(RegistryError::Backwards { .. })));
        assert!(r.check_transition_date("ck3", "eu5", PartialDate::year(1300), native).is_ok());
        assert!(matches!(r.check_transition_date("ck3", "eu5", PartialDate::year(800), native),
                         Err(RegistryError::DateOutsideEra { .. })));
        // "Finish CK3" mode: EU5 world starts at 1453 → forward-only, allowed.
        assert!(r.check_transition_date("ck3", "eu5", PartialDate::ymd(1453, 1, 1), TargetStart::Custom).is_ok());
        assert_eq!(r.transition("ck3", "eu5").unwrap().alternatives, vec![PartialDate::ymd(1453, 1, 1)]);
    }

    #[test]
    fn rejects_unknown_chain_member() {
        let y = "version: 1\ngames: {}\nchain: [nope]\ntransitions: []\n";
        assert!(matches!(GameRegistry::from_yaml(y), Err(RegistryError::UnknownGame(..))));
    }
}
