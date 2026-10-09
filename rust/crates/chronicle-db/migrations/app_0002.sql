-- App migration 0002: more install sources (GOG, Xbox/Microsoft Store). SQLite cannot alter a
-- CHECK constraint, so the small table is rebuilt (nothing references it).
CREATE TABLE game_installations_v2 (
    game_key         TEXT PRIMARY KEY,
    source           TEXT NOT NULL CHECK (source IN ('steam', 'gog', 'xbox', 'manual', 'other', 'missing')),
    install_path     TEXT,
    save_path        TEXT,
    steam_app_id     INTEGER,
    detected_version TEXT,
    fingerprint      TEXT NOT NULL DEFAULT 'unknown',
    install_manual   INTEGER NOT NULL DEFAULT 0,
    save_manual      INTEGER NOT NULL DEFAULT 0,
    last_scan        TEXT
);
INSERT INTO game_installations_v2 SELECT * FROM game_installations;
DROP TABLE game_installations;
ALTER TABLE game_installations_v2 RENAME TO game_installations;
