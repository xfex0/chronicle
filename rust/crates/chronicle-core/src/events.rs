//! Event provenance vocabulary. A journal entry is either OBSERVED (the game recorded it) or
//! INFERRED (Chronicle reconstructed it, e.g. by diffing two snapshots) — and says so.

use serde::{Deserialize, Serialize};

use crate::date::PartialDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatePrecision {
    Day,
    Month,
    Year,
    /// Happened somewhere between `date` and `date_to` (e.g. between two snapshots).
    Range,
    Unknown,
}

impl DatePrecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Month => "month",
            Self::Year => "year",
            Self::Range => "range",
            Self::Unknown => "unknown",
        }
    }
    /// Precision implied by which parts of a date are known.
    pub fn of(d: &PartialDate) -> Self {
        match (d.month, d.day) {
            (Some(_), Some(_)) => Self::Day,
            (Some(_), None) => Self::Month,
            _ => Self::Year,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventOrigin {
    /// Read from game data files (scripted history).
    Game,
    /// Read directly from a save.
    Save,
    /// Reconstructed from the difference between two snapshots.
    SnapshotDiff,
    /// Created by Chronicle itself (campaign started, conversion, split into successors).
    Converter,
    User,
}

impl EventOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Save => "save",
            Self::SnapshotDiff => "snapshot_diff",
            Self::Converter => "converter",
            Self::User => "user",
        }
    }
    pub fn evidence(self) -> EventEvidence {
        match self {
            Self::SnapshotDiff => EventEvidence::Inferred,
            _ => EventEvidence::Observed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventEvidence {
    Observed,
    Inferred,
}

impl EventEvidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Inferred => "inferred",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precision_from_date_and_evidence_from_origin() {
        assert_eq!(DatePrecision::of(&PartialDate::ymd(1092, 4, 12)), DatePrecision::Day);
        assert_eq!(DatePrecision::of(&PartialDate { year: 1092, month: Some(4), day: None }), DatePrecision::Month);
        assert_eq!(DatePrecision::of(&PartialDate::year(1092)), DatePrecision::Year);
        assert_eq!(EventOrigin::SnapshotDiff.evidence(), EventEvidence::Inferred);
        assert_eq!(EventOrigin::Save.evidence(), EventEvidence::Observed);
    }
}
