//! SQLite storage.
//!
//! - [`AppDb`]: one per user — settings (language…), detected game installations, the list
//!   of campaigns. Lives in the OS app-data folder.
//! - [`CampaignDb`]: one per campaign, inside the campaign folder — the historical truth.
//!   The campaign folder is portable: copy it to another PC and open it there.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chronicle_core::registry::GameRegistry;
use chronicle_core::{
    CHRONICLE_VERSION, CampaignSettings, ChronicleId, DatePrecision, EntityKind, EventEvidence, EventOrigin, PartialDate,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod dev;
pub mod migrate;
pub mod periods;
pub mod world;

pub use periods::{Aspect, OwnershipPeriod, Period, PeriodError};
pub use world::{NamedTable, SemanticInput};

/// Campaign folder layout (originals are read-only copies; nothing is ever written to user saves).
pub const CAMPAIGN_DIRS: [&str; 8] =
    ["original", "backups", "snapshots", "working", "generated", "reports", "database", "logs"];

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("data error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("folder {0} is not empty; choose an empty or new folder for the campaign")]
    NotEmpty(PathBuf),
    #[error("no campaign database in {0}")]
    NotACampaign(PathBuf),
    #[error("invalid campaign settings: {0}")]
    Settings(String),
    #[error(transparent)]
    Period(#[from] PeriodError),
    #[error("only read-only SELECT queries are allowed in the developer console")]
    NotReadOnly,
    #[error("this action is only allowed in an empty or demo campaign")]
    NotDemoSafe,
    #[error("database integrity check failed: {0}")]
    Integrity(String),
}

pub type Result<T> = std::result::Result<T, DbError>;

fn open_conn(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    // WAL + synchronous=NORMAL: durable at checkpoints, far fewer disk flushes on Windows.
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
    Ok(conn)
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// `2026-10-09_215501` (UTC) for backup file names, without a date-time dependency.
pub fn utc_stamp(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Howard Hinnant's civil-from-days algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}_{:02}{:02}{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    /// Before imports and other routine operations. Only the newest 10 are kept.
    Auto,
    /// Before an era transition. Never deleted automatically.
    Transition,
    /// Before a database upgrade. Never deleted automatically.
    Migration,
    /// Requested by the user. Never deleted automatically.
    Manual,
}

impl BackupKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Transition => "transition",
            Self::Migration => "migration",
            Self::Manual => "manual",
        }
    }
}

pub const AUTO_BACKUPS_KEPT: usize = 10;

fn integrity(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?)
}

// ======================================================================================
// App database
// ======================================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstallationRow {
    pub game_key: String,
    /// steam | manual | other | missing
    pub source: String,
    pub install_path: Option<String>,
    pub save_path: Option<String>,
    pub steam_app_id: Option<u32>,
    pub detected_version: Option<String>,
    /// verified | unverified | mismatch | unknown
    pub fingerprint: String,
    pub install_manual: bool,
    pub save_manual: bool,
    pub last_scan: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignIndexRow {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub last_opened: Option<String>,
}

pub struct AppDb {
    conn: Connection,
}

impl AppDb {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut conn = open_conn(path)?;
        migrate::migrate(&mut conn, migrate::APP_MIGRATIONS)?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        migrate::migrate(&mut conn, migrate::APP_MIGRATIONS)?;
        Ok(Self { conn })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Store a scan result. Paths the user set manually survive rescans.
    pub fn upsert_detected(&self, row: &InstallationRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO game_installations
                 (game_key, source, install_path, save_path, steam_app_id, detected_version, fingerprint, last_scan)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
             ON CONFLICT(game_key) DO UPDATE SET
                 source       = CASE WHEN install_manual THEN source       ELSE excluded.source END,
                 install_path = CASE WHEN install_manual THEN install_path ELSE excluded.install_path END,
                 fingerprint  = CASE WHEN install_manual THEN fingerprint  ELSE excluded.fingerprint END,
                 save_path    = CASE WHEN save_manual    THEN save_path    ELSE excluded.save_path END,
                 steam_app_id = excluded.steam_app_id,
                 detected_version = excluded.detected_version,
                 last_scan    = datetime('now')",
            params![
                row.game_key,
                row.source,
                row.install_path,
                row.save_path,
                row.steam_app_id,
                row.detected_version,
                row.fingerprint
            ],
        )?;
        Ok(())
    }

    pub fn set_manual_install(&self, game_key: &str, path: &str, fingerprint: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO game_installations(game_key, source, install_path, fingerprint, install_manual)
             VALUES (?1, 'manual', ?2, ?3, 1)
             ON CONFLICT(game_key) DO UPDATE SET source = 'manual', install_path = excluded.install_path,
                 fingerprint = excluded.fingerprint, install_manual = 1",
            params![game_key, path, fingerprint],
        )?;
        Ok(())
    }

    pub fn set_manual_save(&self, game_key: &str, path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO game_installations(game_key, source, save_path, save_manual)
             VALUES (?1, 'missing', ?2, 1)
             ON CONFLICT(game_key) DO UPDATE SET save_path = excluded.save_path, save_manual = 1",
            params![game_key, path],
        )?;
        Ok(())
    }

    /// "Reset auto detection": the next scan may overwrite both paths again.
    pub fn reset_manual(&self, game_key: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE game_installations SET install_manual = 0, save_manual = 0 WHERE game_key = ?1",
            [game_key],
        )?;
        Ok(())
    }

    pub fn installations(&self) -> Result<Vec<InstallationRow>> {
        let mut st = self.conn.prepare(
            "SELECT game_key, source, install_path, save_path, steam_app_id, detected_version, fingerprint,
                    install_manual, save_manual, last_scan
             FROM game_installations ORDER BY game_key",
        )?;
        let rows = st.query_map([], |r| {
            Ok(InstallationRow {
                game_key: r.get(0)?,
                source: r.get(1)?,
                install_path: r.get(2)?,
                save_path: r.get(3)?,
                steam_app_id: r.get(4)?,
                detected_version: r.get(5)?,
                fingerprint: r.get(6)?,
                install_manual: r.get(7)?,
                save_manual: r.get(8)?,
                last_scan: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn register_campaign(&self, id: &str, name: &str, path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO campaign_index(id, name, path, last_opened) VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, path = excluded.path, last_opened = datetime('now')",
            params![id, name, path],
        )?;
        Ok(())
    }

    pub fn campaigns(&self) -> Result<Vec<CampaignIndexRow>> {
        let mut st = self.conn.prepare(
            "SELECT id, name, path, created_at, last_opened FROM campaign_index
             ORDER BY COALESCE(last_opened, created_at) DESC, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(CampaignIndexRow {
                id: r.get(0)?,
                name: r.get(1)?,
                path: r.get(2)?,
                created_at: r.get(3)?,
                last_opened: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn forget_campaign(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM campaign_index WHERE id = ?1", [id])?;
        Ok(())
    }
}

// ======================================================================================
// Campaign database
// ======================================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignSummary {
    pub id: String,
    pub is_demo: bool,
    pub name: String,
    pub root: String,
    pub current_game: String,
    pub current_date: Option<PartialDate>,
    pub status: String,
    pub settings: CampaignSettings,
    pub event_count: u64,
    pub entity_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRow {
    pub event_id: i64,
    pub date: PartialDate,
    /// End of the window when `date_precision` is `range`.
    pub date_to: Option<PartialDate>,
    pub date_precision: String,
    pub game: String,
    pub event_type: String,
    pub actor_entity_id: Option<String>,
    pub target_entity_id: Option<String>,
    pub payload: Value,
    pub importance: u8,
    pub origin: String,
    pub evidence: String,
}

#[derive(Debug, Clone)]
pub struct NewEvent<'a> {
    pub date: PartialDate,
    /// Some(end) makes this a `range` event (happened between `date` and `end`).
    pub date_to: Option<PartialDate>,
    pub game: &'a str,
    pub game_version: Option<&'a str>,
    pub event_type: &'a str,
    pub actor: Option<&'a ChronicleId>,
    pub target: Option<&'a ChronicleId>,
    pub payload: Value,
    pub importance: u8,
    pub origin: EventOrigin,
}

impl<'a> NewEvent<'a> {
    /// A converter-made event with day precision and no actors.
    pub fn simple(date: PartialDate, game: &'a str, event_type: &'a str, origin: EventOrigin) -> Self {
        Self {
            date,
            date_to: None,
            game,
            game_version: None,
            event_type,
            actor: None,
            target: None,
            payload: Value::Object(Default::default()),
            importance: 1,
            origin,
        }
    }
}

pub struct CampaignDb {
    conn: Connection,
    root: PathBuf,
    id: String,
}

impl CampaignDb {
    pub fn db_path(root: &Path) -> PathBuf {
        root.join("database").join("campaign.db")
    }

    /// Create a new campaign in an empty (or not yet existing) folder.
    pub fn create(root: &Path, name: &str, settings: &CampaignSettings, registry: &GameRegistry) -> Result<Self> {
        settings.validate(registry).map_err(|e| DbError::Settings(e.to_string()))?;
        if root.exists() && std::fs::read_dir(root)?.next().is_some() {
            return Err(DbError::NotEmpty(root.to_path_buf()));
        }
        for d in CAMPAIGN_DIRS {
            std::fs::create_dir_all(root.join(d))?;
        }
        let mut conn = open_conn(&Self::db_path(root))?;
        migrate::migrate(&mut conn, migrate::CAMPAIGN_MIGRATIONS)?;

        let id = format!("campaign_{}", uuid::Uuid::new_v4().simple());
        let start = registry
            .game(&settings.start_game)
            .map(|g| g.era.start)
            .ok_or_else(|| DbError::Settings(format!("unknown start game {}", settings.start_game)))?;
        conn.execute(
            "INSERT INTO campaigns(id, name, current_game, current_year, current_month, current_day, seed,
                                   settings_json, chronicle_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                name,
                settings.start_game,
                start.year,
                start.month,
                start.day,
                settings.seed as i64,
                serde_json::to_string(settings)?,
                CHRONICLE_VERSION
            ],
        )?;
        for key in &registry.chain {
            if let Some(g) = registry.game(key) {
                conn.execute(
                    "INSERT OR IGNORE INTO games(game_id, name) VALUES (?1, ?2)",
                    params![key, g.display_name],
                )?;
            }
        }
        let db = Self { conn, root: root.to_path_buf(), id };
        db.add_event(&NewEvent {
            payload: serde_json::json!({ "name": name }),
            importance: 5,
            ..NewEvent::simple(start, &settings.start_game, "campaign_started", EventOrigin::Converter)
        })?;
        Ok(db)
    }

    pub fn open(root: &Path) -> Result<Self> {
        let path = Self::db_path(root);
        if !path.exists() {
            return Err(DbError::NotACampaign(root.to_path_buf()));
        }
        let mut conn = open_conn(&path)?;
        // Campaigns created by older Chronicle versions are upgraded in place
        // (a backup is taken first if anything needs to change).
        let from = migrate::current_version(&conn)?;
        let mut upgraded_backup = None;
        if from < migrate::latest(migrate::CAMPAIGN_MIGRATIONS) {
            std::fs::create_dir_all(root.join("backups"))?;
            let dest = root.join("backups").join(format!("campaign_{}_migration-v{from}.db", utc_stamp(unix_now())));
            conn.execute("VACUUM INTO ?1", [dest.to_string_lossy().into_owned()])?;
            migrate::migrate(&mut conn, migrate::CAMPAIGN_MIGRATIONS)?;
            let check = integrity(&conn)?;
            if check != "ok" {
                return Err(DbError::Integrity(check));
            }
            upgraded_backup = Some(dest);
        }
        let id: String = conn.query_row("SELECT id FROM campaigns LIMIT 1", [], |r| r.get(0))?;
        if let Some(dest) = upgraded_backup {
            conn.execute(
                "INSERT INTO backups(path, kind, label, integrity) VALUES (?1, 'migration', ?2, 'ok')",
                params![dest.to_string_lossy().into_owned(), format!("upgrade from v{from}")],
            )?;
        }
        Ok(Self { conn, root: root.to_path_buf(), id })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    pub(crate) fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn summary(&self) -> Result<CampaignSummary> {
        type Row = (String, String, Option<i32>, Option<u8>, Option<u8>, String, String, bool);
        let (name, game, y, m, d, status, settings_json, is_demo): Row = self.conn.query_row(
            "SELECT name, current_game, current_year, current_month, current_day, status, settings_json, is_demo
             FROM campaigns WHERE id = ?1",
            [&self.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )?;
        let event_count: i64 = self.conn.query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?;
        let entity_count: i64 = self.conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
        Ok(CampaignSummary {
            id: self.id.clone(),
            is_demo,
            name,
            root: self.root.display().to_string(),
            current_game: game,
            current_date: y.map(|year| PartialDate { year, month: m, day: d }),
            status,
            settings: serde_json::from_str(&settings_json)?,
            event_count: event_count as u64,
            entity_count: entity_count as u64,
        })
    }

    pub fn update_settings(&self, settings: &CampaignSettings, registry: &GameRegistry) -> Result<()> {
        settings.validate(registry).map_err(|e| DbError::Settings(e.to_string()))?;
        self.conn.execute(
            "UPDATE campaigns SET settings_json = ?1 WHERE id = ?2",
            params![serde_json::to_string(settings)?, self.id],
        )?;
        Ok(())
    }

    /// Allocate a new stable Chronicle id.
    pub fn next_id(&mut self, kind: EntityKind) -> Result<ChronicleId> {
        // A savepoint works both on its own and inside a bulk import transaction.
        let tx = self.conn.savepoint()?;
        tx.execute("INSERT OR IGNORE INTO id_counters(kind, next) VALUES (?1, 1)", [kind.as_str()])?;
        let n: i64 = tx.query_row("SELECT next FROM id_counters WHERE kind = ?1", [kind.as_str()], |r| r.get(0))?;
        tx.execute("UPDATE id_counters SET next = next + 1 WHERE kind = ?1", [kind.as_str()])?;
        let id = ChronicleId::new(kind, n as u64);
        tx.execute("INSERT INTO entities(id, kind) VALUES (?1, ?2)", params![id.as_str(), kind.as_str()])?;
        tx.commit()?;
        Ok(id)
    }

    pub fn add_event(&self, e: &NewEvent<'_>) -> Result<i64> {
        let precision = if e.date_to.is_some() { DatePrecision::Range } else { DatePrecision::of(&e.date) };
        let evidence: EventEvidence = e.origin.evidence();
        self.conn.execute(
            "INSERT INTO events(campaign_id, year, month, day, date_key, date_to_year, date_to_month, date_to_day,
                                date_precision, game, game_version, event_type, actor_entity_id, target_entity_id,
                                payload, importance, origin, evidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                self.id,
                e.date.year,
                e.date.month,
                e.date.day,
                e.date.sort_key(),
                e.date_to.map(|d| d.year),
                e.date_to.and_then(|d| d.month),
                e.date_to.and_then(|d| d.day),
                precision.as_str(),
                e.game,
                e.game_version,
                e.event_type,
                e.actor.map(ChronicleId::as_str),
                e.target.map(ChronicleId::as_str),
                serde_json::to_string(&e.payload)?,
                e.importance,
                e.origin.as_str(),
                evidence.as_str()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Chronological journal; `min_importance` filters minor events.
    pub fn events(&self, min_importance: u8, limit: u32) -> Result<Vec<EventRow>> {
        let mut st = self.conn.prepare(
            "SELECT event_id, year, month, day, date_precision, game, event_type, actor_entity_id,
                    target_entity_id, payload, importance, origin, evidence, date_to_year, date_to_month, date_to_day
             FROM events WHERE importance >= ?1
             ORDER BY date_key, event_id
             LIMIT ?2",
        )?;
        let rows = st.query_map(params![min_importance, limit], |r| {
            let payload: String = r.get(9)?;
            Ok(EventRow {
                event_id: r.get(0)?,
                date: PartialDate { year: r.get(1)?, month: r.get(2)?, day: r.get(3)? },
                date_to: r
                    .get::<_, Option<i32>>(13)?
                    .map(|year| PartialDate { year, month: r.get(14).ok().flatten(), day: r.get(15).ok().flatten() }),
                date_precision: r.get(4)?,
                game: r.get(5)?,
                event_type: r.get(6)?,
                actor_entity_id: r.get(7)?,
                target_entity_id: r.get(8)?,
                payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
                importance: r.get(10)?,
                origin: r.get(11)?,
                evidence: r.get(12)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Consistent copy of the whole database into `backups/` (safe while open), registered in
    /// the `backups` table, followed by an integrity check of the live database.
    /// Retention: only the newest [`AUTO_BACKUPS_KEPT`] `Auto` backups are kept.
    pub fn backup(&self, kind: BackupKind, label: &str) -> Result<PathBuf> {
        let safe: String = label.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let dir = self.root.join("backups");
        std::fs::create_dir_all(&dir)?;
        let mut dest = dir.join(format!("campaign_{}_{}_{safe}.db", utc_stamp(unix_now()), kind.as_str()));
        let mut n = 1;
        while dest.exists() {
            n += 1;
            dest = dir.join(format!("campaign_{}_{}_{safe}_{n}.db", utc_stamp(unix_now()), kind.as_str()));
        }
        let dest_str: String = dest.to_string_lossy().into_owned();
        self.conn.execute("VACUUM INTO ?1", [&dest_str])?;
        let check = integrity(&self.conn)?;
        self.conn.execute(
            "INSERT INTO backups(path, kind, label, integrity) VALUES (?1, ?2, ?3, ?4)",
            params![dest_str, kind.as_str(), label, check],
        )?;
        if check != "ok" {
            return Err(DbError::Integrity(check));
        }
        if kind == BackupKind::Auto {
            self.prune_auto_backups()?;
        }
        Ok(dest)
    }

    fn prune_auto_backups(&self) -> Result<()> {
        let mut st = self.conn.prepare(
            "SELECT id, path FROM backups WHERE kind = 'auto' ORDER BY id DESC LIMIT -1 OFFSET ?1",
        )?;
        let old: Vec<(i64, String)> = st
            .query_map([AUTO_BACKUPS_KEPT as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        for (id, path) in old {
            let _ = std::fs::remove_file(&path); // already gone is fine
            self.conn.execute("DELETE FROM backups WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    pub fn backups(&self) -> Result<Vec<(String, String, String)>> {
        let mut st = self.conn.prepare("SELECT kind, path, created_at FROM backups ORDER BY id DESC")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn integrity_check(&self) -> Result<()> {
        let check = integrity(&self.conn)?;
        if check == "ok" { Ok(()) } else { Err(DbError::Integrity(check)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg() -> GameRegistry {
        GameRegistry::builtin().unwrap()
    }

    fn row(game: &str, path: &str) -> InstallationRow {
        InstallationRow {
            game_key: game.into(),
            source: "steam".into(),
            install_path: Some(path.into()),
            save_path: Some("auto-saves".into()),
            steam_app_id: Some(1158310),
            detected_version: None,
            fingerprint: "unverified".into(),
            install_manual: false,
            save_manual: false,
            last_scan: None,
        }
    }

    #[test]
    fn settings_roundtrip() {
        let db = AppDb::open_in_memory().unwrap();
        assert_eq!(db.get_setting("language").unwrap(), None);
        db.set_setting("language", "uk").unwrap();
        db.set_setting("language", "en").unwrap();
        assert_eq!(db.get_setting("language").unwrap().as_deref(), Some("en"));
    }

    #[test]
    fn manual_paths_survive_rescan() {
        let db = AppDb::open_in_memory().unwrap();
        db.upsert_detected(&row("ck3", "C:/Steam/ck3")).unwrap();
        db.set_manual_install("ck3", "D:/Games/CK3", "verified").unwrap();
        db.set_manual_save("ck3", "E:/Saves/CK3").unwrap();
        db.upsert_detected(&row("ck3", "C:/Steam/ck3-new")).unwrap();
        let r = &db.installations().unwrap()[0];
        assert_eq!(r.install_path.as_deref(), Some("D:/Games/CK3"));
        assert_eq!(r.save_path.as_deref(), Some("E:/Saves/CK3"));
        assert_eq!(r.source, "manual");
        db.reset_manual("ck3").unwrap();
        db.upsert_detected(&row("ck3", "C:/Steam/ck3-new")).unwrap();
        assert_eq!(db.installations().unwrap()[0].install_path.as_deref(), Some("C:/Steam/ck3-new"));
    }

    #[test]
    fn campaign_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("My World");
        let settings = CampaignSettings::new("ck3", 45819283);
        let mut db = CampaignDb::create(&root, "My World", &settings, &reg()).unwrap();
        for d in CAMPAIGN_DIRS {
            assert!(root.join(d).is_dir(), "{d} missing");
        }

        let a = db.next_id(EntityKind::Country).unwrap();
        let b = db.next_id(EntityKind::Country).unwrap();
        assert_eq!(a.as_str(), "chronicle_country_000001");
        assert_eq!(b.as_str(), "chronicle_country_000002");

        db.add_event(&NewEvent {
            game_version: Some("test"),
            actor: Some(&a),
            importance: 3,
            ..NewEvent::simple(PartialDate { year: 1092, month: Some(4), day: None }, "ck3", "civil_war", EventOrigin::Save)
        })
        .unwrap();
        // Inferred from two snapshots: capital moved somewhere between 1200 and 1205.
        db.add_event(&NewEvent {
            date_to: Some(PartialDate::year(1205)),
            actor: Some(&b),
            importance: 2,
            ..NewEvent::simple(PartialDate::year(1200), "ck3", "capital_changed", EventOrigin::SnapshotDiff)
        })
        .unwrap();
        let ev = db.events(0, 100).unwrap();
        assert_eq!(ev.len(), 3);
        assert_eq!((ev[0].event_type.as_str(), ev[0].origin.as_str()), ("campaign_started", "converter"));
        assert_eq!(ev[0].date, PartialDate::ymd(867, 1, 1));
        assert_eq!((ev[1].date_precision.as_str(), ev[1].evidence.as_str()), ("month", "observed"));
        assert_eq!((ev[2].date_precision.as_str(), ev[2].evidence.as_str()), ("range", "inferred"));
        assert_eq!(ev[2].date_to, Some(PartialDate::year(1205)));
        assert_eq!(db.events(4, 100).unwrap().len(), 1);

        let s = db.summary().unwrap();
        assert_eq!((s.current_game.as_str(), s.event_count, s.entity_count), ("ck3", 3, 2));

        let backup = db.backup(BackupKind::Manual, "before import").unwrap();
        assert!(backup.exists());
        drop(db);

        let reopened = CampaignDb::open(&root).unwrap();
        assert_eq!(reopened.summary().unwrap().name, "My World");
    }

    #[test]
    fn auto_backup_retention_keeps_ten_and_all_transitions() {
        let tmp = tempfile::tempdir().unwrap();
        let db = CampaignDb::create(&tmp.path().join("c"), "x", &CampaignSettings::new("ck3", 1), &reg()).unwrap();
        let t = db.backup(BackupKind::Transition, "ck3 to eu5").unwrap();
        for i in 0..13 {
            db.backup(BackupKind::Auto, &format!("import {i}")).unwrap();
        }
        let all = db.backups().unwrap();
        assert_eq!(all.iter().filter(|b| b.0 == "auto").count(), AUTO_BACKUPS_KEPT);
        assert_eq!(all.iter().filter(|b| b.0 == "transition").count(), 1);
        assert!(t.exists(), "transition backups are never pruned");
        let files = std::fs::read_dir(tmp.path().join("c/backups")).unwrap().count();
        assert_eq!(files, AUTO_BACKUPS_KEPT + 1);
        db.integrity_check().unwrap();
    }

    #[test]
    fn utc_stamp_format() {
        assert_eq!(utc_stamp(0), "1970-01-01_000000");
        assert_eq!(utc_stamp(1_791_582_901), "2026-10-09_215501");
    }

    #[test]
    fn refuses_non_empty_folder_and_bad_settings() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("something.txt"), "x").unwrap();
        let s = CampaignSettings::new("ck3", 1);
        assert!(matches!(CampaignDb::create(tmp.path(), "x", &s, &reg()), Err(DbError::NotEmpty(_))));

        let mut bad = CampaignSettings::new("ck3", 1);
        bad.confidence.auto_min = 0.2;
        let fresh = tmp.path().join("fresh");
        assert!(matches!(CampaignDb::create(&fresh, "x", &bad, &reg()), Err(DbError::Settings(_))));
    }
}
