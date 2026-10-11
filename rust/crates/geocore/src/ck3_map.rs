//! Crusader Kings III map input for GeoCore, read from the player's own install at run time:
//! `game/map_data/definition.csv` (`id;R;G;B;name;x;`), `game/map_data/provinces.png`
//! and `game/common/landed_titles/*.txt` (baronies carry `province = <id>`).
//! (Paths and formats as documented for CK3 modding; confirm on the player's version.)

use std::collections::{BTreeMap, HashMap};

use paradox_parser::{Container, Value};

/// Colour → province id from definition.csv. Header/comment lines and id 0 are skipped.
pub fn definition_colors(csv: &str) -> HashMap<[u8; 3], u32> {
    csv.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let f: Vec<&str> = l.split(';').collect();
            if f.len() < 4 {
                return None;
            }
            let id: u32 = f[0].trim().parse().ok()?;
            let rgb = [f[1].trim().parse().ok()?, f[2].trim().parse().ok()?, f[3].trim().parse().ok()?];
            (id != 0).then_some((rgb, id))
        })
        .collect()
}

/// Province id → county key, from landed_titles (any nesting e_ → k_ → d_ → c_ → b_).
pub fn province_counties(landed_titles: &str) -> BTreeMap<u32, String> {
    fn walk(c: &Container<'_>, county: Option<&str>, out: &mut BTreeMap<u32, String>) {
        for (key, _, value) in c.pairs() {
            let k = key.as_str();
            let Value::Container(inner) = value else { continue };
            let here = if k.starts_with("c_") { Some(k.as_ref()) } else { county };
            if k.starts_with("b_") {
                if let (Some(cty), Some(p)) = (here, inner.get("province").and_then(|v| v.as_scalar()).and_then(|s| s.as_str().parse::<u32>().ok())) {
                    out.insert(p, cty.to_string());
                }
            }
            walk(inner, here, out);
        }
    }
    let mut out = BTreeMap::new();
    if let Ok(parsed) = paradox_parser::parse(landed_titles.as_bytes()) {
        walk(&parsed.root, None, &mut out);
    }
    out
}

/// Numbered county labels for a raster relabel: (province → county index, index → key).
pub fn county_labels(provinces: &BTreeMap<u32, String>) -> (HashMap<u32, u32>, Vec<String>) {
    let mut keys: Vec<String> = provinces.values().cloned().collect();
    keys.sort();
    keys.dedup();
    let index: HashMap<&str, u32> = keys.iter().enumerate().map(|(i, k)| (k.as_str(), i as u32 + 1)).collect();
    let map = provinces.iter().map(|(p, c)| (*p, index[c.as_str()])).collect();
    (map, keys)
}
