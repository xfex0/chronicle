-- Migration 0002: time periods, so "who owned X in year Y?" and "how long has dynasty Z ruled?"
-- are direct queries instead of snapshot scans.
--
-- Dates: *_key = year*10000 + month*100 + day (unknown parts = 0), see PartialDate::sort_key.
-- A period is [from, to). to_key NULL = still current. Periods of one subject never overlap,
-- and new periods can only start at or after the current one (history moves forward).

CREATE TABLE IF NOT EXISTS territory_ownership (
    id           INTEGER PRIMARY KEY,
    territory_id TEXT NOT NULL REFERENCES entities(id),
    owner_id     TEXT REFERENCES entities(id),        -- NULL = unowned / wasteland
    controller_id TEXT REFERENCES entities(id),       -- NULL = same as owner
    from_year INTEGER NOT NULL, from_month INTEGER, from_day INTEGER, from_key INTEGER NOT NULL,
    to_year   INTEGER,          to_month   INTEGER, to_day   INTEGER, to_key   INTEGER,
    game         TEXT NOT NULL,
    source       TEXT NOT NULL CHECK (source IN ('save', 'snapshot_diff', 'chronicle', 'user', 'demo')),
    event_id     INTEGER REFERENCES events(event_id),
    CHECK (to_key IS NULL OR to_key >= from_key)
);
CREATE INDEX IF NOT EXISTS ix_ownership_territory ON territory_ownership(territory_id, from_key);
CREATE INDEX IF NOT EXISTS ix_ownership_owner ON territory_ownership(owner_id, from_key);
CREATE UNIQUE INDEX IF NOT EXISTS ux_ownership_open ON territory_ownership(territory_id) WHERE to_key IS NULL;

-- Generic state periods of an entity: government form, ruler, ruling dynasty, capital, ...
CREATE TABLE IF NOT EXISTS entity_periods (
    id              INTEGER PRIMARY KEY,
    entity_id       TEXT NOT NULL REFERENCES entities(id),
    aspect          TEXT NOT NULL CHECK (aspect IN
                        ('government', 'ruler', 'dynasty', 'capital', 'religion', 'primary_culture', 'overlord')),
    value           TEXT,                              -- e.g. "monarchy" (government)
    value_entity_id TEXT REFERENCES entities(id),      -- e.g. dynasty / ruler / capital id
    from_year INTEGER NOT NULL, from_month INTEGER, from_day INTEGER, from_key INTEGER NOT NULL,
    to_year   INTEGER,          to_month   INTEGER, to_day   INTEGER, to_key   INTEGER,
    game            TEXT NOT NULL,
    source          TEXT NOT NULL CHECK (source IN ('save', 'snapshot_diff', 'chronicle', 'user', 'demo')),
    event_id        INTEGER REFERENCES events(event_id),
    CHECK (value IS NOT NULL OR value_entity_id IS NOT NULL),
    CHECK (to_key IS NULL OR to_key >= from_key)
);
CREATE INDEX IF NOT EXISTS ix_periods ON entity_periods(entity_id, aspect, from_key);
CREATE UNIQUE INDEX IF NOT EXISTS ux_periods_open ON entity_periods(entity_id, aspect) WHERE to_key IS NULL;

-- Territories need a display name (the timeline, map and developer tools show it).
ALTER TABLE territories ADD COLUMN name TEXT;

-- Demo data written by developer mode is tagged so it can never be mistaken for real history.
ALTER TABLE campaigns ADD COLUMN is_demo INTEGER NOT NULL DEFAULT 0;
