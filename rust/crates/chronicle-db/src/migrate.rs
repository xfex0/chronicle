//! Versioned migrations. Each migration runs exactly once, inside a transaction, and is
//! recorded in `schema_version`. Never edit a released migration; add a new one.

use rusqlite::Connection;

pub struct Migration {
    pub version: u32,
    pub sql: &'static str,
    /// Table rebuilds (SQLite cannot alter constraints) need foreign keys off during the
    /// migration; they are checked with `PRAGMA foreign_key_check` before re-enabling.
    pub foreign_keys_off: bool,
}

const fn m(version: u32, sql: &'static str) -> Migration {
    Migration { version, sql, foreign_keys_off: false }
}

const fn rebuild(version: u32, sql: &'static str) -> Migration {
    Migration { version, sql, foreign_keys_off: true }
}

pub const APP_MIGRATIONS: &[Migration] = &[
    m(1, include_str!("../migrations/app_0001.sql")),
    rebuild(2, include_str!("../migrations/app_0002.sql")),
];

pub const CAMPAIGN_MIGRATIONS: &[Migration] = &[
    m(1, include_str!("../migrations/campaign_0001.sql")),
    m(2, include_str!("../migrations/campaign_0002.sql")),
    rebuild(3, include_str!("../migrations/campaign_0003.sql")),
];

pub fn latest(migrations: &[Migration]) -> u32 {
    migrations.last().map_or(0, |m| m.version)
}

pub fn current_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
             version INTEGER PRIMARY KEY,
             applied_at TEXT NOT NULL DEFAULT (datetime('now')))",
    )?;
    conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))
}

/// Apply every migration newer than the database. Returns the versions applied.
pub fn migrate(conn: &mut Connection, migrations: &[Migration]) -> rusqlite::Result<Vec<u32>> {
    let current = current_version(conn)?;
    let mut applied = Vec::new();
    for mig in migrations.iter().filter(|m| m.version > current) {
        if mig.foreign_keys_off {
            // Must be set outside a transaction to take effect.
            conn.execute_batch("PRAGMA foreign_keys = OFF")?;
        }
        let result = (|| {
            let tx = conn.transaction()?;
            tx.execute_batch(mig.sql)?;
            if mig.foreign_keys_off {
                let broken: i64 = tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
                if broken > 0 {
                    return Err(rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
                        Some(format!("migration {} left {broken} broken foreign keys", mig.version)),
                    ));
                }
            }
            tx.execute("INSERT INTO schema_version(version) VALUES (?1)", [mig.version])?;
            tx.commit()
        })();
        if mig.foreign_keys_off {
            conn.execute_batch("PRAGMA foreign_keys = ON")?;
        }
        result?;
        applied.push(mig.version);
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_old_database_step_by_step_once() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        assert_eq!(migrate(&mut conn, &CAMPAIGN_MIGRATIONS[..1]).unwrap(), vec![1]);
        conn.execute_batch(
            "INSERT INTO campaigns(id, name, current_game, seed, settings_json, chronicle_version)
                 VALUES ('c', 'x', 'ck3', 1, '{}', 'old');
             INSERT INTO events(campaign_id, year, month, day, date_precision, game, event_type, source)
                 VALUES ('c', 1092, 4, NULL, 'exact', 'ck3', 'civil_war', 'save');",
        )
        .unwrap();
        assert_eq!(migrate(&mut conn, CAMPAIGN_MIGRATIONS).unwrap(), vec![2, 3]);
        assert_eq!(migrate(&mut conn, CAMPAIGN_MIGRATIONS).unwrap(), Vec::<u32>::new(), "idempotent");

        let (precision, origin, evidence): (String, String, String) = conn
            .query_row("SELECT date_precision, origin, evidence FROM events", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((precision.as_str(), origin.as_str(), evidence.as_str()), ("month", "save", "observed"));
        let fk_on: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
        assert_eq!(fk_on, 1, "foreign keys re-enabled after a rebuild migration");
    }

    #[test]
    fn app_migrations_apply() {
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(migrate(&mut conn, APP_MIGRATIONS).unwrap(), vec![1, 2]);
        conn.execute("INSERT INTO game_installations(game_key, source) VALUES ('eu5', 'gog')", []).unwrap();
    }
}
