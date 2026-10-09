-- campaign.db — full history of a mega campaign. Migration 0001.
-- Full UWM snapshots live in snapshots/<id>.json.zst; these tables are indexed
-- projections for queries (GUI, rules, timeline) plus the complete audit trail.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS campaigns (
    id                 INTEGER PRIMARY KEY,
    name               TEXT NOT NULL,
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    seed               INTEGER NOT NULL,
    config_yaml        TEXT NOT NULL,          -- campaign.yaml at creation
    converter_version  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS snapshots (
    id                 INTEGER PRIMARY KEY,
    campaign_id        INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    game               TEXT NOT NULL,          -- imperator | ck3 | eu4 | eu5 | victoria3 | hoi4 | stellaris
    game_version       TEXT,
    kind               TEXT NOT NULL CHECK (kind IN ('import', 'converted', 'manual')),
    date_year          INTEGER, date_month INTEGER, date_day INTEGER,   -- negative year = BC
    source_file        TEXT,                   -- path inside input/
    source_sha256      TEXT,
    world_path         TEXT NOT NULL,          -- snapshots/<id>.json.zst
    world_sha256       TEXT NOT NULL,
    uwm_schema_version INTEGER NOT NULL,
    parent_snapshot_id INTEGER REFERENCES snapshots(id),
    created_at         TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS ix_snapshots_campaign ON snapshots(campaign_id, date_year);

-- Stable campaign-wide identities and their native ids in every snapshot.
CREATE TABLE IF NOT EXISTS entities (
    uid         TEXT PRIMARY KEY,              -- e.g. country:000042
    kind        TEXT NOT NULL,                 -- country | territory | culture | ...
    campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    first_seen_snapshot_id INTEGER REFERENCES snapshots(id)
);
CREATE TABLE IF NOT EXISTS entity_lineage (
    uid         TEXT NOT NULL REFERENCES entities(uid) ON DELETE CASCADE,
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    game        TEXT NOT NULL,
    native_id   TEXT NOT NULL,
    PRIMARY KEY (uid, snapshot_id, native_id)
);
CREATE INDEX IF NOT EXISTS ix_lineage_native ON entity_lineage(snapshot_id, native_id);

-- Indexed projections: hot columns typed, the rest in attrs (JSON).
CREATE TABLE IF NOT EXISTS countries (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    uid TEXT NOT NULL, name TEXT NOT NULL,
    capital_uid TEXT, primary_culture_uid TEXT, religion_uid TEXT,
    overlord_uid TEXT, ruling_dynasty_uid TEXT, government_type TEXT,
    stability REAL, legitimacy REAL, centralization REAL, prestige REAL,
    population REAL, economic_power REAL, military_power REAL,
    attrs TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (snapshot_id, uid)
);
CREATE TABLE IF NOT EXISTS territories (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    uid TEXT NOT NULL, name TEXT NOT NULL,
    owner_uid TEXT, controller_uid TEXT, region_uid TEXT,
    population REAL, development REAL, urbanization REAL, economic_output REAL,
    attrs TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (snapshot_id, uid)
);
CREATE INDEX IF NOT EXISTS ix_territories_owner ON territories(snapshot_id, owner_uid);

CREATE TABLE IF NOT EXISTS cultures (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    uid TEXT NOT NULL, name TEXT NOT NULL, group_name TEXT, parent_uid TEXT,
    attrs TEXT NOT NULL DEFAULT '{}', PRIMARY KEY (snapshot_id, uid)
);
CREATE TABLE IF NOT EXISTS religions (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    uid TEXT NOT NULL, name TEXT NOT NULL, family TEXT, parent_uid TEXT,
    attrs TEXT NOT NULL DEFAULT '{}', PRIMARY KEY (snapshot_id, uid)
);
CREATE TABLE IF NOT EXISTS populations (            -- per territory, per culture/religion/class slice
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    territory_uid TEXT NOT NULL,
    culture_uid TEXT, religion_uid TEXT, class_uid TEXT,
    size REAL NOT NULL CHECK (size >= 0),
    literacy REAL, wealth REAL, radicalism REAL
);
CREATE INDEX IF NOT EXISTS ix_populations_territory ON populations(snapshot_id, territory_uid);

CREATE TABLE IF NOT EXISTS dynasties (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    uid TEXT NOT NULL, name TEXT NOT NULL, founded_year INTEGER, culture_uid TEXT,
    attrs TEXT NOT NULL DEFAULT '{}', PRIMARY KEY (snapshot_id, uid)
);
CREATE TABLE IF NOT EXISTS characters (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    uid TEXT NOT NULL, name TEXT NOT NULL, dynasty_uid TEXT,
    birth_year INTEGER, death_year INTEGER,
    attrs TEXT NOT NULL DEFAULT '{}', PRIMARY KEY (snapshot_id, uid)
);
CREATE TABLE IF NOT EXISTS wars (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    uid TEXT NOT NULL, name TEXT, start_year INTEGER, end_year INTEGER,
    is_civil_war INTEGER NOT NULL DEFAULT 0,
    attrs TEXT NOT NULL DEFAULT '{}', PRIMARY KEY (snapshot_id, uid)
);
CREATE TABLE IF NOT EXISTS governments (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    country_uid TEXT NOT NULL, government_type TEXT, attrs TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (snapshot_id, country_uid)
);
CREATE TABLE IF NOT EXISTS economies (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    country_uid TEXT NOT NULL, estimated_output REAL, attrs TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (snapshot_id, country_uid)
);
CREATE TABLE IF NOT EXISTS technologies (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    country_uid TEXT NOT NULL, attrs TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (snapshot_id, country_uid)
);
CREATE TABLE IF NOT EXISTS relationships (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    a_uid TEXT NOT NULL, b_uid TEXT NOT NULL,
    kind TEXT NOT NULL,                    -- alliance | rivalry | subject:<kind> | ...
    value REAL, since_year INTEGER,
    PRIMARY KEY (snapshot_id, a_uid, b_uid, kind)
);
CREATE TABLE IF NOT EXISTS modifiers (
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    entity_uid TEXT NOT NULL, key TEXT NOT NULL,
    values_json TEXT NOT NULL, source TEXT,
    PRIMARY KEY (snapshot_id, entity_uid, key)
);

-- Append-only historical timeline (survives every conversion).
CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY,
    campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    snapshot_id INTEGER REFERENCES snapshots(id),
    year INTEGER NOT NULL, month INTEGER NOT NULL DEFAULT 1, day INTEGER NOT NULL DEFAULT 1,
    kind        TEXT NOT NULL,             -- state_created | civil_war | capital_moved | ...
    actors      TEXT NOT NULL DEFAULT '[]',-- JSON array of uids
    magnitude   REAL,
    payload     TEXT NOT NULL DEFAULT '{}',
    origin      TEXT NOT NULL CHECK (origin IN ('save', 'diff', 'converter', 'manual'))
);
CREATE INDEX IF NOT EXISTS ix_events_time ON events(campaign_id, year, month, day);
CREATE INDEX IF NOT EXISTS ix_events_kind ON events(campaign_id, kind);

-- Audit trail
CREATE TABLE IF NOT EXISTS conversions (
    id INTEGER PRIMARY KEY,
    campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    from_snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
    to_snapshot_id INTEGER REFERENCES snapshots(id),
    from_game TEXT NOT NULL, to_game TEXT NOT NULL,
    seed INTEGER NOT NULL, converter_version TEXT NOT NULL,
    rules_sha256 TEXT NOT NULL, formulas_sha256 TEXT NOT NULL, overrides_sha256 TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('preview', 'generated', 'failed')),
    output_path TEXT, report_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS rule_applications (
    conversion_id INTEGER NOT NULL REFERENCES conversions(id) ON DELETE CASCADE,
    rule TEXT NOT NULL, entity_uid TEXT NOT NULL, phase TEXT NOT NULL,
    matched INTEGER NOT NULL, effects_json TEXT NOT NULL DEFAULT '{}',
    missing_metrics TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS ix_rule_apps ON rule_applications(conversion_id, entity_uid);

CREATE TABLE IF NOT EXISTS manual_overrides (
    id INTEGER PRIMARY KEY,
    campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    entity_uid TEXT NOT NULL, field TEXT NOT NULL, value_json TEXT NOT NULL,
    note TEXT, created_at TEXT NOT NULL DEFAULT (datetime('now')),
    removed_at TEXT
);

CREATE TABLE IF NOT EXISTS ai_recommendations (
    id INTEGER PRIMARY KEY,
    campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    entity_uid TEXT NOT NULL,
    task TEXT NOT NULL,                    -- recommend_government | successor_states | flavour_text ...
    input_sha256 TEXT NOT NULL,            -- cache key ⇒ reproducibility
    model TEXT NOT NULL,
    output_json TEXT NOT NULL,
    confidence REAL,
    accepted INTEGER,                      -- NULL = pending, 0/1 = decision
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (campaign_id, task, input_sha256)
);

INSERT OR IGNORE INTO schema_version(version) VALUES (1);
