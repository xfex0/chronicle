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

pub mod align;
pub mod ck3_map;
pub mod overlap;
pub mod raster;

pub use align::{Affine, ControlPoint};
pub use overlap::{OverlapCounts, OverlapLink, overlap};
pub use raster::{LabelRaster, RasterError};

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

#[cfg(test)]
mod geocore_engine_tests {
    use std::collections::HashMap;

    use super::*;

    /// 4×4 source map: left half province 1, right half 2.
    fn source() -> LabelRaster {
        let labels = (0..16).map(|i| if i % 4 < 2 { 1 } else { 2 }).collect();
        LabelRaster { width: 4, height: 4, labels }
    }

    #[test]
    fn affine_from_control_points() {
        // target is the source scaled ×2 and shifted by (10, 0)
        let pts = [
            ControlPoint { from: (10.0, 0.0), to: (0.0, 0.0) },
            ControlPoint { from: (18.0, 0.0), to: (4.0, 0.0) },
            ControlPoint { from: (10.0, 8.0), to: (0.0, 4.0) },
            ControlPoint { from: (18.0, 8.0), to: (4.0, 4.0) },
        ];
        let t = Affine::fit(&pts).unwrap();
        assert!(t.max_error(&pts) < 1e-9);
        let (x, y) = t.apply((14.0, 4.0));
        assert!((x - 2.0).abs() < 1e-9 && (y - 2.0).abs() < 1e-9);
        assert!(Affine::fit(&pts[..2]).is_none(), "two points are not enough");
    }

    #[test]
    fn overlap_shares_both_ways() {
        // target 8×8: one big location 10 on the left 3 columns (×2), location 20 on the rest
        let labels = (0..64).map(|i| if i % 8 < 3 { 10 } else { 20 }).collect();
        let target = LabelRaster { width: 8, height: 8, labels };
        let to_source = Affine { a: 0.5, b: 0.0, c: 0.0, d: 0.0, e: 0.5, f: 0.0 };
        let counts = overlap(&source(), &target, &to_source, 1);
        let links = counts.links(0.0);
        let get = |s, t| links.iter().find(|l| l.source == s && l.target == t).cloned();
        // source 1 (target columns 0..4): 3 columns in 10, 1 in 20
        assert!((get(1, 10).unwrap().share_of_source - 0.75).abs() < 1e-9);
        assert!((get(1, 20).unwrap().share_of_source - 0.25).abs() < 1e-9);
        assert_eq!(get(2, 10), None);
        assert!((get(2, 20).unwrap().share_of_source - 1.0).abs() < 1e-9);
        // location 20: 1 column from province 1, 4 from province 2
        assert!((get(2, 20).unwrap().share_of_target - 0.8).abs() < 1e-9);
        assert!(counts.unmatched_sources([1u32, 2, 3].iter()).contains(&3));
    }

    #[test]
    fn png_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("provinces.png");
        let (w, h) = (3u32, 2u32);
        let pixels: Vec<u8> = vec![255, 0, 0, 0, 255, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255, 9, 9, 9];
        {
            let file = std::fs::File::create(&path).unwrap();
            let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&pixels).unwrap();
        }
        let colors: HashMap<[u8; 3], u32> = [([255, 0, 0], 1), ([0, 255, 0], 2), ([0, 0, 255], 3)].into_iter().collect();
        let r = LabelRaster::from_png(&path, &colors).unwrap();
        assert_eq!(r.labels, vec![1, 2, 1, 3, 3, 0]);
        let merged = r.relabel(&[(1, 7), (2, 7)].into_iter().collect());
        assert_eq!(merged.labels, vec![7, 7, 7, 0, 0, 0]);
    }

    #[test]
    fn ck3_definition_and_landed_titles() {
        let csv = "0;0;0;0;x;x;\n1;42;3;128;Vestisland;x;\n2;84;6;1;Reykjavik;x;\n# comment\n";
        let colors = ck3_map::definition_colors(csv);
        assert_eq!(colors.get(&[42, 3, 128]), Some(&1));
        assert_eq!(colors.len(), 2);
        let lt = "e_britannia = { k_england = { d_york = { c_york = { b_york = { province = 1 } b_tadcaster = { province = 2 } }\n c_hull = { b_hull = { province = 3 } } } } }";
        let p = ck3_map::province_counties(lt);
        assert_eq!(p.get(&1).map(String::as_str), Some("c_york"));
        assert_eq!(p.get(&2).map(String::as_str), Some("c_york"));
        assert_eq!(p.get(&3).map(String::as_str), Some("c_hull"));
        let (map, keys) = ck3_map::county_labels(&p);
        assert_eq!(keys, ["c_hull", "c_york"]);
        assert_eq!(map[&1], map[&2]);
    }
}
