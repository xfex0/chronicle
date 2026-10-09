//! Developer mode: a fictional demo world to exercise every table, a self-test of database
//! invariants, and a read-only SQL console. Demo data is always tagged (`campaigns.is_demo`,
//! `source = 'demo'`, event payload `"demo": true`) so it can never pass for real history.

use std::collections::BTreeMap;

use chronicle_core::{ChronicleId, ConfidenceComponents, ConfidencePolicy, EntityKind, EventOrigin, PartialDate};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::migrate::{CAMPAIGN_MIGRATIONS, current_version, latest};
use crate::periods::Aspect;
use crate::{BackupKind, CampaignDb, DbError, NewEvent, Result};

#[derive(Debug, Clone, Serialize)]
pub struct DemoSummary {
    pub countries: usize,
    pub territories: usize,
    pub ownership_changes: usize,
    pub periods: usize,
    pub events: usize,
    pub semantic_values: usize,
    pub geo_areas: usize,
}

fn d(s: &str) -> PartialDate {
    s.parse().expect("valid demo date")
}

struct Demo<'a> {
    db: &'a mut CampaignDb,
    ids: BTreeMap<&'static str, ChronicleId>,
    summary: DemoSummary,
}

impl Demo<'_> {
    fn id(&self, key: &str) -> &ChronicleId {
        &self.ids[key]
    }

    fn entity(&mut self, key: &'static str, kind: EntityKind, name: &str) -> Result<()> {
        let id = self.db.next_id(kind)?;
        let c = self.db.conn();
        match kind {
            EntityKind::Country => {
                c.execute("INSERT INTO countries(entity_id, current_name) VALUES (?1, ?2)", params![id.as_str(), name])?;
                self.summary.countries += 1;
            }
            EntityKind::Territory => {
                c.execute("INSERT INTO territories(entity_id, name) VALUES (?1, ?2)", params![id.as_str(), name])?;
                self.summary.territories += 1;
            }
            EntityKind::Dynasty => {
                c.execute("INSERT INTO dynasties(entity_id, name) VALUES (?1, ?2)", params![id.as_str(), name])?;
            }
            EntityKind::Culture => {
                c.execute("INSERT INTO cultures(entity_id, name) VALUES (?1, ?2)", params![id.as_str(), name])?;
            }
            EntityKind::Religion => {
                c.execute("INSERT INTO religions(entity_id, name) VALUES (?1, ?2)", params![id.as_str(), name])?;
            }
            _ => {}
        }
        c.execute(
            "INSERT INTO game_entity_mappings(entity_id, game_id, game_entity_id) VALUES (?1, 'ck3', ?2)",
            params![id.as_str(), format!("demo_{key}")],
        )?;
        self.ids.insert(key, id);
        Ok(())
    }

    fn own(&mut self, territory: &str, owner: &str, date: &str) -> Result<()> {
        let (t, o) = (self.id(territory).clone(), self.id(owner).clone());
        self.db.set_owner(&t, Some(&o), d(date), "ck3", "demo")?;
        self.summary.ownership_changes += 1;
        Ok(())
    }

    fn period(&mut self, entity: &str, aspect: Aspect, value: Option<&str>, value_entity: Option<&str>, date: &str) -> Result<()> {
        let e = self.id(entity).clone();
        let ve = value_entity.map(|k| self.id(k).clone());
        self.db.set_period(&e, aspect, value, ve.as_ref(), d(date), "ck3", "demo")?;
        self.summary.periods += 1;
        Ok(())
    }

    fn event(&mut self, date: &str, kind: &str, actor: Option<&str>, target: Option<&str>, importance: u8, payload: Value) -> Result<()> {
        self.event_full(date, None, EventOrigin::Save, kind, actor, target, importance, payload)
    }

    #[allow(clippy::too_many_arguments)]
    fn event_full(
        &mut self,
        date: &str,
        date_to: Option<&str>,
        origin: EventOrigin,
        kind: &str,
        actor: Option<&str>,
        target: Option<&str>,
        importance: u8,
        payload: Value,
    ) -> Result<()> {
        let a = actor.map(|k| self.id(k).clone());
        let t = target.map(|k| self.id(k).clone());
        let mut payload = payload;
        payload["demo"] = json!(true);
        self.db.add_event(&NewEvent {
            date_to: date_to.map(d),
            game_version: Some("demo"),
            actor: a.as_ref(),
            target: t.as_ref(),
            payload,
            importance,
            ..NewEvent::simple(d(date), "ck3", kind, origin)
        })?;
        self.summary.events += 1;
        Ok(())
    }

    /// One semantic value with full provenance ("Why?" data) and confidence components.
    #[allow(clippy::too_many_arguments)]
    fn semantic(
        &mut self,
        entity: &str,
        date: &str,
        key: &str,
        value: f64,
        contributions: &[(&str, f64)],
        formula: &str,
        comp: ConfidenceComponents,
        policy: &ConfidencePolicy,
    ) -> Result<()> {
        let confidence = comp.confidence();
        let review = policy.decide(&comp).review_state();
        let fields: Vec<&str> = contributions.iter().map(|(f, _)| *f).collect();
        let contrib: BTreeMap<&str, f64> = contributions.iter().copied().collect();
        let c = self.db.conn();
        c.execute(
            "INSERT INTO provenance(source_game, source_fields, contributions, formula_id, confidence, adapter_version,
                                    data_coverage, mapping_reliability, version_supported, validation_passed)
             VALUES ('ck3', ?1, ?2, ?3, ?4, 'demo', ?5, ?6, ?7, ?8)",
            params![
                serde_json::to_string(&fields)?,
                serde_json::to_string(&contrib)?,
                formula,
                confidence,
                comp.data_coverage,
                comp.mapping_reliability,
                comp.version_supported,
                comp.validation_passed
            ],
        )?;
        let prov = c.last_insert_rowid();
        let date = d(date);
        c.execute(
            "INSERT INTO semantic_values(entity_id, year, month, day, semantic_key, value, confidence, provenance_id, review_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![self.id(entity).as_str(), date.year, date.month, date.day, key, value, confidence, prov, review],
        )?;
        self.summary.semantic_values += 1;
        Ok(())
    }
}

/// Fill an empty campaign with a small fictional world (names are invented on purpose).
pub fn seed_demo(db: &mut CampaignDb) -> Result<DemoSummary> {
    let entities: i64 = db.conn().query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
    // Only into an empty campaign: never mixes with real history, and never seeds twice.
    if entities > 0 {
        return Err(DbError::NotDemoSafe);
    }
    let policy = db.summary()?.settings.confidence;
    db.backup(BackupKind::Auto, "before demo")?;
    db.conn().execute("UPDATE campaigns SET is_demo = 1, current_year = 1336, current_month = 1, current_day = 1", [])?;

    let mut demo = Demo {
        db,
        ids: BTreeMap::new(),
        summary: DemoSummary {
            countries: 0,
            territories: 0,
            ownership_changes: 0,
            periods: 0,
            events: 0,
            semantic_values: 0,
            geo_areas: 0,
        },
    };

    use EntityKind as K;
    for (k, kind, name) in [
        ("ruthenian", K::Culture, "Ruthenian"),
        ("eastern", K::Culture, "Eastern"),
        ("western", K::Culture, "Western"),
        ("old_faith", K::Religion, "Old Faith"),
        ("volodarids", K::Dynasty, "Volodarids"),
        ("ostrozhsky", K::Dynasty, "House Ostroh"),
        ("kesarids", K::Dynasty, "Kesarids"),
        ("ruthenia", K::Country, "Kingdom of Ruthenia"),
        ("empire", K::Country, "Empire of the East"),
        ("march", K::Country, "Duchy of the West March"),
        ("kyiv", K::Territory, "Kyiv"),
        ("halych", K::Territory, "Halych"),
        ("chernihiv", K::Territory, "Chernihiv"),
        ("volodymyr", K::Territory, "Volodymyr"),
        ("basileia", K::Territory, "Basileia"),
        ("westmark", K::Territory, "Westmark"),
    ] {
        demo.entity(k, kind, name)?;
    }

    // Country facts + name history
    let c = demo.db.conn();
    for (country, culture, capital, y) in [
        ("ruthenia", "ruthenian", "kyiv", 1021),
        ("empire", "eastern", "basileia", 867),
        ("march", "western", "westmark", 1180),
    ] {
        c.execute(
            "UPDATE countries SET primary_culture = ?1, religion = ?2, capital_entity_id = ?3, founded_year = ?4 WHERE entity_id = ?5",
            params![demo.ids[culture].as_str(), demo.ids["old_faith"].as_str(), demo.ids[capital].as_str(), y, demo.ids[country].as_str()],
        )?;
    }
    for (name, from, to) in [("Principality of Ruthenia", 1021, Some(1205)), ("Kingdom of Ruthenia", 1205, None)] {
        c.execute(
            "INSERT INTO country_names(country_id, name, start_year, end_year) VALUES (?1, ?2, ?3, ?4)",
            params![demo.ids["ruthenia"].as_str(), name, from, to],
        )?;
    }

    // Ownership timeline
    for (t, o, date) in [
        ("basileia", "empire", "867.1.1"),
        ("halych", "empire", "867.1.1"),
        ("kyiv", "empire", "867.1.1"),
        ("chernihiv", "empire", "867.1.1"),
        ("volodymyr", "empire", "867.1.1"),
        ("kyiv", "ruthenia", "1021.6.1"),
        ("chernihiv", "ruthenia", "1021.6.1"),
        ("volodymyr", "ruthenia", "1088"),
        ("westmark", "march", "1180"),
        ("halych", "ruthenia", "1199"),
        ("halych", "march", "1240.12.6"),
        ("halych", "ruthenia", "1302"),
    ] {
        demo.own(t, o, date)?;
    }

    // State periods
    demo.period("ruthenia", Aspect::Government, Some("principality"), None, "1021.6.1")?;
    demo.period("ruthenia", Aspect::Government, Some("feudal_monarchy"), None, "1205")?;
    demo.period("ruthenia", Aspect::Dynasty, None, Some("volodarids"), "1021.6.1")?;
    demo.period("ruthenia", Aspect::Capital, None, Some("kyiv"), "1021.6.1")?;
    demo.period("empire", Aspect::Government, Some("imperial_administration"), None, "867.1.1")?;
    demo.period("empire", Aspect::Dynasty, None, Some("kesarids"), "867.1.1")?;
    demo.period("empire", Aspect::Dynasty, None, Some("ostrozhsky"), "1185")?;
    demo.period("march", Aspect::Government, Some("feudal_monarchy"), None, "1180")?;
    demo.period("march", Aspect::Overlord, None, Some("empire"), "1180")?;

    // Journal
    demo.event("1021.6.1", "state_created", Some("ruthenia"), None, 5, json!({"name": "Principality of Ruthenia"}))?;
    for y in ["1092.4.12", "1123", "1146", "1171", "1199", "1255", "1290"] {
        demo.event(y, "civil_war", Some("empire"), None, 3, json!({}))?;
    }
    demo.event("1185", "dynasty_changed", Some("empire"), Some("ostrozhsky"), 4, json!({}))?;
    demo.event("1205", "government_changed", Some("ruthenia"), None, 4, json!({"to": "feudal_monarchy", "title": "Kingdom of Ruthenia"}))?;
    demo.event("1240.12.6", "war_ended", Some("march"), Some("ruthenia"), 4, json!({"result": "Halych lost"}))?;
    demo.event("1302", "war_ended", Some("ruthenia"), Some("march"), 4, json!({"result": "Halych restored"}))?;
    // Inferred: two snapshots disagree on the Empire's capital → moved sometime in 1200–1205.
    demo.event_full("1200", Some("1205"), EventOrigin::SnapshotDiff, "capital_changed", Some("empire"), Some("basileia"), 3,
                    json!({"from": "Old Basileia", "to": "Basileia"}))?;

    // Semantics with provenance — the "Why?" example from the spec, plus warning/review cases
    let ok = |found, expected, mapping| ConfidenceComponents::new(found, expected, mapping, true, true);
    demo.semantic("ruthenia", "1336", "centralization", 0.74,
        &[("crown_authority", 0.31), ("average_control", 0.24), ("vassal_autonomy", -0.12), ("government_structure", 0.31)],
        "ck3_centralization_v2", ok(4, 4, 0.91), &policy)?;
    demo.semantic("ruthenia", "1336", "dynastic_legitimacy", 0.82,
        &[("dynasty_age", 0.45), ("succession_stability", 0.37)], "ck3_legitimacy_v1", ok(2, 2, 0.88), &policy)?;
    demo.semantic("ruthenia", "1336", "militarization", 0.41,
        &[("levies", 0.22), ("men_at_arms", 0.19)], "ck3_militarization_v1", ok(2, 3, 0.95), &policy)?;
    demo.semantic("ruthenia", "1336", "merchant_power", 0.63,
        &[("trade_buildings", 0.40), ("city_holdings", 0.23)], "ck3_merchant_power_v0", ok(2, 4, 0.86), &policy)?;
    // Full data and a good formula, but validation failed → gate closed → review.
    demo.semantic("empire", "1336", "regional_autonomy", 0.71,
        &[("vassal_autonomy", 0.52), ("civil_wars", 0.19)], "ck3_autonomy_v1",
        ConfidenceComponents::new(2, 2, 0.90, true, false), &policy)?;

    // GeoCore mini dataset: Historical Territory ≠ Game Province.
    // Kyiv (territory) = one geo area; that area is split 70/30 between two demo CK3 provinces,
    // and maps entirely into one demo EU5 location.
    let c = demo.db.conn();
    c.execute(
        "INSERT INTO geo_datasets(id, description, base_game, base_game_version, status)
         VALUES ('chronicle_geo_demo', 'Developer demo dataset', 'ck3', 'demo', 'draft')",
        [],
    )?;
    for (game, prov, name) in [("ck3", "demo_b1", "Kyiv barony"), ("ck3", "demo_b2", "Vyshhorod barony"), ("eu5", "demo_l1", "Kyiv location")] {
        c.execute(
            "INSERT INTO game_provinces(game, game_version, province_id, name) VALUES (?1, 'demo', ?2, ?3)",
            params![game, prov, name],
        )?;
    }
    let area = demo.db.next_id(K::GeoArea)?;
    let c = demo.db.conn();
    c.execute(
        "INSERT INTO geo_areas(id, dataset_id, name, centroid_x, centroid_y) VALUES (?1, 'chronicle_geo_demo', 'Kyiv area', 30.5, 50.4)",
        [area.as_str()],
    )?;
    c.execute(
        "INSERT INTO territory_areas(territory_id, area_id, share) VALUES (?1, ?2, 1.0)",
        params![demo.ids["kyiv"].as_str(), area.as_str()],
    )?;
    for (game, prov, share_area, share_prov) in [("ck3", "demo_b1", 0.7, 1.0), ("ck3", "demo_b2", 0.3, 1.0), ("eu5", "demo_l1", 1.0, 0.8)] {
        c.execute(
            "INSERT INTO province_area_overlap(dataset_id, area_id, game, game_version, province_id, share_of_area,
                                               share_of_province, method)
             VALUES ('chronicle_geo_demo', ?1, ?2, 'demo', ?3, ?4, ?5, 'manual')",
            params![area.as_str(), game, prov, share_area, share_prov],
        )?;
    }
    demo.summary.geo_areas += 1;

    // Population samples
    let c = demo.db.conn();
    for (t, y, pop, culture) in [("kyiv", 1200, 52_000.0, "ruthenian"), ("kyiv", 1336, 61_000.0, "ruthenian"), ("halych", 1336, 23_000.0, "ruthenian")] {
        c.execute(
            "INSERT INTO population_snapshots(territory_id, year, population, culture_distribution) VALUES (?1, ?2, ?3, ?4)",
            params![demo.ids[t].as_str(), y, pop, {
                let mut dist = serde_json::Map::new();
                dist.insert(demo.ids[culture].to_string(), json!(1.0));
                Value::Object(dist).to_string()
            }],
        )?;
    }

    Ok(demo.summary)
}

// ======================================================================================

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

fn count(conn: &Connection, sql: &str) -> rusqlite::Result<i64> {
    conn.query_row(sql, [], |r| r.get(0))
}

/// Database invariants. Every check is cheap; run it after any import or in developer mode.
pub fn self_test(db: &CampaignDb) -> Result<Vec<Check>> {
    let c = db.conn();
    let mut out = Vec::new();
    let mut push = |name: &'static str, ok: bool, detail: String| out.push(Check { name, ok, detail });

    let integrity: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    push("sqlite_integrity", integrity == "ok", integrity);

    let fk = count(c, "SELECT COUNT(*) FROM pragma_foreign_key_check")?;
    push("foreign_keys", fk == 0, format!("{fk} broken references"));

    let latest = latest(CAMPAIGN_MIGRATIONS);
    let v = current_version(c)?;
    push("schema_version", v == latest, format!("database v{v}, latest v{latest}"));

    let overlaps = count(c,
        "SELECT COUNT(*) FROM territory_ownership a JOIN territory_ownership b
           ON a.territory_id = b.territory_id AND a.id < b.id
          AND a.from_key < COALESCE(b.to_key, 9223372036854775807)
          AND b.from_key < COALESCE(a.to_key, 9223372036854775807)")?;
    push("ownership_no_overlap", overlaps == 0, format!("{overlaps} overlapping ownership periods"));

    let p_overlaps = count(c,
        "SELECT COUNT(*) FROM entity_periods a JOIN entity_periods b
           ON a.entity_id = b.entity_id AND a.aspect = b.aspect AND a.id < b.id
          AND a.from_key < COALESCE(b.to_key, 9223372036854775807)
          AND b.from_key < COALESCE(a.to_key, 9223372036854775807)")?;
    push("periods_no_overlap", p_overlaps == 0, format!("{p_overlaps} overlapping state periods"));

    let stale_owner = count(c,
        "SELECT COUNT(*) FROM territories t JOIN territory_ownership o
           ON o.territory_id = t.entity_id AND o.to_key IS NULL
         WHERE COALESCE(t.current_owner, '') <> COALESCE(o.owner_id, '')")?;
    push("current_owner_matches_history", stale_owner == 0, format!("{stale_owner} territories out of sync"));

    let wrong_kind = count(c,
        "SELECT COUNT(*) FROM territory_ownership o JOIN entities e ON e.id = o.territory_id WHERE e.kind <> 'territory'")?;
    push("ownership_subjects_are_territories", wrong_kind == 0, format!("{wrong_kind} rows"));

    let bad_dates = count(c,
        "SELECT COUNT(*) FROM events WHERE (month IS NOT NULL AND month NOT BETWEEN 1 AND 12)
            OR (day IS NOT NULL AND (day NOT BETWEEN 1 AND 31 OR month IS NULL))")?;
    push("event_dates_valid", bad_dates == 0, format!("{bad_dates} events with impossible dates"));

    let policy = db.summary()?.settings.confidence;
    let unreviewed: i64 = c.query_row(
        "SELECT COUNT(*) FROM semantic_values WHERE confidence < ?1 AND review_state IN ('auto', 'warning')",
        [policy.warn_min],
        |r| r.get(0),
    )?;
    push("low_confidence_needs_review", unreviewed == 0,
         format!("{unreviewed} values below {:.0}% applied without review", policy.warn_min * 100.0));

    let gated = count(c,
        "SELECT COUNT(*) FROM semantic_values s JOIN provenance p ON p.id = s.provenance_id
         WHERE (p.version_supported = 0 OR p.validation_passed = 0) AND s.review_state IN ('auto', 'warning')")?;
    push("closed_gates_need_review", gated == 0,
         format!("{gated} values applied although version support or validation failed"));

    let overfull = count(c,
        "SELECT COUNT(*) FROM (SELECT SUM(share_of_area) AS s FROM province_area_overlap
                              GROUP BY dataset_id, area_id, game, game_version HAVING s > 1.000001)")?;
    push("geo_area_shares_sum_to_at_most_1", overfull == 0, format!("{overfull} areas over-allocated"));

    let orphans = count(c, "SELECT COUNT(*) FROM semantic_values WHERE provenance_id IS NULL")?;
    push("semantic_values_have_provenance", orphans == 0, format!("{orphans} values without a 'Why?'"));

    Ok(out)
}

// ======================================================================================

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub truncated: bool,
}

/// Run one SELECT on a separate read-only connection. Writes are impossible twice over:
/// the connection is opened read-only and the statement must report itself read-only.
pub fn readonly_query(db: &CampaignDb, sql: &str, limit: usize) -> Result<QueryResult> {
    let conn = Connection::open_with_flags(
        CampaignDb::db_path(db.root()),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut st = conn.prepare(sql)?;
    if !st.readonly() {
        return Err(DbError::NotReadOnly);
    }
    let columns: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    let n = columns.len();
    let limit = limit.clamp(1, 1000);
    let mut rows = st.query([])?;
    let mut out = Vec::new();
    let mut truncated = false;
    while let Some(r) = rows.next()? {
        if out.len() == limit {
            truncated = true;
            break;
        }
        let mut row = Vec::with_capacity(n);
        for i in 0..n {
            row.push(match r.get_ref(i)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(v) => json!(v),
                ValueRef::Real(v) => json!(v),
                ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
                ValueRef::Blob(b) => Value::String(format!("<blob {} bytes>", b.len())),
            });
        }
        out.push(row);
    }
    Ok(QueryResult { columns, rows: out, truncated })
}

pub fn table_counts(db: &CampaignDb) -> Result<Vec<(String, i64)>> {
    let c = db.conn();
    let mut st = c.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
    let names: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    names
        .into_iter()
        .map(|n| -> Result<(String, i64)> {
            let k = count(c, &format!("SELECT COUNT(*) FROM \"{}\"", n.replace('"', "\"\"")))?;
            Ok((n, k))
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityLabel {
    pub id: String,
    pub kind: String,
    pub name: Option<String>,
}

/// Entities with a display name, for developer pickers.
pub fn entity_labels(db: &CampaignDb, kind: Option<&str>) -> Result<Vec<EntityLabel>> {
    let mut st = db.conn().prepare(
        "SELECT e.id, e.kind, COALESCE(c.current_name, t.name, d.name, cu.name, r.name)
         FROM entities e
         LEFT JOIN countries c ON c.entity_id = e.id
         LEFT JOIN territories t ON t.entity_id = e.id
         LEFT JOIN dynasties d ON d.entity_id = e.id
         LEFT JOIN cultures cu ON cu.entity_id = e.id
         LEFT JOIN religions r ON r.entity_id = e.id
         WHERE ?1 IS NULL OR e.kind = ?1
         ORDER BY e.id",
    )?;
    let rows = st.query_map([kind], |r| Ok(EntityLabel { id: r.get(0)?, kind: r.get(1)?, name: r.get(2)? }))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CampaignSettings;
    use chronicle_core::registry::GameRegistry;

    fn demo_campaign() -> (tempfile::TempDir, CampaignDb, DemoSummary) {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = CampaignDb::create(
            &tmp.path().join("demo"),
            "Demo",
            &CampaignSettings::new("ck3", 7),
            &GameRegistry::builtin().unwrap(),
        )
        .unwrap();
        let s = seed_demo(&mut db).unwrap();
        (tmp, db, s)
    }

    #[test]
    fn demo_world_passes_self_test() {
        let (_tmp, db, s) = demo_campaign();
        assert_eq!((s.countries, s.territories, s.geo_areas), (3, 6, 1));
        let checks = self_test(&db).unwrap();
        let failed: Vec<_> = checks.iter().filter(|c| !c.ok).collect();
        assert!(failed.is_empty(), "{failed:#?}");
        assert!(db.summary().unwrap().is_demo);
    }

    #[test]
    fn demo_answers_time_questions() {
        let (_tmp, db, _) = demo_campaign();
        let halych = entity_labels(&db, Some("territory")).unwrap().into_iter().find(|e| e.name.as_deref() == Some("Halych")).unwrap();
        let owner_1250 = db.owner_at(&halych.id, "1250".parse().unwrap()).unwrap().unwrap().owner_id.unwrap();
        let march = entity_labels(&db, Some("country")).unwrap().into_iter().find(|e| e.name.as_deref() == Some("Duchy of the West March")).unwrap();
        assert_eq!(owner_1250, march.id);
        assert_eq!(db.ownership_history(&halych.id).unwrap().len(), 4);
    }

    #[test]
    fn low_confidence_value_goes_to_review() {
        let (_tmp, db, _) = demo_campaign();
        let q = readonly_query(&db, "SELECT semantic_key, review_state FROM semantic_values ORDER BY semantic_key", 50).unwrap();
        let states: BTreeMap<String, String> = q
            .rows
            .iter()
            .map(|r| (r[0].as_str().unwrap().to_string(), r[1].as_str().unwrap().to_string()))
            .collect();
        assert_eq!(states["centralization"], "auto"); // 4/4 × 0.91
        assert_eq!(states["militarization"], "warning"); // 2/3 × 0.95 ≈ 0.63
        assert_eq!(states["merchant_power"], "pending_review"); // 2/4 × 0.86 = 0.43
        assert_eq!(states["regional_autonomy"], "pending_review"); // validation gate closed
    }

    #[test]
    fn demo_journal_marks_inferred_events() {
        let (_tmp, db, _) = demo_campaign();
        let ev = db.events(0, 1000).unwrap();
        let cap = ev.iter().find(|e| e.event_type == "capital_changed").unwrap();
        assert_eq!((cap.date_precision.as_str(), cap.evidence.as_str(), cap.origin.as_str()), ("range", "inferred", "snapshot_diff"));
        assert!(ev.iter().filter(|e| e.event_type == "civil_war").all(|e| e.evidence == "observed"));
    }

    #[test]
    fn console_is_read_only_and_seed_runs_once() {
        let (_tmp, mut db, _) = demo_campaign();
        assert!(matches!(readonly_query(&db, "DELETE FROM events", 10), Err(DbError::NotReadOnly)));
        assert!(readonly_query(&db, "DROP TABLE events", 10).is_err());
        let q = readonly_query(&db, "SELECT * FROM events", 2).unwrap();
        assert!(q.truncated && q.rows.len() == 2);
        assert!(matches!(seed_demo(&mut db), Err(DbError::NotDemoSafe)));
        assert!(table_counts(&db).unwrap().iter().any(|(t, n)| t == "territory_ownership" && *n > 0));
    }

    #[test]
    fn refuses_to_seed_real_campaign() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = CampaignDb::create(&tmp.path().join("real"), "Real", &CampaignSettings::new("ck3", 7),
                                        &GameRegistry::builtin().unwrap()).unwrap();
        db.next_id(EntityKind::Country).unwrap();
        assert!(matches!(seed_demo(&mut db), Err(DbError::NotDemoSafe)));
    }
}
