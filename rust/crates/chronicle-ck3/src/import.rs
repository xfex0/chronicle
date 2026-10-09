//! Import a CK3 world into a Chronicle campaign.
//!
//! - backup (auto) → copy of the save into `original/` (read-only) → save file + snapshot rows;
//! - cultures, faiths, houses, rulers, realms (countries) and counties (territories), each with
//!   a stable Chronicle id mapped to its CK3 id (re-imports reuse ids);
//! - territory ownership and ruler/dynasty periods (forward-only: older saves add warnings);
//! - semantic values with provenance + confidence (centralization, regional autonomy,
//!   average development), gated by the verified CK3 versions;
//! - journal events from kingdom/empire title history: before the bookmark = scripted history
//!   (origin `game`), after = this campaign (origin `save`). Re-imports never duplicate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chronicle_core::{ChronicleId, ConfidenceComponents, EntityKind, EventOrigin, PartialDate};
use chronicle_db::{Aspect, BackupKind, CampaignDb, DbError, NamedTable, NewEvent, PeriodError, SemanticInput};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::read::{Ck3World, HistoryKind};

const GAME: &str = "ck3";
pub const ADAPTER_VERSION: &str = concat!("chronicle-ck3 ", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("the save has no date")]
    NoDate,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Weighted {
    pub formula: String,
    #[serde(default)]
    pub authority_law: f64,
    #[serde(default)]
    pub county_control: f64,
    pub mapping_reliability: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ck3Mapping {
    pub version: u32,
    pub verified_versions: Vec<String>,
    pub authority_law_level: BTreeMap<String, f64>,
    pub centralization: Weighted,
    pub regional_autonomy: Weighted,
    pub average_development: Weighted,
    pub history_min_tier: i8,
}

pub const BUILTIN_MAPPING: &str = include_str!("../../../../games/ck3/semantic_mapping.yaml");

impl Ck3Mapping {
    pub fn builtin() -> &'static Ck3Mapping {
        static M: OnceLock<Ck3Mapping> = OnceLock::new();
        M.get_or_init(|| serde_yaml::from_str(BUILTIN_MAPPING).expect("games/ck3/semantic_mapping.yaml is valid"))
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportReport {
    pub game_version: Option<String>,
    pub version_verified: bool,
    pub date: Option<PartialDate>,
    pub realms: usize,
    pub counties: usize,
    pub rulers: usize,
    pub dynasties: usize,
    pub cultures: usize,
    pub faiths: usize,
    pub new_entities: usize,
    pub ownership_changes: usize,
    pub events_added: usize,
    pub semantic_values: usize,
    pub player_realm: Option<String>,
    pub stored_copy: String,
    pub snapshot_id: i64,
    pub warnings: Vec<String>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Copy the user's save into `original/` (content-addressed, read-only). Never writes to the source.
fn store_original(db: &CampaignDb, save: &Path) -> Result<(PathBuf, String), ImportError> {
    let bytes = std::fs::read(save)?;
    let sha = sha256_hex(&bytes);
    let name = save.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "save.ck3".into());
    let dir = db.root().join("original");
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{}_{name}", &sha[..12]));
    if !dest.exists() {
        std::fs::write(&dest, &bytes)?;
        let mut perm = std::fs::metadata(&dest)?.permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&dest, perm)?;
    }
    Ok((dest, sha))
}

fn period_warning(r: Result<(), DbError>, what: &str, warnings: &mut Vec<String>) -> Result<bool, ImportError> {
    match r {
        Ok(()) => Ok(true),
        Err(DbError::Period(PeriodError::Backwards { .. })) => {
            warnings.push(format!("{what}: this save is older than the campaign's history; kept the newer data"));
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn import_world(db: &mut CampaignDb, world: &Ck3World, save: &Path) -> Result<ImportReport, ImportError> {
    let map = Ck3Mapping::builtin();
    let date = world.date.ok_or(ImportError::NoDate)?;
    let version_verified = world.version.as_ref().is_some_and(|v| map.verified_versions.contains(v));
    let mut rep = ImportReport {
        game_version: world.version.clone(),
        version_verified,
        date: Some(date),
        ..Default::default()
    };
    if !version_verified {
        rep.warnings.push(format!(
            "CK3 {} is not a verified save layout yet; every derived value goes to manual review",
            world.version.clone().unwrap_or_else(|| "?".into())
        ));
    }

    db.backup(BackupKind::Auto, "before ck3 import")?;
    let (stored, sha) = store_original(db, save)?;
    rep.stored_copy = stored.display().to_string();

    // Everything below is ONE transaction: a single disk flush instead of thousands, and an
    // all-or-nothing import (a failure leaves the campaign exactly as it was).
    db.begin_bulk()?;
    match import_body(db, world, save, &sha, date, version_verified, &mut rep) {
        Ok(()) => db.commit_bulk()?,
        Err(e) => {
            db.rollback_bulk();
            return Err(e);
        }
    }
    db.integrity_check()?;
    rep.warnings.sort();
    rep.warnings.dedup();
    Ok(rep)
}

#[allow(clippy::too_many_arguments)]
fn import_body(
    db: &mut CampaignDb,
    world: &Ck3World,
    save: &Path,
    sha: &str,
    date: PartialDate,
    version_verified: bool,
    rep: &mut ImportReport,
) -> Result<(), ImportError> {
    let map = Ck3Mapping::builtin();
    let bookmark = world.bookmark.unwrap_or(PartialDate::ymd(867, 1, 1));
    let policy = db.summary()?.settings.confidence;

    let save_id = db.record_save_file(GAME, &save.display().to_string(), &rep.stored_copy, sha, Some(date), world.version.as_deref())?;
    let kind = if db.snapshot_count(GAME)? == 0 { "initial" } else { "periodic" };
    rep.snapshot_id = db.record_snapshot(GAME, date, kind, &rep.stored_copy, sha, Some(save_id))?;

    // lookups built once (the save has ~12 000 titles)
    let title_by_key: BTreeMap<&str, &crate::read::Ck3Title> = world.titles.values().map(|t| (t.key.as_str(), t)).collect();

    let mut new_entities = 0usize;
    let mut id_for = |db: &mut CampaignDb, gid: String, kind: EntityKind| -> Result<ChronicleId, ImportError> {
        let (id, created) = db.entity_for(GAME, &gid, kind)?;
        new_entities += usize::from(created);
        Ok(id)
    };

    let mut culture_ids = BTreeMap::new();
    let used_cultures: BTreeSet<&String> = world
        .counties
        .values()
        .filter_map(|c| c.culture.as_ref())
        .chain(world.characters.values().filter_map(|c| c.culture.as_ref()))
        .collect();
    for c in used_cultures {
        let id = id_for(&mut *db, format!("culture:{c}"), EntityKind::Culture)?;
        db.upsert_named(NamedTable::Culture, &id, world.cultures.get(c).map(String::as_str).unwrap_or(c))?;
        culture_ids.insert(c.clone(), id);
    }
    rep.cultures = culture_ids.len();
    let mut faith_ids = BTreeMap::new();
    let used_faiths: BTreeSet<&String> = world
        .counties
        .values()
        .filter_map(|c| c.faith.as_ref())
        .chain(world.characters.values().filter_map(|c| c.faith.as_ref()))
        .collect();
    for f in used_faiths {
        let id = id_for(&mut *db, format!("faith:{f}"), EntityKind::Religion)?;
        db.upsert_named(NamedTable::Religion, &id, world.faiths.get(f).map(String::as_str).unwrap_or(f))?;
        faith_ids.insert(f.clone(), id);
    }
    rep.faiths = faith_ids.len();
    let mut house_ids = BTreeMap::new();
    for (hid, h) in &world.houses {
        let id = id_for(&mut *db, format!("house:{hid}"), EntityKind::Dynasty)?;
        db.upsert_dynasty(&id, &h.name, h.founded)?;
        house_ids.insert(hid.clone(), id);
    }
    rep.dynasties = house_ids.len();

    // realms, rulers, counties
    let mut country_ids: BTreeMap<String, ChronicleId> = BTreeMap::new();
    for realm in &world.realms {
        let Some(title) = world.titles.get(&realm.primary_title) else {
            rep.warnings.push(format!("realm of character {} skipped: primary title not found", realm.ruler));
            continue;
        };
        let ruler = world.characters.get(&realm.ruler);
        let country = id_for(&mut *db, format!("title:{}", title.key), EntityKind::Country)?;
        let culture = ruler.and_then(|r| r.culture.as_ref()).and_then(|c| culture_ids.get(c));
        let faith = ruler.and_then(|r| r.faith.as_ref()).and_then(|f| faith_ids.get(f));
        let created = title.history.iter().rev().find(|h| h.kind == HistoryKind::Created).map(|h| h.date);
        let name = if title.name.is_empty() { title.key.clone() } else { title.name.clone() };
        db.upsert_country(&country, &name, culture, faith, created)?;
        if world.player.as_deref() == Some(realm.ruler.as_str()) {
            rep.player_realm = Some(name.clone());
        }
        if let Some(r) = ruler {
            let cid = id_for(&mut *db, format!("char:{}", r.id), EntityKind::Character)?;
            let house = r.house.as_ref().and_then(|h| house_ids.get(h)).cloned();
            db.upsert_character(&cid, &r.name, r.birth, house.as_ref(), culture, faith)?;
            let since = r.became_ruler.unwrap_or(date);
            period_warning(db.set_period(&country, Aspect::Ruler, None, Some(&cid), since, GAME, "save"), "ruler", &mut rep.warnings)?;
            if let Some(h) = &house {
                period_warning(db.set_period(&country, Aspect::Dynasty, None, Some(h), since, GAME, "save"), "dynasty", &mut rep.warnings)?;
            }
            rep.rulers += 1;
        }
        for ck in &realm.counties {
            let tid = id_for(&mut *db, format!("county:{ck}"), EntityKind::Territory)?;
            let tname = title_by_key.get(ck.as_str()).map(|t| t.name.as_str()).unwrap_or("");
            db.upsert_territory(&tid, if tname.is_empty() { ck.as_str() } else { tname })?;
            if period_warning(db.set_owner(&tid, Some(&country), date, GAME, "save"), "ownership", &mut rep.warnings)? {
                rep.ownership_changes += 1;
            }
            rep.counties += 1;
        }
        country_ids.insert(realm.primary_title.clone(), country);
    }
    rep.realms = country_ids.len();
    rep.new_entities = new_entities;

    // semantic values (once per save date)
    for realm in &world.realms {
        let Some(country) = country_ids.get(&realm.primary_title) else { continue };
        if db.semantic_exists(country, "centralization", date, GAME)? {
            continue; // this save was already imported
        }
        let ruler = world.characters.get(&realm.ruler);
        let counties: Vec<_> = realm.counties.iter().filter_map(|k| world.counties.get(k)).collect();
        if counties.is_empty() {
            continue;
        }
        let n = counties.len() as f64;
        let avg_control = counties.iter().map(|c| c.control).sum::<f64>() / n;
        let avg_dev = counties.iter().map(|c| c.development).sum::<f64>() / n;
        let law = ruler.and_then(|r| r.laws.iter().find_map(|l| map.authority_law_level.get(l).copied()));

        let c = &map.centralization;
        let centralization = c.authority_law * law.unwrap_or(0.0) + c.county_control * avg_control / 100.0;
        let mut contrib = BTreeMap::new();
        contrib.insert("authority_law".to_string(), c.authority_law * law.unwrap_or(0.0));
        contrib.insert("county_control".to_string(), c.county_control * avg_control / 100.0);
        db.add_semantic(
            &SemanticInput {
                entity: country,
                date,
                key: "centralization",
                value: centralization,
                source_game: GAME,
                source_fields: &["landed_data.laws (authority)", "county_manager.county_control"],
                contributions: contrib,
                formula: &c.formula,
                components: ConfidenceComponents::new(
                    1 + usize::from(law.is_some()),
                    2,
                    c.mapping_reliability,
                    version_verified,
                    (0.0..=1.0).contains(&centralization),
                ),
                adapter_version: ADAPTER_VERSION,
            },
            &policy,
        )?;

        let domain: BTreeSet<&String> = ruler.map(|r| r.domain.iter().collect()).unwrap_or_default();
        let direct = realm
            .counties
            .iter()
            .filter(|k| title_by_key.get(k.as_str()).is_some_and(|t| domain.contains(&t.id)))
            .count() as f64;
        let autonomy = 1.0 - direct / n;
        let ra = &map.regional_autonomy;
        let mut contrib = BTreeMap::new();
        contrib.insert("vassal_held_share".to_string(), autonomy);
        db.add_semantic(
            &SemanticInput {
                entity: country,
                date,
                key: "regional_autonomy",
                value: autonomy,
                source_game: GAME,
                source_fields: &["landed_data.domain", "landed_titles.de_facto_liege"],
                contributions: contrib,
                formula: &ra.formula,
                components: ConfidenceComponents::new(usize::from(ruler.is_some()), 1, ra.mapping_reliability, version_verified, true),
                adapter_version: ADAPTER_VERSION,
            },
            &policy,
        )?;

        let ad = &map.average_development;
        let mut contrib = BTreeMap::new();
        contrib.insert("county_development_mean".to_string(), avg_dev);
        db.add_semantic(
            &SemanticInput {
                entity: country,
                date,
                key: "average_development",
                value: avg_dev,
                source_game: GAME,
                source_fields: &["county_manager.development"],
                contributions: contrib,
                formula: &ad.formula,
                components: ConfidenceComponents::new(1, 1, ad.mapping_reliability, version_verified, avg_dev >= 0.0),
                adapter_version: ADAPTER_VERSION,
            },
            &policy,
        )?;
        rep.semantic_values += 3;
    }

    // journal from kingdom/empire history
    for title in world.titles.values().filter(|t| t.tier() >= map.history_min_tier) {
        let actor = country_ids.get(&title.id);
        for h in &title.history {
            let (event_type, importance) = match h.kind {
                HistoryKind::Holder => ("ruler_changed", if title.tier() >= 4 { 4 } else { 3 }),
                HistoryKind::Created => ("title_created", 4),
                HistoryKind::Destroyed => ("title_destroyed", 4),
            };
            let key = format!("{}:{}:{}", title.key, h.date, event_type);
            if db.event_exists(event_type, h.date, &key)? {
                continue;
            }
            let origin = if h.date < bookmark { EventOrigin::Game } else { EventOrigin::Save };
            db.add_event(&NewEvent {
                game_version: world.version.as_deref(),
                actor,
                payload: serde_json::json!({
                    "key": key,
                    "title": title.key,
                    "title_name": title.name,
                    "ck3_holder": h.holder,
                }),
                importance,
                ..NewEvent::simple(h.date, GAME, event_type, origin)
            })?;
            rep.events_added += 1;
        }
    }
    db.set_current(GAME, date)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CampaignSettings;
    use chronicle_core::registry::GameRegistry;

    fn setup() -> (tempfile::TempDir, CampaignDb, PathBuf, Ck3World) {
        let tmp = tempfile::tempdir().unwrap();
        let db = CampaignDb::create(&tmp.path().join("camp"), "Test", &CampaignSettings::new("ck3", 1),
                                    &GameRegistry::builtin().unwrap()).unwrap();
        let save = tmp.path().join("fixture.ck3");
        std::fs::write(&save, crate::read::tests::FIXTURE).unwrap();
        let world = crate::read::read_gamestate(crate::read::tests::FIXTURE).unwrap();
        (tmp, db, save, world)
    }

    #[test]
    fn imports_world_into_campaign() {
        let (_tmp, mut db, save, world) = setup();
        let r = import_world(&mut db, &world, &save).unwrap();
        assert!(r.version_verified);
        assert_eq!((r.realms, r.counties, r.rulers), (2, 3, 2));
        assert_eq!(r.player_realm.as_deref(), Some("Гуджарат"));
        assert_eq!(r.ownership_changes, 3);
        assert_eq!(r.events_added, 4, "k_gujarat history: 2 scripted + 2 campaign events");
        assert_eq!(r.semantic_values, 6);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);

        let s = db.summary().unwrap();
        assert_eq!(s.current_date, Some(PartialDate::ymd(923, 4, 28)));
        let ev = db.events(0, 100).unwrap();
        let origins: Vec<(&str, &str)> = ev.iter().map(|e| (e.event_type.as_str(), e.origin.as_str())).collect();
        assert!(origins.contains(&("ruler_changed", "game")), "502.1.1 is before the 867 bookmark");
        assert!(origins.contains(&("title_created", "save")));

        let gujarat = db.entity_for(GAME, "title:k_gujarat", EntityKind::Country).unwrap().0;
        let kheta = db.entity_for(GAME, "county:c_b", EntityKind::Territory).unwrap().0;
        let owner = db.owner_at(kheta.as_str(), PartialDate::ymd(923, 4, 28)).unwrap().unwrap();
        assert_eq!(owner.owner_id.as_deref(), Some(gujarat.as_str()), "vassal county belongs to the realm");
        let years = db.years_in_current_period(gujarat.as_str(), Aspect::Ruler, PartialDate::ymd(923, 4, 28)).unwrap();
        assert_eq!(years, Some(0), "ruler since 923.4.21");
    }

    #[test]
    fn centralization_and_autonomy_values() {
        let (_tmp, mut db, save, world) = setup();
        import_world(&mut db, &world, &save).unwrap();
        let q = chronicle_db::dev::readonly_query(
            &db,
            "SELECT s.semantic_key, s.value, s.review_state FROM semantic_values s
             JOIN game_entity_mappings m ON m.entity_id = s.entity_id
             WHERE m.game_entity_id = 'title:k_gujarat' ORDER BY s.semantic_key",
            10,
        )
        .unwrap();
        let get = |k: &str| q.rows.iter().find(|r| r[0] == k).map(|r| (r[1].as_f64().unwrap(), r[2].as_str().unwrap().to_string())).unwrap();
        let (c, _) = get("centralization");
        assert!((c - 0.51).abs() < 1e-9, "0.6 × 0.25 + 0.4 × 0.90 = {c}");
        assert_eq!(get("regional_autonomy").0, 0.5, "1 of 2 counties held directly");
        assert_eq!(get("average_development").0, 12.0);
    }

    #[test]
    fn reimport_is_idempotent() {
        let (_tmp, mut db, save, world) = setup();
        let first = import_world(&mut db, &world, &save).unwrap();
        let second = import_world(&mut db, &world, &save).unwrap();
        assert!(first.new_entities > 0);
        assert_eq!(second.new_entities, 0, "same CK3 ids map to the same Chronicle ids");
        assert_eq!(second.events_added, 0, "history is not duplicated");
        assert_eq!(second.ownership_changes, 3, "same owners: no-op periods");
        let failed: Vec<_> = chronicle_db::dev::self_test(&db).unwrap().into_iter().filter(|c| !c.ok).collect();
        assert!(failed.is_empty(), "{failed:?}");
    }

    #[test]
    fn unverified_version_goes_to_review() {
        let (_tmp, mut db, save, mut world) = setup();
        world.version = Some("1.16.0".into());
        let r = import_world(&mut db, &world, &save).unwrap();
        assert!(!r.version_verified && !r.warnings.is_empty());
        let q = chronicle_db::dev::readonly_query(&db, "SELECT DISTINCT review_state FROM semantic_values", 10).unwrap();
        assert_eq!(q.rows, vec![vec![serde_json::Value::String("pending_review".into())]]);
    }
}
