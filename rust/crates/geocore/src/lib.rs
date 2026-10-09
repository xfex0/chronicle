//! GeoCore prototype — weighted overlap projection (MVP 2 grows this into polygons,
//! adjacency and georeferenced game maps; see docs/CHRONICLE.md §GeoCore).
//!
//! Territory mapping between games.
//!
//! A mapping is a set of weighted links `source territory → target province`. For every
//! source, outgoing weights sum to 1: a source split across 3 targets sends 1/3 (or the
//! configured share) of its *additive* quantities to each. Distributions (culture,
//! religion) are mixed weighted by the population each link carries. Ownership is the
//! owner contributing the largest weight (ties → lowest Uid, deterministic).
//!
//! Mapping files are YAML in `games/<game>/mapping/` and `config/mappings/`; Python loads
//! them and passes the parsed structure here (no YAML dependency in Rust).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
/// Chronicle geographic / entity id, e.g. `geo_area_004821`.
pub type Uid = String;

/// Sparse share map; shares sum to 1 after `normalize`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Distribution(pub BTreeMap<Uid, f64>);

impl Distribution {
    pub fn total(&self) -> f64 {
        self.0.values().sum()
    }
    pub fn normalize(&mut self) -> bool {
        let t = self.total();
        if t <= 0.0 || !t.is_finite() {
            return false;
        }
        self.0.values_mut().for_each(|v| *v /= t);
        true
    }
    /// Largest share; ties broken by id order (deterministic).
    pub fn dominant(&self) -> Option<&Uid> {
        self.0
            .iter()
            .fold(None::<(&Uid, f64)>, |best, (k, &v)| match best {
                Some((_, bv)) if bv >= v => best,
                _ => Some((k, v)),
            })
            .map(|(k, _)| k)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub source: Uid,
    /// Target-game native province id (kept as string; numbering is game-specific).
    pub target: String,
    #[serde(default = "one")]
    pub weight: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerritoryMapping {
    pub links: Vec<Link>,
}

#[derive(Debug, Error, PartialEq)]
pub enum MappingError {
    #[error("negative or non-finite weight on link {from} -> {target}")]
    BadWeight { from: Uid, target: String },
    #[error("source {0} has zero total weight")]
    ZeroTotal(Uid),
}

/// Input for projection: per-source additive amount + distribution + owner.
#[derive(Debug, Clone, Default)]
pub struct SourceCell {
    pub amount: f64,
    pub distribution: Distribution,
    pub owner: Option<Uid>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TargetCell {
    pub amount: f64,
    pub distribution: Distribution,
    pub owner: Option<Uid>,
}

impl TerritoryMapping {
    /// Normalize outgoing weights per source to sum to 1.
    pub fn normalized(&self) -> Result<Self, MappingError> {
        let mut totals: BTreeMap<&Uid, f64> = BTreeMap::new();
        for l in &self.links {
            if !(l.weight.is_finite() && l.weight >= 0.0) {
                return Err(MappingError::BadWeight { from: l.source.clone(), target: l.target.clone() });
            }
            *totals.entry(&l.source).or_default() += l.weight;
        }
        if let Some((s, _)) = totals.iter().find(|(_, t)| **t <= 0.0) {
            return Err(MappingError::ZeroTotal((*s).clone()));
        }
        let links = self
            .links
            .iter()
            .map(|l| Link { weight: l.weight / totals[&l.source], ..l.clone() })
            .collect();
        Ok(Self { links })
    }

    /// Sources that have no outgoing link (their data would be lost).
    pub fn unmapped<'a>(&self, sources: impl IntoIterator<Item = &'a Uid>) -> Vec<Uid> {
        let mapped: std::collections::BTreeSet<&Uid> = self.links.iter().map(|l| &l.source).collect();
        sources.into_iter().filter(|s| !mapped.contains(s)).cloned().collect()
    }

    /// Project source cells onto targets. `self` must be normalized.
    pub fn project(&self, cells: &BTreeMap<Uid, SourceCell>) -> BTreeMap<String, TargetCell> {
        let mut out: BTreeMap<String, TargetCell> = BTreeMap::new();
        let mut owner_votes: BTreeMap<String, BTreeMap<Uid, f64>> = BTreeMap::new();
        for l in &self.links {
            let Some(c) = cells.get(&l.source) else { continue };
            let carried = c.amount * l.weight;
            let t = out.entry(l.target.clone()).or_default();
            t.amount += carried;
            for (k, share) in &c.distribution.0 {
                *t.distribution.0.entry(k.clone()).or_default() += share * carried;
            }
            if let Some(o) = &c.owner {
                *owner_votes.entry(l.target.clone()).or_default().entry(o.clone()).or_default() += l.weight;
            }
        }
        for (target, cell) in out.iter_mut() {
            cell.distribution.normalize();
            if let Some(votes) = owner_votes.get(target) {
                cell.owner = Distribution(votes.clone()).dominant().cloned();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Uid {
        s.to_string()
    }

    #[test]
    fn split_and_merge() {
        let m = TerritoryMapping {
            links: vec![
                Link { source: u("t:1"), target: "100".into(), weight: 1.0 },
                Link { source: u("t:1"), target: "101".into(), weight: 3.0 },
                Link { source: u("t:2"), target: "101".into(), weight: 1.0 },
            ],
        }
        .normalized()
        .unwrap();

        let mut cells = BTreeMap::new();
        cells.insert(u("t:1"), SourceCell {
            amount: 1000.0,
            distribution: Distribution([(u("c:a"), 1.0)].into_iter().collect()),
            owner: Some(u("country:1")),
        });
        cells.insert(u("t:2"), SourceCell {
            amount: 250.0,
            distribution: Distribution([(u("c:b"), 1.0)].into_iter().collect()),
            owner: Some(u("country:2")),
        });
        let out = m.project(&cells);
        assert!((out["100"].amount - 250.0).abs() < 1e-9);
        assert!((out["101"].amount - 1000.0).abs() < 1e-9);
        // 750 of culture a, 250 of b
        assert!((out["101"].distribution.0[&u("c:a")] - 0.75).abs() < 1e-9);
        // country:1 contributes weight 0.75, country:2 contributes 1.0
        assert_eq!(out["101"].owner, Some(u("country:2")));
        let total: f64 = out.values().map(|c| c.amount).sum();
        assert!((total - 1250.0).abs() < 1e-9, "additive quantities are conserved");
    }

    #[test]
    fn rejects_bad_weights_and_reports_unmapped() {
        let m = TerritoryMapping { links: vec![Link { source: u("t:1"), target: "1".into(), weight: -1.0 }] };
        assert!(matches!(m.normalized(), Err(MappingError::BadWeight { .. })));
        let m = TerritoryMapping { links: vec![Link { source: u("t:1"), target: "1".into(), weight: 1.0 }] };
        assert_eq!(m.unmapped([&u("t:1"), &u("t:9")]), vec![u("t:9")]);
    }
}
