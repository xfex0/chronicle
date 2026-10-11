//! Import a HoI4 text save into a Chronicle campaign (same rules as the CK3 importer):
//! backup → read-only copy of the save → one transaction → integrity check.
//!
//! - countries that own states → country entities (keyed by tag, e.g. "tag:GER");
//! - states → territory entities ("state:64") with ownership periods;
//! - ruling party → government period ("fascism", "democratic", ...);
//! - semantic values per country: industrial capacity, stability, war support;
//! - journal: every war with its start date (observed in the save) and one world snapshot
//!   event (largest powers, factions, wars) at the save date;
//! - the campaign's current game becomes HoI4 at the save date.
//!
//! HoI4 saves carry no long history (unlike CK3 title history), so the journal grows with
//! every imported save; snapshot diffs between saves come next.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chronicle_core::{ChronicleId, ConfidenceComponents, EntityKind, EventOrigin, PartialDate};
use chronicle_db::{Aspect, BackupKind, CampaignDb, DbError, NewEvent, OwnerChange, PeriodError, SemanticInput};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::hoi4_save::{Hoi4Mapping, Hoi4World, to_civilization};

const GAME: &str = "hoi4";
const ADAPTER: &str = concat!("chronicle-bridge/hoi4 ", env!("CARGO_PKG_VERSION"));
/// Save layouts checked against real saves.
const VERIFIED: [&str; 1] = ["1.19.3"];

#[derive(Debug, thiserror::Error)]
pub enum Hoi4ImportError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("the save has no readable date")]
    NoDate,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Hoi4ImportReport {
    pub game_version: Option<String>,
    pub version_verified: bool,
    pub date: Option<PartialDate>,
    pub player: Option<String>,
    pub countries: usize,
    pub states: usize,
    pub wars: usize,
    pub events_added: usize,
    pub ownership_changes: usize,
    /// States whose owner differs from the previous imported save (journal: inferred events).
    pub territories_changed: usize,
    pub semantic_values: usize,
    pub new_entities: usize,
    pub stored_copy: String,
    pub warnings: Vec<String>,
}

/// "1940.1.3.14" (date + hour) → 1940.1.3
pub fn hoi4_date(s: &str) -> Option<PartialDate> {
    let parts: Vec<&str> = s.trim_matches('"').split('.').take(3).collect();
    parts.join(".").parse().ok()
}

/// "Operation Postern v1.19.3.0.c01a (5632)" → "1.19.3"
pub fn hoi4_version(s: &str) -> Option<String> {
    paradox_parser::sniff_version(s.as_bytes())
}

fn store_original(db: &CampaignDb, save: &Path) -> Result<(PathBuf, String), Hoi4ImportError> {
    let bytes = std::fs::read(save)?;
    let sha: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let name = save.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "save.hoi4".into());
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

fn period_ok(r: Result<(), DbError>, what: &str, warnings: &mut Vec<String>) -> Result<bool, Hoi4ImportError> {
    match r {
        Ok(()) => Ok(true),
        Err(DbError::Period(PeriodError::Backwards { .. })) => {
            warnings.push(format!("{what}: this save is older than the campaign's history; kept the newer data"));
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn import_hoi4(db: &mut CampaignDb, world: &Hoi4World, save: &Path) -> Result<Hoi4ImportReport, Hoi4ImportError> {
    let date = world.date.as_deref().and_then(hoi4_date).ok_or(Hoi4ImportError::NoDate)?;
    let version = world.version.as_deref().and_then(hoi4_version);
    let version_verified = version.as_deref().is_some_and(|v| VERIFIED.contains(&v));
    let mut rep = Hoi4ImportReport {
        game_version: version.clone(),
        version_verified,
        date: Some(date),
        player: world.player.clone(),
        ..Default::default()
    };
    if !version_verified {
        rep.warnings.push(format!(
            "HoI4 {} is not a verified save layout yet; derived values go to manual review",
            version.unwrap_or_else(|| "?".into())
        ));
    }
    db.backup(BackupKind::Auto, "before hoi4 import")?;
    let (stored, sha) = store_original(db, save)?;
    rep.stored_copy = stored.display().to_string();

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
    world: &Hoi4World,
    save: &Path,
    sha: &str,
    date: PartialDate,
    version_verified: bool,
    rep: &mut Hoi4ImportReport,
) -> Result<(), Hoi4ImportError> {
    let policy = db.summary()?.settings.confidence;
    let previous_snapshot = db.previous_snapshot_date(GAME, date)?;
    let save_id = db.record_save_file(GAME, &save.display().to_string(), &rep.stored_copy, sha, Some(date), rep.game_version.as_deref())?;
    let kind = if db.snapshot_count(GAME)? == 0 { "initial" } else { "periodic" };
    db.record_snapshot(GAME, date, kind, &rep.stored_copy, sha, Some(save_id))?;

    let mut new_entities = 0usize;
    let mut id_for = |db: &mut CampaignDb, gid: String, kind: EntityKind| -> Result<ChronicleId, Hoi4ImportError> {
        let (id, created) = db.entity_for(GAME, &gid, kind)?;
        new_entities += usize::from(created);
        Ok(id)
    };

    // countries that own at least one state
    let mut country_ids: BTreeMap<String, ChronicleId> = BTreeMap::new();
    for c in &world.countries {
        let Some(f) = world.factories.get(&c.tag) else { continue };
        let id = id_for(&mut *db, format!("tag:{}", c.tag), EntityKind::Country)?;
        db.upsert_country(&id, &c.tag, None, None, None)?;
        if let Some(party) = &c.ruling_party {
            period_ok(db.set_period(&id, Aspect::Government, Some(party.as_str()), None, date, GAME, "save"), "government", &mut rep.warnings)?;
        }
        if !db.semantic_exists(&id, "industrial_capacity", date, GAME)? {
            let comps = ConfidenceComponents::new(1, 1, 0.9, version_verified, true);
            for (key, value, field) in [
                ("industrial_capacity", f64::from(f.civilian + f.military), "states.buildings (industrial_complex + arms_factory)"),
                ("stability", c.stability, "countries.stability"),
                ("war_support", c.war_support, "countries.war_support"),
            ] {
                let mut contrib = BTreeMap::new();
                contrib.insert(key.to_string(), value);
                db.add_semantic(
                    &SemanticInput {
                        entity: &id,
                        date,
                        key,
                        value,
                        source_game: GAME,
                        source_fields: &[field],
                        contributions: contrib,
                        formula: "hoi4_direct_v1",
                        components: comps,
                        adapter_version: ADAPTER,
                    },
                    &policy,
                )?;
                rep.semantic_values += 1;
            }
        }
        country_ids.insert(c.tag.clone(), id);
    }
    rep.countries = country_ids.len();

    // states and owners
    for (sid, owner) in &world.states {
        let Some(owner_id) = country_ids.get(owner) else { continue };
        let tid = id_for(&mut *db, format!("state:{sid}"), EntityKind::Territory)?;
        db.upsert_territory(&tid, &format!("State {sid}"))?;
        match db.set_owner_tracked(&tid, Some(owner_id), date, GAME, "save") {
            Ok(change) => {
                rep.ownership_changes += 1;
                if let OwnerChange::Changed { previous } = change {
                    rep.territories_changed += 1;
                    let prev_id = previous.and_then(|p| p.parse::<ChronicleId>().ok());
                    let from = previous_snapshot.unwrap_or(date);
                    let key = format!("hoi4:owner:{sid}:{date}");
                    if !db.event_exists("territory_changed_owner", from, &key)? {
                        db.add_event(&NewEvent {
                            date_to: (from < date).then_some(date),
                            game_version: rep.game_version.as_deref(),
                            actor: Some(owner_id),
                            target: prev_id.as_ref(),
                            payload: serde_json::json!({ "key": key, "territory": format!("State {sid}"), "new_owner": owner }),
                            importance: 2,
                            ..NewEvent::simple(from, GAME, "territory_changed_owner", EventOrigin::SnapshotDiff)
                        })?;
                        rep.events_added += 1;
                    }
                }
            }
            Err(e) => {
                period_ok(Err(e), "ownership", &mut rep.warnings)?;
            }
        }
        rep.states += 1;
    }
    rep.new_entities = new_entities;

    // journal: wars (observed start dates) + one world snapshot
    for w in &world.war_details {
        let start = w.start.as_deref().and_then(hoi4_date).unwrap_or(date);
        let key = format!("hoi4:war:{}:{}:{}", w.first, w.second, start);
        rep.wars += 1;
        if db.event_exists("war_started", start, &key)? {
            continue;
        }
        db.add_event(&NewEvent {
            game_version: rep.game_version.as_deref(),
            actor: country_ids.get(&w.first),
            target: country_ids.get(&w.second),
            payload: serde_json::json!({ "key": key, "attacker": w.first, "defender": w.second }),
            importance: 4,
            ..NewEvent::simple(start, GAME, "war_started", EventOrigin::Save)
        })?;
        rep.events_added += 1;
    }
    let key = format!("hoi4:snapshot:{date}");
    if !db.event_exists("world_snapshot", date, &key)? {
        let civ = to_civilization(world, Hoi4Mapping::builtin());
        db.add_event(&NewEvent {
            game_version: rep.game_version.as_deref(),
            actor: world.player.as_ref().and_then(|p| country_ids.get(p)),
            payload: serde_json::json!({
                "key": key,
                "player": world.player,
                "top_powers": civ.summary.top_powers,
                "factions": civ.summary.factions,
                "countries_at_war": civ.summary.countries_at_war,
                "world_factories": civ.summary.world_factories,
            }),
            importance: 3,
            ..NewEvent::simple(date, GAME, "world_snapshot", EventOrigin::Save)
        })?;
        rep.events_added += 1;
    }
    db.set_current(GAME, date)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hoi4_save::read_hoi4_bytes;
    use chronicle_core::CampaignSettings;
    use chronicle_core::registry::GameRegistry;

    const FIXTURE: &[u8] = include_bytes!("../../../../tests/fixtures/hoi4/1.19.3/mini_save.hoi4.txt");

    #[test]
    fn dates_and_versions() {
        assert_eq!(hoi4_date("1940.1.3.14"), Some(PartialDate::ymd(1940, 1, 3)));
        assert_eq!(hoi4_date("\"1939.8.14.12\""), Some(PartialDate::ymd(1939, 8, 14)));
        assert_eq!(hoi4_version("Operation Postern v1.19.3.0.c01a (5632)").as_deref(), Some("1.19.3"));
    }

    #[test]
    fn imports_into_campaign_and_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = CampaignDb::create(&tmp.path().join("c"), "x", &CampaignSettings::new("hoi4", 1),
                                        &GameRegistry::builtin().unwrap()).unwrap();
        let save = tmp.path().join("GER_1940.hoi4");
        std::fs::write(&save, FIXTURE).unwrap();
        let world = read_hoi4_bytes(FIXTURE).unwrap();
        let r = import_hoi4(&mut db, &world, &save).unwrap();
        assert!(r.version_verified);
        assert_eq!((r.countries, r.states, r.wars), (5, 5, 1));
        assert_eq!(r.events_added, 2, "one war + one world snapshot");
        assert_eq!(r.semantic_values, 15);
        let s = db.summary().unwrap();
        assert_eq!((s.current_game.as_str(), s.current_date), ("hoi4", Some(PartialDate::ymd(1940, 1, 3))));
        let ev = db.events(0, 100).unwrap();
        let war = ev.iter().find(|e| e.event_type == "war_started").unwrap();
        assert_eq!(war.date, PartialDate::ymd(1939, 8, 14));

        let again = import_hoi4(&mut db, &world, &save).unwrap();
        assert_eq!((again.events_added, again.new_entities, again.semantic_values), (0, 0, 0));
        let failed: Vec<_> = chronicle_db::dev::self_test(&db).unwrap().into_iter().filter(|c| !c.ok).collect();
        assert!(failed.is_empty(), "{failed:?}");
    }
}
