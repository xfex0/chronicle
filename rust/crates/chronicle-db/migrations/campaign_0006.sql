-- Migration 0006 (rebuild): eras can be 'skipped' — a transition may jump over games
-- (e.g. CK3 → HoI4 through external converters); skipped eras stay visible in the chain.
CREATE TABLE eras_v6 (
    game            TEXT PRIMARY KEY,
    status          TEXT NOT NULL CHECK (status IN ('planned', 'active', 'completed', 'skipped')),
    started_year    INTEGER, started_month INTEGER, started_day INTEGER,
    completed_year  INTEGER, completed_month INTEGER, completed_day INTEGER,
    final_snapshot  INTEGER REFERENCES snapshots(snapshot_id),
    conversion_id   INTEGER REFERENCES conversions(conversion_id),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO eras_v6 SELECT * FROM eras;
DROP TABLE eras;
ALTER TABLE eras_v6 RENAME TO eras;
