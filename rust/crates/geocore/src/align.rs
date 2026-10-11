//! Map alignment: an affine transform fitted to control points (the same real place picked on
//! both maps — a cape, a river mouth, a city). Least squares, so more points average out
//! picking errors. Different projections are handled per region with their own points.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ControlPoint {
    /// Pixel on the map we sample (usually the target map).
    pub from: (f64, f64),
    /// The same place on the other map.
    pub to: (f64, f64),
}

/// (x, y) → (a·x + b·y + c, d·x + e·y + f)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 0.0, e: 1.0, f: 0.0 };

    pub fn apply(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (self.a * x + self.b * y + self.c, self.d * x + self.e * y + self.f)
    }

    /// Least-squares fit; needs at least 3 points not on one line.
    pub fn fit(points: &[ControlPoint]) -> Option<Affine> {
        if points.len() < 3 {
            return None;
        }
        // normal equations: (MᵀM) p = Mᵀt, rows of M = [x y 1]
        let mut m = [[0.0f64; 3]; 3];
        let mut tx = [0.0f64; 3];
        let mut ty = [0.0f64; 3];
        for p in points {
            let row = [p.from.0, p.from.1, 1.0];
            for i in 0..3 {
                for j in 0..3 {
                    m[i][j] += row[i] * row[j];
                }
                tx[i] += row[i] * p.to.0;
                ty[i] += row[i] * p.to.1;
            }
        }
        let px = solve3(m, tx)?;
        let py = solve3(m, ty)?;
        Some(Affine { a: px[0], b: px[1], c: px[2], d: py[0], e: py[1], f: py[2] })
    }

    /// Largest distance between a point's mapped position and where it should be (pixels).
    pub fn max_error(&self, points: &[ControlPoint]) -> f64 {
        points
            .iter()
            .map(|p| {
                let (x, y) = self.apply(p.from);
                ((x - p.to.0).powi(2) + (y - p.to.1).powi(2)).sqrt()
            })
            .fold(0.0, f64::max)
    }
}

/// Gaussian elimination with partial pivoting; None when singular.
fn solve3(mut m: [[f64; 3]; 3], mut v: [f64; 3]) -> Option<[f64; 3]> {
    for col in 0..3 {
        let pivot = (col..3).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[pivot][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivot);
        v.swap(col, pivot);
        for row in col + 1..3 {
            let k = m[row][col] / m[col][col];
            for j in col..3 {
                m[row][j] -= k * m[col][j];
            }
            v[row] -= k * v[col];
        }
    }
    let mut x = [0.0f64; 3];
    for i in (0..3).rev() {
        let s: f64 = (i + 1..3).map(|j| m[i][j] * x[j]).sum();
        x[i] = (v[i] - s) / m[i][i];
    }
    Some(x)
}
