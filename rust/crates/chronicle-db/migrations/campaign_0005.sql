-- Migration 0005: eras of the campaign. An era is one game's part of the world's history.
-- A transition completes the source era (keeping all its data) and activates the next one.
CREATE TABLE IF NOT EXISTS eras (
    game            TEXT PRIMARY KEY,
    status          TEXT NOT NULL CHECK (status IN ('planned', 'active', 'completed')),
    started_year    INTEGER, started_month INTEGER, started_day INTEGER,
    completed_year  INTEGER, completed_month INTEGER, completed_day INTEGER,
    final_snapshot  INTEGER REFERENCES snapshots(snapshot_id),
    conversion_id   INTEGER REFERENCES conversions(conversion_id),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
