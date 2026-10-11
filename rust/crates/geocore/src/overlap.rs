//! Overlap calculator: which target units cover which source units, and by how much.
//!
//! The target raster is sampled on a grid; each sample is mapped into the source raster with
//! the alignment, and the (source, target) pair is counted. Counts become shares in both
//! directions: share of the source that lands in a target (to split additive values like
//! population) and share of the target that comes from a source (to decide ownership).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::align::Affine;
use crate::raster::LabelRaster;

#[derive(Debug, Clone, Default)]
pub struct OverlapCounts {
    pub pairs: BTreeMap<(u32, u32), u64>,
    pub source_totals: BTreeMap<u32, u64>,
    pub target_totals: BTreeMap<u32, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OverlapLink {
    pub source: u32,
    pub target: u32,
    pub share_of_source: f64,
    pub share_of_target: f64,
}

/// `target_to_source` maps a target pixel to source pixel coordinates. `step` ≥ 1 samples
/// every step-th pixel in both directions (2 = a quarter of the pixels, plenty for provinces).
pub fn overlap(source: &LabelRaster, target: &LabelRaster, target_to_source: &Affine, step: u32) -> OverlapCounts {
    let step = step.max(1);
    let mut out = OverlapCounts::default();
    let mut y = 0;
    while y < target.height {
        let mut x = 0;
        while x < target.width {
            let t = target.get(x, y);
            if t != 0 {
                let (sx, sy) = target_to_source.apply((f64::from(x) + 0.5, f64::from(y) + 0.5));
                if sx >= 0.0 && sy >= 0.0 {
                    let s = source.get(sx as u32, sy as u32);
                    if s != 0 {
                        *out.pairs.entry((s, t)).or_default() += 1;
                        *out.source_totals.entry(s).or_default() += 1;
                        *out.target_totals.entry(t).or_default() += 1;
                    }
                }
            }
            x += step;
        }
        y += step;
    }
    out
}

impl OverlapCounts {
    /// Links with at least `min_share` on one side (drops slivers from border noise).
    pub fn links(&self, min_share: f64) -> Vec<OverlapLink> {
        self.pairs
            .iter()
            .map(|(&(s, t), &n)| OverlapLink {
                source: s,
                target: t,
                share_of_source: n as f64 / self.source_totals[&s] as f64,
                share_of_target: n as f64 / self.target_totals[&t] as f64,
            })
            .filter(|l| l.share_of_source >= min_share || l.share_of_target >= min_share)
            .collect()
    }

    /// Source units no sample reached (too small for the step, or off the aligned area).
    pub fn unmatched_sources<'a>(&self, all: impl IntoIterator<Item = &'a u32>) -> Vec<u32> {
        all.into_iter().filter(|s| !self.source_totals.contains_key(s)).copied().collect()
    }
}
