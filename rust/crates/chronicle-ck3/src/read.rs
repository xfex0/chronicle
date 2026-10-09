//! Read a CK3 save into a small, game-shaped model. Paths verified on 1.0.2
//! (games/ck3/SAVE_FORMAT.md). Only the parts Chronicle needs are parsed; the big `living`
//! section is indexed and only the needed characters are parsed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chronicle_core::PartialDate;
use paradox_parser::container::{ContainerKind, Encoding, archive};
use paradox_parser::index::{SectionSpan, parse_section};
use paradox_parser::{Container, ParseOptions, TopLevelIndex, Value, detect, source};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Ck3Error {
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("save format error: {0}")]
    Parse(#[from] paradox_parser::ParseError),
    #[error("binary (ironman) CK3 saves are not supported; use a normal save")]
    Binary,
    #[error("not a CK3 save: {0}")]
    NotCk3(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryKind {
    Holder,
    Created,
    Destroyed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TitleHistory {
    pub date: PartialDate,
    pub kind: HistoryKind,
    pub holder: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Ck3Title {
    pub id: String,
    pub key: String,
    pub name: String,
    pub holder: Option<String>,
    pub liege: Option<String>,
    pub capital: Option<String>,
    pub history: Vec<TitleHistory>,
}

impl Ck3Title {
    /// 4 empire, 3 kingdom, 2 duchy, 1 county, 0 barony, -1 other (dynamic x_ titles).
    pub fn tier(&self) -> i8 {
        match self.key.get(..2) {
            Some("e_") => 4,
            Some("k_") => 3,
            Some("d_") => 2,
            Some("c_") => 1,
            Some("b_") => 0,
            _ => -1,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Ck3County {
    pub key: String,
    pub development: f64,
    pub control: f64,
    pub culture: Option<String>,
    pub faith: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Ck3Character {
    pub id: String,
    pub name: String,
    pub birth: Option<PartialDate>,
    pub female: bool,
    pub culture: Option<String>,
    pub faith: Option<String>,
    pub house: Option<String>,
    pub domain: Vec<String>,
    pub laws: Vec<String>,
    pub became_ruler: Option<PartialDate>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Ck3House {
    pub id: String,
    pub name: String,
    pub dynasty: Option<String>,
    pub founded: Option<PartialDate>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ck3Realm {
    pub ruler: String,
    pub primary_title: String,
    pub counties: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Ck3World {
    pub version: Option<String>,
    pub date: Option<PartialDate>,
    pub bookmark: Option<PartialDate>,
    pub player: Option<String>,
    pub player_name: Option<String>,
    pub player_title_name: Option<String>,
    pub titles: BTreeMap<String, Ck3Title>,
    pub counties: BTreeMap<String, Ck3County>,
    pub characters: BTreeMap<String, Ck3Character>,
    pub houses: BTreeMap<String, Ck3House>,
    pub cultures: BTreeMap<String, String>,
    pub faiths: BTreeMap<String, String>,
    pub realms: Vec<Ck3Realm>,
}

fn inner<'a>(src: &'a [u8], span: &SectionSpan) -> &'a [u8] {
    if !span.is_block {
        return &src[span.value_start..span.value_end];
    }
    let start = (span.value_start + 1).min(src.len());
    let end = span.value_end.saturating_sub(1).max(start).min(src.len());
    &src[start..end]
}

fn text(v: Option<&Value<'_>>) -> Option<String> {
    v.and_then(|v| v.as_scalar()).map(|s| s.as_str().into_owned())
}

fn num(v: Option<&Value<'_>>) -> Option<f64> {
    v.and_then(|v| v.as_scalar()).and_then(|s| s.as_f64())
}

fn date(v: Option<&Value<'_>>) -> Option<PartialDate> {
    text(v).and_then(|s| s.parse().ok())
}

fn container<'c, 'a>(v: Option<&'c Value<'a>>) -> Option<&'c Container<'a>> {
    v.and_then(|v| v.as_container())
}

fn strings(c: Option<&Container<'_>>) -> Vec<String> {
    c.map(|c| c.items().filter_map(|v| v.as_scalar()).map(|s| s.as_str().into_owned()).collect())
        .unwrap_or_default()
}

/// The gamestate text from a `.ck3` file (header + text meta + ZIP, or plain text).
pub fn gamestate_bytes(path: &Path) -> Result<Vec<u8>, Ck3Error> {
    let bytes = source::open(path)?;
    let d = detect(&bytes);
    if !d.header_line.as_deref().is_some_and(|h| h.starts_with("SAV")) {
        return Err(Ck3Error::NotCk3("missing SAV header".into()));
    }
    match d.container {
        ContainerKind::Zip => {
            let at = d.archive_offset.unwrap_or(d.payload_offset);
            Ok(archive::read_entry(&bytes[at..], "gamestate")?)
        }
        ContainerKind::Plain if d.encoding == Encoding::Binary => Err(Ck3Error::Binary),
        ContainerKind::Plain => Ok(bytes[d.payload_offset..].to_vec()),
        other => Err(Ck3Error::NotCk3(format!("{other:?} container"))),
    }
}

pub fn read_ck3(path: &Path) -> Result<Ck3World, Ck3Error> {
    let g = gamestate_bytes(path)?;
    if paradox_parser::container::sniff_encoding(&g) == Encoding::Binary {
        return Err(Ck3Error::Binary);
    }
    read_gamestate(&g)
}

pub fn read_gamestate(g: &[u8]) -> Result<Ck3World, Ck3Error> {
    let opts = ParseOptions::default();
    let idx = TopLevelIndex::build(g)?;
    let section = |key: &str| idx.find(key).next();
    let mut w = Ck3World::default();

    if let Some(span) = section("meta_data") {
        let m = parse_section(g, span, &opts)?;
        w.version = text(m.root.get("version"));
        w.player_name = text(m.root.get("meta_player_name"));
        w.player_title_name = text(m.root.get("meta_title_name"));
    }
    let scalar_date = |key: &str| {
        section(key)
            .filter(|s| !s.is_block)
            .and_then(|s| String::from_utf8_lossy(&g[s.value_start..s.value_end]).parse::<PartialDate>().ok())
    };
    w.date = scalar_date("date");
    w.bookmark = scalar_date("bookmark_date");
    if let Some(span) = section("played_character") {
        w.player = text(parse_section(g, span, &opts)?.root.get("character"));
    }

    // titles
    let lt_span = section("landed_titles").ok_or_else(|| Ck3Error::NotCk3("no landed_titles".into()))?;
    let lt = parse_section(g, lt_span, &opts)?;
    if let Some(titles) = container(lt.root.get("landed_titles")) {
        for (id, _, v) in titles.pairs() {
            let Some(t) = v.as_container() else { continue };
            let mut history = Vec::new();
            if let Some(h) = container(t.get("history")) {
                for (d, _, hv) in h.pairs() {
                    let Ok(date) = d.as_str().parse::<PartialDate>() else { continue };
                    match hv {
                        Value::Scalar(s) => {
                            history.push(TitleHistory { date, kind: HistoryKind::Holder, holder: Some(s.as_str().into_owned()) })
                        }
                        other => {
                            let c = other.as_container();
                            let kind = match c.and_then(|c| text(c.get("type"))).as_deref() {
                                Some("created") => HistoryKind::Created,
                                Some("destroyed") => HistoryKind::Destroyed,
                                _ => HistoryKind::Holder,
                            };
                            history.push(TitleHistory { date, kind, holder: c.and_then(|c| text(c.get("holder"))) });
                        }
                    }
                }
            }
            let id = id.as_str().into_owned();
            w.titles.insert(
                id.clone(),
                Ck3Title {
                    id,
                    key: text(t.get("key")).unwrap_or_default(),
                    name: text(t.get("name")).unwrap_or_default(),
                    holder: text(t.get("holder")),
                    liege: text(t.get("de_facto_liege")),
                    capital: text(t.get("capital")),
                    history,
                },
            );
        }
    }

    // counties
    if let Some(span) = section("county_manager") {
        let cm = parse_section(g, span, &opts)?;
        if let Some(counties) = container(cm.root.get("counties")) {
            for (key, _, v) in counties.pairs() {
                let Some(c) = v.as_container() else { continue };
                let key = key.as_str().into_owned();
                w.counties.insert(
                    key.clone(),
                    Ck3County {
                        key,
                        development: num(c.get("development")).unwrap_or(0.0),
                        control: num(c.get("county_control")).unwrap_or(0.0),
                        culture: text(c.get("culture")),
                        faith: text(c.get("faith")),
                    },
                );
            }
        }
    }

    // realms (needs titles + counties)
    let groups = realm_groups(&w);

    // characters: only realm rulers + the player
    let mut needed: BTreeSet<String> = groups.keys().cloned().collect();
    if let Some(p) = &w.player {
        needed.insert(p.clone());
    }
    if let Some(span) = section("living") {
        let liv = inner(g, span);
        let lidx = TopLevelIndex::build(liv)?;
        let by_id: BTreeMap<&str, &SectionSpan> = lidx.spans.iter().map(|s| (s.key.as_str(), s)).collect();
        for id in &needed {
            let Some(span) = by_id.get(id.as_str()) else { continue };
            let c = parse_section(liv, span, &opts)?;
            let ld = container(c.root.get("landed_data"));
            w.characters.insert(
                id.clone(),
                Ck3Character {
                    id: id.clone(),
                    name: text(c.root.get("first_name")).unwrap_or_default(),
                    birth: date(c.root.get("birth")),
                    female: c.root.get("female").and_then(|v| v.as_scalar()).and_then(|s| s.as_bool()).unwrap_or(false),
                    culture: text(c.root.get("culture")),
                    faith: text(c.root.get("faith")),
                    house: text(c.root.get("dynasty_house")),
                    domain: strings(ld.and_then(|l| container(l.get("domain")))),
                    laws: strings(ld.and_then(|l| container(l.get("laws")))),
                    became_ruler: ld.and_then(|l| date(l.get("became_ruler_date"))),
                },
            );
        }
    }

    // primary titles: highest tier liege-less title of the ruler, ties by domain order
    let mut realms = Vec::new();
    for (ruler, counties) in groups {
        let domain = w.characters.get(&ruler).map(|c| c.domain.clone()).unwrap_or_default();
        let mut candidates: Vec<&Ck3Title> = w
            .titles
            .values()
            .filter(|t| t.holder.as_deref() == Some(ruler.as_str()) && t.liege.is_none() && t.tier() >= 1)
            .collect();
        candidates.sort_by_key(|t| {
            let pos = domain.iter().position(|d| *d == t.id).unwrap_or(usize::MAX);
            (std::cmp::Reverse(t.tier()), pos, t.id.parse::<u64>().unwrap_or(u64::MAX))
        });
        let primary = candidates.first().map(|t| t.id.clone()).unwrap_or_else(|| counties[0].clone());
        realms.push(Ck3Realm { ruler, primary_title: primary, counties });
    }
    realms.sort_by(|a, b| b.counties.len().cmp(&a.counties.len()).then(a.primary_title.cmp(&b.primary_title)));
    w.realms = realms;

    // houses, cultures, faiths (only what is referenced)
    let houses: BTreeSet<String> = w.characters.values().filter_map(|c| c.house.clone()).collect();
    if let Some(span) = section("dynasties") {
        let ds = inner(g, span);
        let didx = TopLevelIndex::build(ds)?;
        if let Some(hs) = didx.find("dynasty_house").next() {
            let h = parse_section(ds, hs, &opts)?;
            for (id, _, v) in h.root.pairs() {
                let id = id.as_str().into_owned();
                if !houses.contains(&id) {
                    continue;
                }
                let Some(c) = v.as_container() else { continue };
                let name = text(c.get("localized_name")).or_else(|| text(c.get("name"))).unwrap_or_default();
                w.houses.insert(
                    id.clone(),
                    Ck3House {
                        id,
                        name,
                        dynasty: text(c.get("dynasty")),
                        founded: date(c.get("found_date")).filter(|d| d.year < 9999),
                    },
                );
            }
        }
    }
    if let Some(span) = section("culture_manager") {
        let cm = parse_section(g, span, &opts)?;
        if let Some(cs) = container(cm.root.get("cultures")) {
            for (id, _, v) in cs.pairs() {
                if let Some(t) = container(Some(v)).and_then(|c| text(c.get("culture_template"))) {
                    w.cultures.insert(id.as_str().into_owned(), t);
                }
            }
        }
    }
    if let Some(span) = section("religion") {
        let r = parse_section(g, span, &opts)?;
        if let Some(fs) = container(r.root.get("faiths")) {
            for (id, _, v) in fs.pairs() {
                if let Some(t) = container(Some(v)).and_then(|c| text(c.get("template")).or_else(|| text(c.get("tag")))) {
                    w.faiths.insert(id.as_str().into_owned(), t);
                }
            }
        }
    }
    Ok(w)
}

/// Group counties by the ruler at the top of their de facto liege chain.
fn realm_groups(w: &Ck3World) -> BTreeMap<String, Vec<String>> {
    let by_key: BTreeMap<&str, &Ck3Title> = w.titles.values().map(|t| (t.key.as_str(), t)).collect();
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for key in w.counties.keys() {
        let Some(county) = by_key.get(key.as_str()) else { continue };
        let Some(county_holder) = county.holder.clone() else { continue };
        let mut top = *county;
        for _ in 0..32 {
            match top.liege.as_ref().and_then(|l| w.titles.get(l)) {
                Some(next) => top = next,
                None => break,
            }
        }
        let ruler = top.holder.clone().unwrap_or(county_holder);
        groups.entry(ruler).or_default().push(key.clone());
    }
    groups
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const FIXTURE: &[u8] = include_bytes!("../../../../tests/fixtures/ck3/1.0.2/mini_gamestate.txt");

    #[test]
    fn reads_structure_verified_on_1_0_2() {
        let w = read_gamestate(FIXTURE).unwrap();
        assert_eq!(w.version.as_deref(), Some("1.0.2"));
        assert_eq!(w.date, Some(PartialDate::ymd(923, 4, 28)));
        assert_eq!(w.bookmark, Some(PartialDate::ymd(867, 1, 1)));
        assert_eq!(w.player.as_deref(), Some("100"));
        assert_eq!(w.titles.len(), 8);
        let k = &w.titles["1"];
        assert_eq!((k.key.as_str(), k.tier()), ("k_gujarat", 3));
        assert_eq!(k.history.len(), 4);
        assert_eq!(k.history[1].kind, HistoryKind::Destroyed);
        assert_eq!(k.history[2].holder.as_deref(), Some("7001"));
        assert_eq!(w.counties["c_b"].control, 80.0);
        assert_eq!(w.cultures["75"], "gujarati");
        assert_eq!(w.faiths["42"], "smartism");
        assert_eq!(w.houses["10475"].name, "Расаладевииды");
        assert_eq!(w.houses["11"].founded, None, "9999.1.1 means 'not founded in history'");
        assert_eq!(w.characters["100"].laws.last().map(String::as_str), Some("crown_authority_1"));
    }

    #[test]
    fn realms_follow_de_facto_lieges() {
        let w = read_gamestate(FIXTURE).unwrap();
        assert_eq!(w.realms.len(), 2);
        let player = &w.realms[0];
        assert_eq!(player.ruler, "100");
        assert_eq!(w.titles[&player.primary_title].key, "k_gujarat", "ties broken by domain order");
        assert_eq!(player.counties, ["c_a", "c_b"], "c_b is held by a vassal duke");
        assert_eq!(w.titles[&w.realms[1].primary_title].key, "c_c");
    }

    #[test]
    fn zipped_save_with_header_like_the_game() {
        use std::io::Write;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("save.ck3");
        let meta = b"meta_data={\n\tversion=\"1.0.2\"\n}\n";
        let mut file = Vec::new();
        file.extend_from_slice(format!("SAV0102e36d7ab2{:08x}\n", meta.len()).as_bytes());
        file.extend_from_slice(meta);
        let mut zbuf = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut zbuf);
            zw.start_file("gamestate", zip::write::SimpleFileOptions::default()).unwrap();
            zw.write_all(FIXTURE).unwrap();
            zw.finish().unwrap();
        }
        file.extend_from_slice(zbuf.get_ref());
        std::fs::write(&path, file).unwrap();
        let w = read_ck3(&path).unwrap();
        assert_eq!(w.realms.len(), 2);
    }

    #[test]
    fn rejects_other_files() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("x.ck3");
        std::fs::write(&p, b"version=\"Circinus v3.14.15\"").unwrap();
        assert!(matches!(read_ck3(&p), Err(Ck3Error::NotCk3(_))));
    }
}
