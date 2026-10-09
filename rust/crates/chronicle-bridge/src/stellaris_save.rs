//! Read the player's empire from a Stellaris save — the first reader built on a real save.
//!
//! Verified on Stellaris 3.14.15 "Circinus" (see games/stellaris/SAVE_FORMAT.md):
//! a ZIP with `gamestate` + `meta`, plain text even for ironman;
//! `player={ { country=<id> } }`; `country.<id>.ethos.ethic` (repeated);
//! `country.<id>.government.{type, authority, civics{}, origin}`; `country.<id>.name.key`.
//!
//! Used to compare the empire the Modern Era Bridge suggests with the one actually played.

use std::collections::BTreeSet;
use std::path::Path;

use paradox_parser::container::{ContainerKind, Encoding, archive};
use paradox_parser::index::parse_section;
use paradox_parser::{Container, ParseOptions, TopLevelIndex, Value, detect, parse, source};
use serde::Serialize;

use crate::config::Vocabulary;
use crate::design::EmpireDesign;

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("save format error: {0}")]
    Parse(#[from] paradox_parser::ParseError),
    #[error("not a Stellaris save: {0}")]
    NotStellaris(String),
    #[error("binary saves are not supported yet")]
    Binary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StellarisEmpire {
    pub name: String,
    /// True when the name is literal text; false for a localisation key.
    pub name_is_literal: bool,
    pub game_version: Option<String>,
    pub date: Option<String>,
    pub ironman: Option<bool>,
    pub country_id: String,
    pub government_type: Option<String>,
    /// Raw game keys, e.g. "auth_imperial", "ethic_fanatic_spiritualist", "civic_mining_guilds".
    pub raw_authority: Option<String>,
    pub raw_ethics: Vec<String>,
    pub raw_civics: Vec<String>,
    pub raw_origin: Option<String>,
    /// Same values in Chronicle vocabulary terms ("imperial", "fanatic_spiritualist", ...).
    pub authority: Option<String>,
    pub ethics: Vec<String>,
    pub civics: Vec<String>,
    pub origin: Option<String>,
}

fn text(v: Option<&Value<'_>>) -> Option<String> {
    v.and_then(|v| v.as_scalar()).map(|s| s.as_str().into_owned())
}

fn strings(c: Option<&Container<'_>>) -> Vec<String> {
    c.map(|c| c.items().filter_map(|v| v.as_scalar()).map(|s| s.as_str().into_owned()).collect())
        .unwrap_or_default()
}

/// Read from a `.sav` file (ZIP or plain text).
pub fn read_empire(path: &Path) -> Result<StellarisEmpire, ReadError> {
    let bytes = source::open(path)?;
    let d = detect(&bytes);
    match d.container {
        ContainerKind::Zip => {
            let at = d.archive_offset.unwrap_or(0);
            let gamestate = archive::read_entry(&bytes[at..], "gamestate")?;
            let meta = archive::read_entry(&bytes[at..], "meta").ok();
            read_empire_from_text(meta.as_deref(), &gamestate)
        }
        ContainerKind::Plain if d.encoding == Encoding::Binary => Err(ReadError::Binary),
        ContainerKind::Plain => read_empire_from_text(None, &bytes[d.payload_offset..]),
        other => Err(ReadError::NotStellaris(format!("{other:?} container"))),
    }
}

pub fn read_empire_from_text(meta: Option<&[u8]>, gamestate: &[u8]) -> Result<StellarisEmpire, ReadError> {
    let opts = ParseOptions::default();
    let idx = TopLevelIndex::build(gamestate)?;
    let top_scalar = |key: &str| -> Option<String> {
        let span = idx.find(key).next()?;
        if span.is_block {
            return None;
        }
        let raw = String::from_utf8_lossy(&gamestate[span.value_start..span.value_end]).into_owned();
        Some(raw.trim_matches('"').to_string())
    };
    let game_version = top_scalar("version");
    let date = top_scalar("date");
    let ironman = match meta {
        Some(m) => parse(m)?.root.get("ironman").and_then(|v| v.as_scalar()).and_then(|s| s.as_bool()),
        None => None,
    };

    let player_span = idx.find("player").next().ok_or_else(|| ReadError::NotStellaris("no player section".into()))?;
    let player = parse_section(gamestate, player_span, &opts)?;
    let country_id = player
        .root
        .items()
        .filter_map(|v| v.as_container())
        .find_map(|p| text(p.get("country")))
        .ok_or_else(|| ReadError::NotStellaris("player has no country".into()))?;

    let country_span =
        idx.find("country").next().ok_or_else(|| ReadError::NotStellaris("no country section".into()))?;
    let countries = parse_section(gamestate, country_span, &opts)?;
    let c = countries
        .root
        .get(&country_id)
        .and_then(|v| v.as_container())
        .ok_or_else(|| ReadError::NotStellaris(format!("country {country_id} not found")))?;

    let name_block = c.get("name").and_then(|v| v.as_container());
    let name = name_block.and_then(|n| text(n.get("key"))).unwrap_or_default();
    let name_is_literal = name_block
        .and_then(|n| n.get("literal"))
        .and_then(|v| v.as_scalar())
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    let gov = c.get("government").and_then(|v| v.as_container());
    let raw_ethics: Vec<String> = c
        .get("ethos")
        .and_then(|v| v.as_container())
        .map(|e| e.get_all("ethic").filter_map(|v| v.as_scalar()).map(|s| s.as_str().into_owned()).collect())
        .unwrap_or_default();
    let raw_authority = gov.and_then(|g| text(g.get("authority")));
    let raw_civics = strings(gov.and_then(|g| g.get("civics")).and_then(|v| v.as_container()));
    let raw_origin = gov.and_then(|g| text(g.get("origin")));
    let government_type = gov.and_then(|g| text(g.get("type")));

    let vocab = Vocabulary::builtin();
    let origin = raw_origin.as_ref().map(|o| {
        vocab
            .origins
            .iter()
            .find(|(_, def)| def.key == *o)
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| o.trim_start_matches("origin_").to_string())
    });

    Ok(StellarisEmpire {
        name,
        name_is_literal,
        game_version,
        date,
        ironman,
        country_id,
        government_type,
        authority: raw_authority.as_ref().map(|a| a.trim_start_matches("auth_").to_string()),
        ethics: raw_ethics.iter().map(|e| e.trim_start_matches("ethic_").to_string()).collect(),
        civics: raw_civics.iter().map(|c| c.trim_start_matches("civic_").to_string()).collect(),
        origin,
        raw_authority,
        raw_ethics,
        raw_civics,
        raw_origin,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DesignComparison {
    pub authority_matches: bool,
    /// Ethics shared ignoring fanaticism (e.g. "militarist").
    pub shared_ethics: Vec<String>,
    pub shared_civics: Vec<String>,
    pub origin_matches: bool,
    /// 0..1: authority, ethics, civics and origin weigh a quarter each.
    pub score: f64,
}

/// How close the suggested design is to the empire actually played.
pub fn compare(design: &EmpireDesign, actual: &StellarisEmpire) -> DesignComparison {
    let base = |v: &[String]| -> BTreeSet<String> { v.iter().map(|e| e.trim_start_matches("fanatic_").to_string()).collect() };
    let (d_eth, a_eth) = (base(&design.ethics), base(&actual.ethics));
    let shared_ethics: Vec<String> = d_eth.intersection(&a_eth).cloned().collect();
    let d_civ: BTreeSet<&String> = design.civics.iter().collect();
    let shared_civics: Vec<String> = actual.civics.iter().filter(|c| d_civ.contains(c)).cloned().collect();
    let authority_matches = actual.authority.as_deref() == Some(design.authority.as_str());
    let origin_matches = actual.origin.as_deref() == Some(design.origin.as_str());
    let ethic_part = shared_ethics.len() as f64 / d_eth.len().max(a_eth.len()).max(1) as f64;
    let civic_part = shared_civics.len() as f64 / 2.0;
    let score = (f64::from(u8::from(authority_matches)) + ethic_part + civic_part.min(1.0) + f64::from(u8::from(origin_matches))) / 4.0;
    DesignComparison { authority_matches, shared_ethics, shared_civics, origin_matches, score }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../../../tests/fixtures/stellaris/3.14.15/mini_gamestate.txt");
    const META: &[u8] = b"version=\"Circinus v3.14.15\"\ndate=\"2404.11.09\"\nironman=yes\n";

    #[test]
    fn reads_player_empire_from_3_14_structure() {
        let e = read_empire_from_text(Some(META), FIXTURE).unwrap();
        assert_eq!(e.country_id, "0");
        assert_eq!(e.name, "Юґґот");
        assert!(e.name_is_literal);
        assert_eq!(e.game_version.as_deref(), Some("Circinus v3.14.15"));
        assert_eq!(e.date.as_deref(), Some("2404.11.09"));
        assert_eq!(e.ironman, Some(true));
        assert_eq!(e.raw_authority.as_deref(), Some("auth_imperial"));
        assert_eq!(e.authority.as_deref(), Some("imperial"));
        assert_eq!(e.ethics, ["militarist", "fanatic_spiritualist"]);
        assert_eq!(e.civics, ["mining_guilds", "nationalistic_zeal"]);
        assert_eq!(e.raw_origin.as_deref(), Some("origin_necrophage"));
        assert_eq!(e.origin.as_deref(), Some("necrophage"));
        assert_eq!(e.government_type.as_deref(), Some("gov_star_empire"));
    }

    #[test]
    fn origin_default_maps_to_vocabulary_key() {
        let doc = String::from_utf8_lossy(FIXTURE).replace("country=0", "country=1");
        let e = read_empire_from_text(None, doc.as_bytes()).unwrap();
        assert_eq!(e.origin.as_deref(), Some("prosperous_unification"));
        assert_eq!(e.authority.as_deref(), Some("democratic"));
        assert!(!e.name_is_literal);
        assert_eq!(e.ironman, None);
    }

    #[test]
    fn reads_zip_save_like_the_game_writes_it() {
        use std::io::Write;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ironman.sav");
        let mut zw = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default;
        zw.start_file("gamestate", opts()).unwrap();
        zw.write_all(FIXTURE).unwrap();
        zw.start_file("meta", opts()).unwrap();
        zw.write_all(META).unwrap();
        zw.finish().unwrap();
        let e = read_empire(&path).unwrap();
        assert_eq!(e.civics, ["mining_guilds", "nationalistic_zeal"]);
    }

    #[test]
    fn comparison_with_bridge_design() {
        let actual = read_empire_from_text(Some(META), FIXTURE).unwrap();
        let start = crate::presets().into_iter().find(|(n, _)| *n == "military_dictatorship").unwrap().1;
        let r = crate::run(&start, 45_819_283).unwrap();
        let cmp = compare(&r.design, &actual);
        assert_eq!(cmp.shared_ethics, ["militarist"]);
        assert_eq!(cmp.shared_civics, ["nationalistic_zeal"]);
        assert!(!cmp.authority_matches && !cmp.origin_matches);
        assert!(cmp.score > 0.2 && cmp.score < 0.5, "{}", cmp.score);
    }

    #[test]
    fn missing_sections_are_reported() {
        assert!(matches!(read_empire_from_text(None, b"version=\"x\"\n"), Err(ReadError::NotStellaris(_))));
    }
}
