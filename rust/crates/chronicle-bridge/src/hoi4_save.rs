//! Hearts of Iron IV text saves → Modern Era Bridge input.
//!
//! Field paths verified on HoI4 1.19.3 text saves (games/hoi4/SAVE_FORMAT.md):
//! `countries.<TAG>.politics.{parties.<ideology>.popularity, ideas, ruling_party}`,
//! `countries.<TAG>.{stability, war_support, nukes{ {amount} }, technology.technologies}`,
//! `countries.<TAG>.diplomacy.active_relations.<TAG>.war_relation.{first, second}`,
//! `states.<id>.{owner, buildings.{industrial_complex, arms_factory, dockyard}.level}`,
//! top-level `faction={ name ideology members{} }` (repeated).
//! Binary (`HOI4bin`) saves are refused: their token tables cannot be distributed.
//!
//! Mapping weights: games/hoi4/semantic_mapping.yaml. Research twin with the same formulas:
//! tools/research (hoi4 prototype).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use paradox_parser::container::Encoding;
use paradox_parser::index::{SectionSpan, parse_section};
use paradox_parser::{Container, ParseOptions, TopLevelIndex, Value, detect, source};
use serde::{Deserialize, Serialize};

use crate::state::{CivilizationState, Ideology};

#[derive(Debug, thiserror::Error)]
pub enum Hoi4Error {
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("save format error: {0}")]
    Parse(#[from] paradox_parser::ParseError),
    #[error("binary HoI4 save: set save_as_binary=no in settings.txt and save again")]
    Binary,
    #[error("not a HoI4 save: {0}")]
    NotHoi4(String),
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Hoi4Country {
    pub tag: String,
    pub ruling_party: Option<String>,
    pub popularity: BTreeMap<String, f64>,
    pub ideas: Vec<String>,
    pub technologies: Vec<String>,
    pub nukes: f64,
    pub stability: f64,
    pub war_support: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Factories {
    pub civilian: u32,
    pub military: u32,
    pub dockyards: u32,
    pub states: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hoi4Faction {
    pub name: String,
    pub ideology: Option<String>,
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hoi4World {
    pub version: Option<String>,
    pub date: Option<String>,
    pub player: Option<String>,
    /// Countries that have a government, in file order.
    pub countries: Vec<Hoi4Country>,
    pub factories: BTreeMap<String, Factories>,
    pub factions: Vec<Hoi4Faction>,
    pub wars: BTreeSet<(String, String)>,
}

fn inner<'a>(src: &'a [u8], span: &SectionSpan) -> &'a [u8] {
    if !span.is_block {
        return &src[span.value_start..span.value_end];
    }
    let start = (span.value_start + 1).min(src.len());
    let end = span.value_end.saturating_sub(1).max(start).min(src.len());
    &src[start..end]
}

fn span_text(src: &[u8], span: &SectionSpan) -> String {
    String::from_utf8_lossy(&src[span.value_start..span.value_end]).trim_matches('"').to_string()
}

fn text(v: Option<&Value<'_>>) -> Option<String> {
    v.and_then(|v| v.as_scalar()).map(|s| s.as_str().into_owned())
}

fn num(v: Option<&Value<'_>>) -> Option<f64> {
    v.and_then(|v| v.as_scalar()).and_then(|s| s.as_f64())
}

fn container<'c, 'a>(v: Option<&'c Value<'a>>) -> Option<&'c Container<'a>> {
    v.and_then(|v| v.as_container())
}

pub fn read_hoi4(path: &Path) -> Result<Hoi4World, Hoi4Error> {
    let bytes = source::open(path)?;
    read_hoi4_bytes(&bytes)
}

pub fn read_hoi4_bytes(bytes: &[u8]) -> Result<Hoi4World, Hoi4Error> {
    let d = detect(bytes);
    match d.header_line.as_deref() {
        Some("HOI4bin") => return Err(Hoi4Error::Binary),
        Some("HOI4txt") => {}
        _ => return Err(Hoi4Error::NotHoi4("missing HOI4txt header".into())),
    }
    if d.encoding == Encoding::Binary {
        return Err(Hoi4Error::Binary);
    }
    let payload = &bytes[d.payload_offset..];
    let opts = ParseOptions::default();
    let idx = TopLevelIndex::build(payload)?;
    let scalar = |key: &str| idx.find(key).next().filter(|s| !s.is_block).map(|s| span_text(payload, s));

    // countries: index each country block, parse only the parts we need
    let countries_span =
        idx.find("countries").next().ok_or_else(|| Hoi4Error::NotHoi4("no countries section".into()))?;
    let countries_src = inner(payload, countries_span);
    let cidx = TopLevelIndex::build(countries_src)?;
    let mut countries = Vec::new();
    let mut wars = BTreeSet::new();
    for cspan in cidx.spans.iter().filter(|s| s.is_block) {
        let blk = inner(countries_src, cspan);
        let sidx = TopLevelIndex::build(blk)?;
        let Some(pol_span) = sidx.find("politics").next() else { continue };
        let pol = parse_section(blk, pol_span, &opts)?;
        let mut c = Hoi4Country { tag: cspan.key.clone(), ..Default::default() };
        if let Some(parties) = container(pol.root.get("parties")) {
            for (name, _, v) in parties.pairs() {
                let pop = container(Some(v)).and_then(|p| num(p.get("popularity"))).unwrap_or(0.0);
                c.popularity.insert(name.as_str().into_owned(), pop);
            }
        }
        c.ideas = container(pol.root.get("ideas"))
            .map(|i| i.items().filter_map(|v| v.as_scalar()).map(|s| s.as_str().into_owned()).collect())
            .unwrap_or_default();
        c.ruling_party = text(pol.root.get("ruling_party"));
        let scalar_f = |key: &str| sidx.find(key).next().and_then(|s| span_text(blk, s).parse::<f64>().ok());
        c.stability = scalar_f("stability").unwrap_or(0.0);
        c.war_support = scalar_f("war_support").unwrap_or(0.0);
        if let Some(span) = sidx.find("technology").next() {
            let t = parse_section(blk, span, &opts)?;
            if let Some(techs) = container(t.root.get("technologies")) {
                c.technologies = techs.pairs().map(|(k, _, _)| k.as_str().into_owned()).collect();
            }
        }
        if let Some(span) = sidx.find("nukes").next() {
            let n = parse_section(blk, span, &opts)?;
            c.nukes = n.root.items().filter_map(|v| v.as_container()).filter_map(|n| num(n.get("amount"))).sum();
        }
        if let Some(span) = sidx.find("diplomacy").next() {
            let dip = parse_section(blk, span, &opts)?;
            if let Some(rel) = container(dip.root.get("active_relations")) {
                for (_, _, v) in rel.pairs() {
                    if let Some(w) = container(Some(v)).and_then(|r| container(r.get("war_relation"))) {
                        if let (Some(a), Some(b)) = (text(w.get("first")), text(w.get("second"))) {
                            wars.insert((a, b));
                        }
                    }
                }
            }
        }
        countries.push(c);
    }

    // states → factories by owner
    let mut factories: BTreeMap<String, Factories> = BTreeMap::new();
    if let Some(span) = idx.find("states").next() {
        let states = parse_section(payload, span, &opts)?;
        for (_, _, v) in states.root.pairs() {
            let Some(st) = v.as_container() else { continue };
            let Some(owner) = text(st.get("owner")) else { continue };
            let f = factories.entry(owner).or_default();
            f.states += 1;
            if let Some(b) = container(st.get("buildings")) {
                let level = |k: &str| container(b.get(k)).and_then(|x| num(x.get("level"))).unwrap_or(0.0).floor() as u32;
                f.civilian += level("industrial_complex");
                f.military += level("arms_factory");
                f.dockyards += level("dockyard");
            }
        }
    }

    let mut factions = Vec::new();
    for span in idx.find("faction") {
        let f = parse_section(payload, span, &opts)?;
        factions.push(Hoi4Faction {
            name: text(f.root.get("name")).unwrap_or_default(),
            ideology: text(f.root.get("ideology")),
            members: container(f.root.get("members"))
                .map(|m| m.items().filter_map(|v| v.as_scalar()).map(|s| s.as_str().into_owned()).collect())
                .unwrap_or_default(),
        });
    }

    Ok(Hoi4World {
        version: scalar("version"),
        date: scalar("date"),
        player: scalar("player"),
        countries,
        factories,
        factions,
        wars,
    })
}

// ======================================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct Shares3 {
    #[serde(default)]
    pub military_factory_share: f64,
    #[serde(default)]
    pub war_support: f64,
    #[serde(default)]
    pub conscription: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanningWeights {
    pub communist_rule: f64,
    pub economy_law: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CooperationWeights {
    pub peace_share: f64,
    pub democratic_share: f64,
    pub largest_faction_share: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Hoi4Mapping {
    pub version: u32,
    pub ideology_values: BTreeMap<String, BTreeMap<String, f64>>,
    pub ruling_party_weight: f64,
    pub economy_law_level: BTreeMap<String, f64>,
    pub conscription_law_level: BTreeMap<String, f64>,
    pub trade_law_xenophobia: BTreeMap<String, f64>,
    pub militarism: Shares3,
    pub economic_planning: PlanningWeights,
    pub cooperation: CooperationWeights,
    pub tech_reference_count: f64,
    pub factories_per_state_reference: f64,
    pub nukes_scale: f64,
    pub space_tech_markers: Vec<String>,
    pub space_tech_reference: f64,
    pub bloc_min_share: f64,
}

pub const BUILTIN_HOI4_MAPPING: &str = include_str!("../../../../games/hoi4/semantic_mapping.yaml");

impl Hoi4Mapping {
    pub fn builtin() -> &'static Hoi4Mapping {
        static M: OnceLock<Hoi4Mapping> = OnceLock::new();
        M.get_or_init(|| serde_yaml::from_str(BUILTIN_HOI4_MAPPING).expect("games/hoi4/semantic_mapping.yaml is valid"))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Hoi4Summary {
    pub version: Option<String>,
    pub date: Option<String>,
    pub player: Option<String>,
    pub world_factories: u32,
    pub countries_with_industry: usize,
    /// (tag, share of world industry), largest first, top 8.
    pub top_powers: Vec<(String, f64)>,
    pub factions: Vec<(String, f64)>,
    pub countries_at_war: Vec<String>,
    /// Indicators HoI4 does not provide (neutral 0.5, lower data coverage).
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hoi4Civilization {
    pub state: CivilizationState,
    pub summary: Hoi4Summary,
}

fn law(ideas: &[String], table: &BTreeMap<String, f64>, default: f64) -> f64 {
    ideas.iter().find_map(|i| table.get(i).copied()).unwrap_or(default)
}

/// Aggregate the world into the 14 Bridge indicators (12 from HoI4; religion and environment
/// are not modelled by HoI4 and stay neutral with reduced coverage).
pub fn to_civilization(world: &Hoi4World, m: &Hoi4Mapping) -> Hoi4Civilization {
    let fac = |tag: &str| world.factories.get(tag).copied().unwrap_or_default();
    let ic: Vec<(&Hoi4Country, f64)> = world
        .countries
        .iter()
        .map(|c| (c, f64::from(fac(c.tag.as_str()).civilian + fac(c.tag.as_str()).military)))
        .filter(|(_, ic)| *ic > 0.0)
        .collect();
    let total: f64 = ic.iter().map(|(_, v)| v).sum();
    let weight = |v: f64| if total > 0.0 { v / total } else { 0.0 };
    let w_of: BTreeMap<&str, f64> = ic.iter().map(|(c, v)| (c.tag.as_str(), weight(*v))).collect();

    let mix = |ind: &str, c: &Hoi4Country| -> f64 {
        let iv = &m.ideology_values[ind];
        let s: f64 = c.popularity.values().sum();
        let s = if s == 0.0 { 1.0 } else { s };
        let pm: f64 = iv.iter().map(|(p, val)| c.popularity.get(p).copied().unwrap_or(0.0) / s * val).sum();
        let ruling = c.ruling_party.as_deref().and_then(|r| iv.get(r)).copied().unwrap_or(0.5);
        m.ruling_party_weight * ruling + (1.0 - m.ruling_party_weight) * pm
    };
    let at_war: BTreeSet<&str> = world.wars.iter().flat_map(|(a, b)| [a.as_str(), b.as_str()]).collect();

    let mut v: BTreeMap<String, f64> = BTreeMap::new();
    let sum_w = |f: &dyn Fn(&Hoi4Country, f64) -> f64| -> f64 { ic.iter().map(|(c, x)| weight(*x) * f(*c, *x)).sum() };
    v.insert("authoritarianism".into(), sum_w(&|c, _| mix("authoritarianism", c)));
    v.insert("social_equality".into(), sum_w(&|c, _| mix("social_equality", c)));
    v.insert(
        "xenophobia".into(),
        sum_w(&|c, _| (mix("xenophobia", c) + law(&c.ideas, &m.trade_law_xenophobia, 0.0)).clamp(0.0, 1.0)),
    );
    let mi = &m.militarism;
    v.insert(
        "militarism".into(),
        sum_w(&|c, icv| {
            mi.military_factory_share * f64::from(fac(c.tag.as_str()).military) / icv.max(1.0)
                + mi.war_support * c.war_support
                + mi.conscription * law(&c.ideas, &m.conscription_law_level, 0.1)
        }),
    );
    v.insert("global_wars".into(), sum_w(&|c, _| if at_war.contains(c.tag.as_str()) { 1.0 } else { 0.0 }));
    let nukes: f64 = world.countries.iter().map(|c| c.nukes).sum();
    v.insert("nuclear_weapons".into(), 1.0 - (-nukes / m.nukes_scale).exp());
    let states: u32 = world.factories.values().map(|f| f.states).sum();
    v.insert(
        "industrialization".into(),
        (total / f64::from(states.max(1)) / m.factories_per_state_reference).min(1.0),
    );
    v.insert(
        "technology".into(),
        (sum_w(&|c, _| c.technologies.len() as f64) / m.tech_reference_count).min(1.0),
    );
    v.insert(
        "space_program".into(),
        (sum_w(&|c, _| {
            c.technologies.iter().filter(|t| m.space_tech_markers.iter().any(|mk| t.contains(mk.as_str()))).count() as f64
        }) / m.space_tech_reference)
            .min(1.0),
    );
    let ep = &m.economic_planning;
    v.insert(
        "economic_planning".into(),
        sum_w(&|c, _| {
            ep.communist_rule * if c.ruling_party.as_deref() == Some("communism") { 1.0 } else { 0.0 }
                + ep.economy_law * law(&c.ideas, &m.economy_law_level, 0.1)
        }),
    );

    let shares: Vec<(String, Option<String>, f64)> = world
        .factions
        .iter()
        .map(|f| {
            let s: f64 = f.members.iter().map(|t| w_of.get(t.as_str()).copied().unwrap_or(0.0)).sum();
            (f.name.clone(), f.ideology.clone(), s)
        })
        .collect();
    let largest = shares.iter().fold(None::<&(String, Option<String>, f64)>, |best, x| match best {
        Some(b) if b.2 >= x.2 => Some(b),
        _ => Some(x),
    });
    let largest_share = largest.map_or(0.0, |l| l.2);
    v.insert("planetary_unification".into(), largest_share);
    let dem = sum_w(&|c, _| if c.ruling_party.as_deref() == Some("democratic") { 1.0 } else { 0.0 });
    let co = &m.cooperation;
    let wars_share = v["global_wars"];
    v.insert(
        "international_cooperation".into(),
        co.peace_share * (1.0 - wars_share) + co.democratic_share * dem + co.largest_faction_share * largest_share,
    );

    let blocs = shares.iter().filter(|s| s.2 >= m.bloc_min_share).count().clamp(1, 6) as u32;
    let top_country = ic.iter().fold(None::<(&Hoi4Country, f64)>, |best, (c, x)| match best {
        Some((_, bx)) if bx >= *x => best,
        _ => Some((*c, *x)),
    });
    let ideology_name = match largest {
        Some(l) if l.2 >= m.bloc_min_share => l.1.clone(),
        _ => top_country.and_then(|(c, _)| c.ruling_party.clone()),
    };
    let ideology = match ideology_name.as_deref() {
        Some("fascism") => Ideology::Fascism,
        Some("communism") => Ideology::Communism,
        Some("democratic") => Ideology::Democracy,
        _ => Ideology::NonAligned,
    };

    let state = CivilizationState::from_semantics(&v, blocs, ideology);
    let mut top: Vec<(String, f64)> = ic.iter().map(|(c, x)| (c.tag.clone(), weight(*x))).collect();
    top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    top.truncate(8);
    let summary = Hoi4Summary {
        version: world.version.clone(),
        date: world.date.clone(),
        player: world.player.clone(),
        world_factories: total as u32,
        countries_with_industry: ic.len(),
        top_powers: top,
        factions: shares.iter().map(|(n, _, s)| (n.clone(), *s)).collect(),
        countries_at_war: at_war.iter().map(|s| s.to_string()).collect(),
        missing: crate::state::INDICATORS.iter().filter(|k| !v.contains_key(**k)).map(|k| k.to_string()).collect(),
    };
    Hoi4Civilization { state, summary }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../../../tests/fixtures/hoi4/1.19.3/mini_save.hoi4.txt");

    #[test]
    fn reads_structure_verified_on_1_19_3() {
        let w = read_hoi4_bytes(FIXTURE).unwrap();
        assert_eq!(w.date.as_deref(), Some("1940.1.3.14"));
        assert_eq!(w.player.as_deref(), Some("GER"));
        assert_eq!(w.version.as_deref(), Some("Operation Postern v1.19.3.0.c01a (5632)"));
        let tags: Vec<&str> = w.countries.iter().map(|c| c.tag.as_str()).collect();
        assert_eq!(tags, ["GER", "ENG", "SOV", "CHI", "POL"], "XXX has no government and is skipped");
        let ger = &w.countries[0];
        assert_eq!(ger.ruling_party.as_deref(), Some("fascism"));
        assert_eq!(ger.popularity["fascism"], 56.0);
        assert!(ger.ideas.contains(&"war_economy".to_string()));
        assert_eq!(ger.technologies.len(), 3);
        assert_eq!(w.countries[1].nukes, 2.0);
        assert_eq!(w.factories["GER"], Factories { civilian: 6, military: 4, dockyards: 0, states: 1 });
        assert_eq!(w.factions.len(), 2);
        assert!(w.wars.contains(&("SOV".to_string(), "CHI".to_string())));
    }

    /// Same numbers as the research prototype (Python) on the same fixture.
    #[test]
    fn indicators_match_prototype() {
        let w = read_hoi4_bytes(FIXTURE).unwrap();
        let c = to_civilization(&w, Hoi4Mapping::builtin());
        let expect = [
            ("authoritarianism", 0.6384833333333334),
            ("economic_planning", 0.43333333333333335),
            ("global_wars", 0.3333333333333333),
            ("industrialization", 1.0),
            ("international_cooperation", 0.4916666666666667),
            ("militarism", 0.44833333333333336),
            ("nuclear_weapons", 0.18126924692201818),
            ("planetary_unification", 0.4166666666666667),
            ("social_equality", 0.48831666666666673),
            ("space_program", 0.034722222222222224),
            ("technology", 0.004761904761904762),
            ("xenophobia", 0.5338416666666668),
        ];
        for (k, e) in expect {
            assert!((c.state.get(k) - e).abs() < 1e-9, "{k}: {} vs {e}", c.state.get(k));
        }
        assert_eq!(c.state.blocs, 2);
        assert_eq!(c.state.ideology, Ideology::Fascism);
        assert_eq!(c.state.coverage(), (12, 14));
        assert_eq!(c.summary.missing, ["religious_influence", "environmental_policy"]);
        assert!(crate::run(&c.state, 1).unwrap().problems.is_empty());
    }

    #[test]
    fn binary_and_foreign_files_are_refused() {
        assert!(matches!(read_hoi4_bytes(b"HOI4bin\x01\x00\x0f\x00"), Err(Hoi4Error::Binary)));
        assert!(matches!(read_hoi4_bytes(b"version=\"Circinus v3.14.15\""), Err(Hoi4Error::NotHoi4(_))));
    }
}
