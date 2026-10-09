-- Campaign database: the single source of historical truth for one Mega Campaign.
-- Three kinds of data: WORLD STATE (current), HISTORY (events + snapshots), SEMANTICS.
-- Nothing from an earlier era is deleted because a later game lacks the mechanic.


CREATE TABLE IF NOT EXISTS campaigns (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    current_game  TEXT NOT NULL,
    current_year  INTEGER, current_month INTEGER, current_day INTEGER,
    seed          INTEGER NOT NULL,
    status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'transitioning', 'finished', 'archived')),
    settings_json TEXT NOT NULL,
    chronicle_version TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS games (
    game_id         TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    version         TEXT,
    adapter_version TEXT
);

CREATE TABLE IF NOT EXISTS save_files (
    id            INTEGER PRIMARY KEY,
    game_id       TEXT NOT NULL,
    original_path TEXT NOT NULL,
    stored_path   TEXT NOT NULL,          -- copy inside original/ (read-only)
    sha256        TEXT NOT NULL UNIQUE,
    in_game_year  INTEGER, in_game_month INTEGER, in_game_day INTEGER,
    game_version  TEXT,
    imported_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Stable Chronicle ids. Numbers per kind come from id_counters.
CREATE TABLE IF NOT EXISTS id_counters (kind TEXT PRIMARY KEY, next INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS entities (
    id         TEXT PRIMARY KEY,          -- chronicle_country_000001
    kind       TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS game_entity_mappings (
    entity_id      TEXT NOT NULL REFERENCES entities(id),
    game_id        TEXT NOT NULL,
    game_entity_id TEXT NOT NULL,         -- e.g. CK3 title key; never used as a universal id
    valid_from_year INTEGER, valid_to_year INTEGER,
    PRIMARY KEY (entity_id, game_id, game_entity_id)
);
CREATE INDEX IF NOT EXISTS ix_gem_native ON game_entity_mappings(game_id, game_entity_id);

CREATE TABLE IF NOT EXISTS entity_lineage (
    parent_id  TEXT NOT NULL REFERENCES entities(id),
    child_id   TEXT NOT NULL REFERENCES entities(id),
    relation   TEXT NOT NULL CHECK (relation IN ('renamed', 'successor', 'split', 'merged', 'annexed', 'federated', 'restored')),
    year INTEGER, month INTEGER, day INTEGER,
    weight     REAL,                      -- share of heritage passed on (split/merge)
    event_id   INTEGER REFERENCES events(event_id),
    PRIMARY KEY (parent_id, child_id, relation)
);

-- World state
CREATE TABLE IF NOT EXISTS countries (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id),
    current_name TEXT NOT NULL,
    founded_year INTEGER, founded_month INTEGER, founded_day INTEGER,
    destroyed_year INTEGER, destroyed_month INTEGER, destroyed_day INTEGER,
    primary_culture TEXT REFERENCES entities(id),
    religion TEXT REFERENCES entities(id),
    capital_entity_id TEXT REFERENCES entities(id)
);
CREATE TABLE IF NOT EXISTS country_names (
    country_id TEXT NOT NULL REFERENCES entities(id),
    name TEXT NOT NULL,
    start_year INTEGER, end_year INTEGER
);
CREATE TABLE IF NOT EXISTS territories (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id),
    geographic_id TEXT REFERENCES entities(id),
    current_owner TEXT REFERENCES entities(id),
    current_controller TEXT REFERENCES entities(id)
);
CREATE TABLE IF NOT EXISTS characters (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id),
    name TEXT NOT NULL,
    birth_year INTEGER, birth_month INTEGER, birth_day INTEGER,
    death_year INTEGER, death_month INTEGER, death_day INTEGER,
    dynasty_id TEXT REFERENCES entities(id),
    culture TEXT REFERENCES entities(id),
    religion TEXT REFERENCES entities(id)
);
CREATE TABLE IF NOT EXISTS dynasties (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id),
    name TEXT NOT NULL,
    founded_year INTEGER, extinct_year INTEGER
);
CREATE TABLE IF NOT EXISTS cultures (entity_id TEXT PRIMARY KEY REFERENCES entities(id), name TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS religions (entity_id TEXT PRIMARY KEY REFERENCES entities(id), name TEXT NOT NULL);

-- Time series (values = JSON object of semantic or game-specific numbers)
CREATE TABLE IF NOT EXISTS population_snapshots (
    territory_id TEXT NOT NULL REFERENCES entities(id),
    year INTEGER NOT NULL, month INTEGER, day INTEGER,
    population REAL NOT NULL CHECK (population >= 0),
    culture_distribution TEXT NOT NULL DEFAULT '{}',
    religion_distribution TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS economy_snapshots    (entity_id TEXT NOT NULL REFERENCES entities(id), year INTEGER NOT NULL, month INTEGER, day INTEGER, "values" TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS government_snapshots (entity_id TEXT NOT NULL REFERENCES entities(id), year INTEGER NOT NULL, month INTEGER, day INTEGER, "values" TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS military_snapshots   (entity_id TEXT NOT NULL REFERENCES entities(id), year INTEGER NOT NULL, month INTEGER, day INTEGER, "values" TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS technology_snapshots (entity_id TEXT NOT NULL REFERENCES entities(id), year INTEGER NOT NULL, month INTEGER, day INTEGER, "values" TEXT NOT NULL);

-- History
CREATE TABLE IF NOT EXISTS events (
    event_id     INTEGER PRIMARY KEY,
    campaign_id  TEXT NOT NULL REFERENCES campaigns(id),
    year INTEGER NOT NULL, month INTEGER, day INTEGER,      -- partial dates allowed
    date_precision TEXT NOT NULL DEFAULT 'exact' CHECK (date_precision IN ('exact', 'between_snapshots', 'year')),
    game         TEXT NOT NULL,
    game_version TEXT,
    event_type   TEXT NOT NULL,
    actor_entity_id  TEXT REFERENCES entities(id),
    target_entity_id TEXT REFERENCES entities(id),
    payload      TEXT NOT NULL DEFAULT '{}',
    importance   INTEGER NOT NULL DEFAULT 1 CHECK (importance BETWEEN 0 AND 5),
    source       TEXT NOT NULL CHECK (source IN ('save', 'snapshot_diff', 'chronicle', 'user')),
    summarized_into INTEGER REFERENCES events(event_id)    -- future History Optimizer; nothing is deleted
);
CREATE INDEX IF NOT EXISTS ix_events_time ON events(campaign_id, year, month, day);
CREATE INDEX IF NOT EXISTS ix_events_type ON events(event_type);

CREATE TABLE IF NOT EXISTS snapshots (
    snapshot_id INTEGER PRIMARY KEY,
    game     TEXT NOT NULL,
    year INTEGER NOT NULL, month INTEGER, day INTEGER,
    type     TEXT NOT NULL CHECK (type IN ('initial', 'periodic', 'checkpoint', 'final', 'projection')),
    path     TEXT NOT NULL,
    checksum TEXT NOT NULL,
    save_file_id INTEGER REFERENCES save_files(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Semantics
CREATE TABLE IF NOT EXISTS provenance (
    id             INTEGER PRIMARY KEY,
    source_game    TEXT NOT NULL,
    source_fields  TEXT NOT NULL,          -- JSON array
    contributions  TEXT NOT NULL DEFAULT '{}', -- JSON {field: signed contribution} for the "Why?" view
    formula_id     TEXT,
    confidence     REAL NOT NULL CHECK (confidence BETWEEN 0 AND 1),
    adapter_version TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS semantic_values (
    entity_id     TEXT NOT NULL REFERENCES entities(id),
    year INTEGER NOT NULL, month INTEGER, day INTEGER,
    semantic_key  TEXT NOT NULL,           -- from the Semantic Registry
    value         REAL NOT NULL,
    confidence    REAL NOT NULL CHECK (confidence BETWEEN 0 AND 1),
    provenance_id INTEGER REFERENCES provenance(id),
    review_state  TEXT NOT NULL DEFAULT 'auto' CHECK (review_state IN ('auto', 'warning', 'pending_review', 'accepted', 'edited', 'ignored'))
);
CREATE INDEX IF NOT EXISTS ix_semantic ON semantic_values(entity_id, semantic_key, year);

-- Conversions
CREATE TABLE IF NOT EXISTS conversions (
    conversion_id INTEGER PRIMARY KEY,
    source_game TEXT NOT NULL, target_game TEXT NOT NULL,
    source_snapshot INTEGER NOT NULL REFERENCES snapshots(snapshot_id),
    source_checksum TEXT NOT NULL,
    year INTEGER NOT NULL, month INTEGER, day INTEGER,
    converter_version TEXT NOT NULL,
    adapter_versions TEXT NOT NULL,        -- JSON
    rules_version TEXT NOT NULL,
    coefficient_profile TEXT NOT NULL,
    seed INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('preview', 'generated', 'failed')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS conversion_warnings (
    conversion_id INTEGER NOT NULL REFERENCES conversions(conversion_id),
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'error', 'critical')),
    entity_id TEXT,
    message TEXT NOT NULL
);

-- Manual overrides change the PROJECTION into a target game, never historical facts.
CREATE TABLE IF NOT EXISTS manual_overrides (
    id INTEGER PRIMARY KEY,
    entity_id TEXT NOT NULL REFERENCES entities(id),
    target_game TEXT NOT NULL,
    parameter TEXT NOT NULL,
    original_value TEXT,
    override_value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (entity_id, target_game, parameter)
);

