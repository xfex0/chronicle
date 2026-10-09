-- Migration 0003 (runs with foreign keys temporarily off: the events table is rebuilt).
--
-- 1. Honest journal: every event says how it is known.
--    evidence  observed | inferred      origin  game | save | snapshot_diff | converter | user
--    date_precision day | month | year | range | unknown   (range ⇒ date_to_* is set)
-- 2. Composed confidence components on provenance.
-- 3. GeoCore: versioned datasets; Historical Territory ≠ Game Province.
-- 4. Backup register (retention policy).

CREATE TABLE events_v3 (
    event_id     INTEGER PRIMARY KEY,
    campaign_id  TEXT NOT NULL REFERENCES campaigns(id),
    year INTEGER NOT NULL, month INTEGER, day INTEGER,
    date_key     INTEGER NOT NULL,
    date_to_year INTEGER, date_to_month INTEGER, date_to_day INTEGER,
    date_precision TEXT NOT NULL CHECK (date_precision IN ('day', 'month', 'year', 'range', 'unknown')),
    game         TEXT NOT NULL,
    game_version TEXT,
    event_type   TEXT NOT NULL,
    actor_entity_id  TEXT REFERENCES entities(id),
    target_entity_id TEXT REFERENCES entities(id),
    payload      TEXT NOT NULL DEFAULT '{}',
    importance   INTEGER NOT NULL DEFAULT 1 CHECK (importance BETWEEN 0 AND 5),
    origin       TEXT NOT NULL CHECK (origin IN ('game', 'save', 'snapshot_diff', 'converter', 'user')),
    evidence     TEXT NOT NULL CHECK (evidence IN ('observed', 'inferred')),
    summarized_into INTEGER REFERENCES events(event_id),
    CHECK ((date_precision = 'range') = (date_to_year IS NOT NULL))
);
INSERT INTO events_v3
SELECT event_id, campaign_id, year, month, day,
       year * 10000 + COALESCE(month, 0) * 100 + COALESCE(day, 0),
       NULL, NULL, NULL,
       CASE
           WHEN date_precision = 'between_snapshots' THEN 'unknown'
           WHEN date_precision = 'year' OR month IS NULL THEN 'year'
           WHEN day IS NULL THEN 'month'
           ELSE 'day'
       END,
       game, game_version, event_type, actor_entity_id, target_entity_id, payload, importance,
       CASE source WHEN 'chronicle' THEN 'converter' ELSE source END,
       CASE source WHEN 'snapshot_diff' THEN 'inferred' ELSE 'observed' END,
       summarized_into
FROM events;
DROP TABLE events;
ALTER TABLE events_v3 RENAME TO events;
CREATE INDEX ix_events_time ON events(campaign_id, date_key);
CREATE INDEX ix_events_type ON events(event_type);
CREATE INDEX ix_events_actor ON events(actor_entity_id);

ALTER TABLE provenance ADD COLUMN data_coverage REAL;
ALTER TABLE provenance ADD COLUMN mapping_reliability REAL;
ALTER TABLE provenance ADD COLUMN version_supported INTEGER;
ALTER TABLE provenance ADD COLUMN validation_passed INTEGER;

-- GeoCore -----------------------------------------------------------------------------------
-- A dataset is one version of Chronicle's own geography (e.g. chronicle_geo_v1). A new
-- dataset never rewrites history: territories link to areas of a specific dataset.
CREATE TABLE geo_datasets (
    id                TEXT PRIMARY KEY,
    description       TEXT NOT NULL,
    base_game         TEXT,
    base_game_version TEXT,
    status            TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'retired')),
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE geo_areas (
    id         TEXT PRIMARY KEY REFERENCES entities(id),     -- chronicle_geo_area_000001
    dataset_id TEXT NOT NULL REFERENCES geo_datasets(id),
    name       TEXT,
    centroid_x REAL, centroid_y REAL,                        -- dataset coordinate system
    area       REAL CHECK (area IS NULL OR area >= 0),
    is_water   INTEGER NOT NULL DEFAULT 0
);
-- Historical Territory (a place in the campaign's history) is built from Geo Areas …
CREATE TABLE territory_areas (
    territory_id TEXT NOT NULL REFERENCES entities(id),
    area_id      TEXT NOT NULL REFERENCES geo_areas(id),
    share        REAL NOT NULL DEFAULT 1 CHECK (share > 0 AND share <= 1),
    PRIMARY KEY (territory_id, area_id)
);
-- … while a Game Province is one game version's map unit.
CREATE TABLE game_provinces (
    game         TEXT NOT NULL,
    game_version TEXT NOT NULL,
    province_id  TEXT NOT NULL,
    name         TEXT,
    kind         TEXT,                                       -- barony, county, location, state, ...
    PRIMARY KEY (game, game_version, province_id)
);
CREATE TABLE province_area_overlap (
    dataset_id   TEXT NOT NULL REFERENCES geo_datasets(id),
    area_id      TEXT NOT NULL REFERENCES geo_areas(id),
    game         TEXT NOT NULL,
    game_version TEXT NOT NULL,
    province_id  TEXT NOT NULL,
    share_of_area     REAL NOT NULL CHECK (share_of_area > 0 AND share_of_area <= 1),
    share_of_province REAL NOT NULL CHECK (share_of_province > 0 AND share_of_province <= 1),
    method       TEXT NOT NULL CHECK (method IN ('raster', 'control_points', 'manual')),
    PRIMARY KEY (dataset_id, area_id, game, game_version, province_id),
    FOREIGN KEY (game, game_version, province_id) REFERENCES game_provinces(game, game_version, province_id)
);
CREATE TABLE geo_control_points (
    id           INTEGER PRIMARY KEY,
    dataset_id   TEXT NOT NULL REFERENCES geo_datasets(id),
    game         TEXT NOT NULL,
    game_version TEXT NOT NULL,
    name         TEXT NOT NULL,                              -- "mouth of the Thames"
    geo_x REAL NOT NULL, geo_y REAL NOT NULL,                -- dataset coordinates
    map_x REAL NOT NULL, map_y REAL NOT NULL                 -- pixel on the game's map
);

-- Backups ------------------------------------------------------------------------------------
-- Retention: the newest 10 'auto' backups are kept; 'transition', 'migration' and 'manual'
-- backups are never deleted automatically.
CREATE TABLE backups (
    id         INTEGER PRIMARY KEY,
    path       TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('auto', 'transition', 'migration', 'manual')),
    label      TEXT NOT NULL,
    integrity  TEXT,                                         -- PRAGMA integrity_check result
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
