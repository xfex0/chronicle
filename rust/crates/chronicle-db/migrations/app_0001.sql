-- Application database (one per user): settings, detected games, list of campaigns.

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS game_installations (
    game_key         TEXT PRIMARY KEY,
    source           TEXT NOT NULL CHECK (source IN ('steam', 'manual', 'other', 'missing')),
    install_path     TEXT,
    save_path        TEXT,
    steam_app_id     INTEGER,
    detected_version TEXT,
    fingerprint      TEXT NOT NULL DEFAULT 'unknown',  -- verified | unverified | mismatch | unknown
    install_manual   INTEGER NOT NULL DEFAULT 0,       -- user override survives rescans
    save_manual      INTEGER NOT NULL DEFAULT 0,
    last_scan        TEXT
);

CREATE TABLE IF NOT EXISTS campaign_index (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    path        TEXT NOT NULL UNIQUE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    last_opened TEXT
);

