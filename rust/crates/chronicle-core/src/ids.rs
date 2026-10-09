//! Chronicle-owned stable ids: `chronicle_<kind>_<n:06>`. Game ids are never universal ids;
//! they live in `game_entity_mappings`. Numbers come from the per-campaign `id_counters` table.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Country,
    Territory,
    Region,
    Character,
    Dynasty,
    Culture,
    Religion,
    War,
    Organization,
    GeoArea,
}

impl EntityKind {
    pub const ALL: [EntityKind; 10] = [
        Self::Country,
        Self::Territory,
        Self::Region,
        Self::Character,
        Self::Dynasty,
        Self::Culture,
        Self::Religion,
        Self::War,
        Self::Organization,
        Self::GeoArea,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Country => "country",
            Self::Territory => "territory",
            Self::Region => "region",
            Self::Character => "character",
            Self::Dynasty => "dynasty",
            Self::Culture => "culture",
            Self::Religion => "religion",
            Self::War => "war",
            Self::Organization => "organization",
            Self::GeoArea => "geo_area",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChronicleId(String);

impl ChronicleId {
    pub fn new(kind: EntityKind, n: u64) -> Self {
        Self(format!("chronicle_{}_{n:06}", kind.as_str()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn kind(&self) -> Option<EntityKind> {
        let rest = self.0.strip_prefix("chronicle_")?;
        let (kind, _) = rest.rsplit_once('_')?;
        EntityKind::ALL.into_iter().find(|k| k.as_str() == kind)
    }
}

impl fmt::Display for ChronicleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("not a chronicle id: {0}")]
pub struct IdError(String);

impl FromStr for ChronicleId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = ChronicleId(s.to_string());
        let ok = id.kind().is_some() && s.rsplit_once('_').is_some_and(|(_, n)| n.parse::<u64>().is_ok());
        if ok { Ok(id) } else { Err(IdError(s.to_string())) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_and_parse() {
        let id = ChronicleId::new(EntityKind::GeoArea, 4821);
        assert_eq!(id.as_str(), "chronicle_geo_area_004821");
        assert_eq!(id.kind(), Some(EntityKind::GeoArea));
        assert_eq!("chronicle_dynasty_000742".parse::<ChronicleId>().unwrap().kind(), Some(EntityKind::Dynasty));
        assert!("ck3_title_k_ruthenia".parse::<ChronicleId>().is_err());
    }
}
