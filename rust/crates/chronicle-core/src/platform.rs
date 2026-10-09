//! Where a game is installed from, and which OS Chronicle runs on.
//! MVP supports Steam + Manual on Windows 10/11; the rest is reserved so data and UI
//! do not need to change when support arrives.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallSource {
    Steam,
    Gog,
    /// Xbox app / Microsoft Store (PC Game Pass) — one install infrastructure.
    Xbox,
    Manual,
    Other,
    Missing,
}

impl InstallSource {
    pub const ALL: [InstallSource; 6] = [Self::Steam, Self::Gog, Self::Xbox, Self::Manual, Self::Other, Self::Missing];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Steam => "steam",
            Self::Gog => "gog",
            Self::Xbox => "xbox",
            Self::Manual => "manual",
            Self::Other => "other",
            Self::Missing => "missing",
        }
    }

    /// Auto-detection implemented in this version.
    pub fn detection_supported(self) -> bool {
        matches!(self, Self::Steam)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Windows,
    Linux,
    SteamDeck,
    MacOs,
}

impl Platform {
    pub fn current() -> Platform {
        if cfg!(windows) {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if std::env::var_os("SteamDeck").is_some() {
            Platform::SteamDeck
        } else {
            Platform::Linux
        }
    }

    /// MVP target: Windows 10/11.
    pub fn officially_supported(self) -> bool {
        matches!(self, Platform::Windows)
    }
}
