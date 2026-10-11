//! Writing imported world state: stable ids by game id, upserts of current state, save files,
//! snapshots and semantic values with provenance. Used by game importers (chronicle-ck3, …).

use std::collections::BTreeMap;

use chronicle_core::{ChronicleId, ConfidenceComponents, ConfidencePolicy, EntityKind, PartialDate};
use rusqlite::{OptionalExtension, params};

use crate::{CampaignDb, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedTable {
    Culture,
    Religion,
}

/// One derived value with everything the "Why?" view needs.
#[derive(Debug, Clone)]
pub struct SemanticInput<'a> {
    pub entity: &'a ChronicleId,
    pub date: PartialDate,
    pub key: &'a str,
    pub value: f64,
    pub source_game: &'a str,
    pub source_fields: &'a [&'a str],
    pub contributions: BTreeMap<String, f64>,
    pub formula: &'a str,
    pub components: ConfidenceComponents,
    pub adapter_version: &'a str,
}

impl CampaignDb {
    /// Stable Chronicle id for a game object (e.g. game "ck3", id "title:k_gujarat").
    /// Returns (id, created). The same game id always maps to the same Chronicle id.
    pub fn entity_for(&mut self, game: &str, game_id: &str, kind: EntityKind) -> Result<(ChronicleId, bool)> {
        let existing: Option<String> = self
            .conn()
            .query_row(
                "SELECT entity_id FROM game_entity_mappings WHERE game_id = ?1 AND game_entity_id = ?2 LIMIT 1",
                params![game, game_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing.and_then(|s| s.parse::<ChronicleId>().ok()) {
            return Ok((id, false));
        }
        let id = self.next_id(kind)?;
        self.conn().execute(
            "INSERT INTO game_entity_mappings(entity_id, game_id, game_entity_id) VALUES (?1, ?2, ?3)",
            params![id.as_str(), game, game_id],
        )?;
        Ok((id, true))
    }

    pub fn upsert_country(
        &self,
        id: &ChronicleId,
        name: &str,
        culture: Option<&ChronicleId>,
        religion: Option<&ChronicleId>,
        founded: Option<PartialDate>,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT INTO countries(entity_id, current_name, primary_culture, religion, founded_year, founded_month, founded_day)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(entity_id) DO UPDATE SET current_name = excluded.current_name,
                 primary_culture = excluded.primary_culture, religion = excluded.religion,
                 founded_year = COALESCE(countries.founded_year, excluded.founded_year),
                 founded_month = COALESCE(countries.founded_month, excluded.founded_month),
                 founded_day = COALESCE(countries.founded_day, excluded.founded_day)",
            params![
                id.as_str(),
                name,
                culture.map(ChronicleId::as_str),
                religion.map(ChronicleId::as_str),
                founded.map(|d| d.year),
                founded.and_then(|d| d.month),
                founded.and_then(|d| d.day)
            ],
        )?;
        // Keep the name history: open a new name row only when the name changed.
        let current: Option<String> = self
            .conn()
            .query_row(
                "SELECT name FROM country_names WHERE country_id = ?1 AND end_year IS NULL ORDER BY rowid DESC LIMIT 1",
                [id.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        if current.as_deref() != Some(name) {
            self.conn().execute(
                "INSERT INTO country_names(country_id, name, start_year) VALUES (?1, ?2, ?3)",
                params![id.as_str(), name, founded.map(|d| d.year)],
            )?;
        }
        Ok(())
    }

    pub fn upsert_territory(&self, id: &ChronicleId, name: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO territories(entity_id, name) VALUES (?1, ?2)
             ON CONFLICT(entity_id) DO UPDATE SET name = excluded.name",
            params![id.as_str(), name],
        )?;
        Ok(())
    }

    pub fn upsert_named(&self, table: NamedTable, id: &ChronicleId, name: &str) -> Result<()> {
        let sql = match table {
            NamedTable::Culture => {
                "INSERT INTO cultures(entity_id, name) VALUES (?1, ?2) ON CONFLICT(entity_id) DO UPDATE SET name = excluded.name"
            }
            NamedTable::Religion => {
                "INSERT INTO religions(entity_id, name) VALUES (?1, ?2) ON CONFLICT(entity_id) DO UPDATE SET name = excluded.name"
            }
        };
        self.conn().execute(sql, params![id.as_str(), name])?;
        Ok(())
    }

    pub fn upsert_dynasty(&self, id: &ChronicleId, name: &str, founded: Option<PartialDate>) -> Result<()> {
        self.conn().execute(
            "INSERT INTO dynasties(entity_id, name, founded_year) VALUES (?1, ?2, ?3)
             ON CONFLICT(entity_id) DO UPDATE SET name = excluded.name,
                 founded_year = COALESCE(dynasties.founded_year, excluded.founded_year)",
            params![id.as_str(), name, founded.map(|d| d.year)],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_character(
        &self,
        id: &ChronicleId,
        name: &str,
        birth: Option<PartialDate>,
        dynasty: Option<&ChronicleId>,
        culture: Option<&ChronicleId>,
        religion: Option<&ChronicleId>,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT INTO characters(entity_id, name, birth_year, birth_month, birth_day, dynasty_id, culture, religion)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(entity_id) DO UPDATE SET name = excluded.name, dynasty_id = excluded.dynasty_id,
                 culture = excluded.culture, religion = excluded.religion",
            params![
                id.as_str(),
                name,
                birth.map(|d| d.year),
                birth.and_then(|d| d.month),
                birth.and_then(|d| d.day),
                dynasty.map(ChronicleId::as_str),
                culture.map(ChronicleId::as_str),
                religion.map(ChronicleId::as_str)
            ],
        )?;
        Ok(())
    }

    /// True if an event with this type, date and payload `key` is already in the journal
    /// (re-importing the same save must not duplicate history).
    pub fn event_exists(&self, event_type: &str, date: PartialDate, key: &str) -> Result<bool> {
        let n: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM events WHERE event_type = ?1 AND date_key = ?2 AND json_extract(payload, '$.key') = ?3",
            params![event_type, date.sort_key(), key],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    pub fn set_current(&self, game: &str, date: PartialDate) -> Result<()> {
        self.conn().execute(
            "UPDATE campaigns SET current_game = ?1, current_year = ?2, current_month = ?3, current_day = ?4",
            params![game, date.year, date.month, date.day],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_save_file(
        &self,
        game: &str,
        original_path: &str,
        stored_path: &str,
        sha256: &str,
        date: Option<PartialDate>,
        game_version: Option<&str>,
    ) -> Result<i64> {
        self.conn().execute(
            "INSERT INTO save_files(game_id, original_path, stored_path, sha256, in_game_year, in_game_month, in_game_day, game_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(sha256) DO UPDATE SET original_path = excluded.original_path",
            params![
                game,
                original_path,
                stored_path,
                sha256,
                date.map(|d| d.year),
                date.and_then(|d| d.month),
                date.and_then(|d| d.day),
                game_version
            ],
        )?;
        Ok(self.conn().query_row("SELECT id FROM save_files WHERE sha256 = ?1", [sha256], |r| r.get(0))?)
    }

    pub fn record_snapshot(
        &self,
        game: &str,
        date: PartialDate,
        kind: &str,
        path: &str,
        checksum: &str,
        save_file_id: Option<i64>,
    ) -> Result<i64> {
        self.conn().execute(
            "INSERT INTO snapshots(game, year, month, day, type, path, checksum, save_file_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![game, date.year, date.month, date.day, kind, path, checksum, save_file_id],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    /// Date of the newest snapshot of `game` strictly before `before` (for "between snapshots" events).
    pub fn previous_snapshot_date(&self, game: &str, before: PartialDate) -> Result<Option<PartialDate>> {
        let row: Option<(i32, Option<u8>, Option<u8>)> = self
            .conn()
            .query_row(
                "SELECT year, month, day FROM snapshots
                 WHERE game = ?1 AND year * 10000 + COALESCE(month, 0) * 100 + COALESCE(day, 0) < ?2
                 ORDER BY year DESC, COALESCE(month, 0) DESC, COALESCE(day, 0) DESC LIMIT 1",
                params![game, before.sort_key()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        Ok(row.map(|(year, month, day)| PartialDate { year, month, day }))
    }

    /// Mark an era (one game of the chain) planned / active / completed.
    pub fn set_era(&self, game: &str, status: &str, date: PartialDate, final_snapshot: Option<i64>, conversion: Option<i64>) -> Result<()> {
        let (start, end) = match status {
            "completed" => (None, Some(date)),
            "skipped" => (None, None),
            _ => (Some(date), None),
        };
        self.conn().execute(
            "INSERT INTO eras(game, status, started_year, started_month, started_day, completed_year, completed_month,
                              completed_day, final_snapshot, conversion_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(game) DO UPDATE SET status = excluded.status,
                 started_year = COALESCE(eras.started_year, excluded.started_year),
                 started_month = COALESCE(eras.started_month, excluded.started_month),
                 started_day = COALESCE(eras.started_day, excluded.started_day),
                 completed_year = COALESCE(excluded.completed_year, eras.completed_year),
                 completed_month = COALESCE(excluded.completed_month, eras.completed_month),
                 completed_day = COALESCE(excluded.completed_day, eras.completed_day),
                 final_snapshot = COALESCE(excluded.final_snapshot, eras.final_snapshot),
                 conversion_id = COALESCE(excluded.conversion_id, eras.conversion_id),
                 updated_at = datetime('now')",
            params![
                game,
                status,
                start.map(|d| d.year),
                start.and_then(|d| d.month),
                start.and_then(|d| d.day),
                end.map(|d| d.year),
                end.and_then(|d| d.month),
                end.and_then(|d| d.day),
                final_snapshot,
                conversion
            ],
        )?;
        Ok(())
    }

    /// (game, status) of every era recorded so far.
    pub fn eras(&self) -> Result<Vec<(String, String)>> {
        let mut st = self.conn().prepare("SELECT game, status FROM eras ORDER BY rowid")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Record a conversion run (reproducibility: inputs, versions, seed).
    #[allow(clippy::too_many_arguments)]
    pub fn record_conversion(
        &self,
        source_game: &str,
        target_game: &str,
        source_snapshot: i64,
        source_checksum: &str,
        date: PartialDate,
        adapter_versions: &serde_json::Value,
        coefficient_profile: &str,
        seed: u64,
    ) -> Result<i64> {
        self.conn().execute(
            "INSERT INTO conversions(source_game, target_game, source_snapshot, source_checksum, year, month, day,
                                     converter_version, adapter_versions, rules_version, coefficient_profile, seed, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'preview')",
            params![
                source_game,
                target_game,
                source_snapshot,
                source_checksum,
                date.year,
                date.month,
                date.day,
                chronicle_core::CHRONICLE_VERSION,
                serde_json::to_string(adapter_versions)?,
                "1",
                coefficient_profile,
                seed as i64
            ],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub fn set_conversion_status(&self, id: i64, status: &str) -> Result<()> {
        self.conn().execute("UPDATE conversions SET status = ?1 WHERE conversion_id = ?2", params![status, id])?;
        Ok(())
    }

    pub fn add_conversion_warning(&self, id: i64, severity: &str, entity: Option<&str>, message: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO conversion_warnings(conversion_id, severity, entity_id, message) VALUES (?1, ?2, ?3, ?4)",
            params![id, severity, entity, message],
        )?;
        Ok(())
    }

    pub fn snapshot_count(&self, game: &str) -> Result<i64> {
        Ok(self.conn().query_row("SELECT COUNT(*) FROM snapshots WHERE game = ?1", [game], |r| r.get(0))?)
    }

    /// Semantic values dated `date` that wait for the player's review.
    pub fn pending_review_count(&self, date: PartialDate) -> Result<i64> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM semantic_values WHERE year = ?1 AND COALESCE(month, 0) = ?2 AND COALESCE(day, 0) = ?3
               AND review_state = 'pending_review'",
            params![date.year, date.month.unwrap_or(0), date.day.unwrap_or(0)],
            |r| r.get(0),
        )?)
    }

    /// True if this entity already has `key` on `date` from `source_game` (re-import guard).
    pub fn semantic_exists(&self, entity: &ChronicleId, key: &str, date: PartialDate, source_game: &str) -> Result<bool> {
        let n: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM semantic_values s JOIN provenance p ON p.id = s.provenance_id
             WHERE s.entity_id = ?1 AND s.semantic_key = ?2 AND s.year = ?3
               AND COALESCE(s.month, 0) = ?4 AND COALESCE(s.day, 0) = ?5 AND p.source_game = ?6",
            params![entity.as_str(), key, date.year, date.month.unwrap_or(0), date.day.unwrap_or(0), source_game],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Store one semantic value + provenance; the review state follows the campaign policy.
    pub fn add_semantic(&self, s: &SemanticInput<'_>, policy: &ConfidencePolicy) -> Result<()> {
        let confidence = s.components.confidence();
        let review = policy.decide(&s.components).review_state();
        self.conn().execute(
            "INSERT INTO provenance(source_game, source_fields, contributions, formula_id, confidence, adapter_version,
                                    data_coverage, mapping_reliability, version_supported, validation_passed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                s.source_game,
                serde_json::to_string(s.source_fields)?,
                serde_json::to_string(&s.contributions)?,
                s.formula,
                confidence,
                s.adapter_version,
                s.components.data_coverage,
                s.components.mapping_reliability,
                s.components.version_supported,
                s.components.validation_passed
            ],
        )?;
        let prov = self.conn().last_insert_rowid();
        self.conn().execute(
            "INSERT INTO semantic_values(entity_id, year, month, day, semantic_key, value, confidence, provenance_id, review_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![s.entity.as_str(), s.date.year, s.date.month, s.date.day, s.key, s.value, confidence, prov, review],
        )?;
        Ok(())
    }

    /// Group many writes into one transaction (much faster for bulk imports: one disk flush
    /// instead of thousands). Period setters and id allocation use savepoints, so they nest.
    pub fn begin_bulk(&self) -> Result<()> {
        // A transaction left open by an earlier failure would block this one: undo it.
        if !self.conn().is_autocommit() {
            self.conn().execute_batch("ROLLBACK")?;
        }
        self.conn().execute_batch("BEGIN")?;
        Ok(())
    }

    pub fn commit_bulk(&self) -> Result<()> {
        self.conn().execute_batch("COMMIT")?;
        Ok(())
    }

    pub fn rollback_bulk(&self) {
        let _ = self.conn().execute_batch("ROLLBACK");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CampaignSettings;
    use chronicle_core::registry::GameRegistry;

    #[test]
    fn periods_and_ids_nest_inside_bulk_transaction() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = CampaignDb::create(&tmp.path().join("c"), "x", &CampaignSettings::new("ck3", 1),
                                        &GameRegistry::builtin().unwrap()).unwrap();
        db.begin_bulk().unwrap();
        let t = db.next_id(EntityKind::Territory).unwrap();
        let c = db.next_id(EntityKind::Country).unwrap();
        db.set_owner(&t, Some(&c), PartialDate::year(900), "ck3", "save").unwrap();
        db.commit_bulk().unwrap();
        assert!(db.owner_at(t.as_str(), PartialDate::year(901)).unwrap().is_some());

        db.begin_bulk().unwrap();
        let lost = db.next_id(EntityKind::Country).unwrap();
        db.rollback_bulk();
        let n: i64 = db.conn().query_row("SELECT COUNT(*) FROM entities WHERE id = ?1", [lost.as_str()], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "rollback undoes everything in the bulk transaction");
    }

    #[test]
    fn entity_ids_are_stable_per_game_id() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = CampaignDb::create(&tmp.path().join("c"), "x", &CampaignSettings::new("ck3", 1),
                                        &GameRegistry::builtin().unwrap()).unwrap();
        let (a, created_a) = db.entity_for("ck3", "title:k_gujarat", EntityKind::Country).unwrap();
        let (b, created_b) = db.entity_for("ck3", "title:k_gujarat", EntityKind::Country).unwrap();
        assert!(created_a && !created_b);
        assert_eq!(a, b);
        db.upsert_country(&a, "Гуджарат", None, None, None).unwrap();
        db.upsert_country(&a, "Гуджарат", None, None, None).unwrap();
        db.upsert_country(&a, "Расаладевииды", None, None, None).unwrap();
        let names: i64 = db.conn().query_row("SELECT COUNT(*) FROM country_names", [], |r| r.get(0)).unwrap();
        assert_eq!(names, 2, "a new name row only when the name changes");
    }
}
