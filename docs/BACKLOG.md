# Backlog — agreed, scheduled for later

- **GOG / Xbox detection**: enum values exist (`gog`, `xbox`); implement auto-detection
  (GOG Galaxy registry/paths, Xbox app install folders and save locations).
- **Linux / Steam Deck via Proton**: saves live inside the Proton prefix
  (`steamapps/compatdata/<appid>/pfx/...`).
- **Binary / ironman import**: capability per adapter version is modelled
  (`save_formats`); implementation needs token data — evaluate `jomini`, check licences.
- **Stable-save detector** (Steam Cloud safe): write detected → wait → size stable → mtime
  stable → open shared-read → checksum → short wait → checksum again → equal ⇒ stable.
- **History Collector**: every valid autosave → lightweight parse (only needed sections via the
  top-level index, per-entity state hashes) → diff against the last snapshot → events.
  Important saves → full snapshot; routine autosaves → diff only.
- **Ukrainian installer language**: the app UI is EN/UK; the NSIS installer is English only
  until a Ukrainian NSIS translation is verified with Tauri's bundler.
- **Code signing** for public releases (removes the SmartScreen warning).
- **History Optimizer** (summarize low-value events; `events.summarized_into` reserved).
- **PostgreSQL** for multiplayer/server campaigns.
- **Mod compatibility packages** (version 1 = vanilla + official DLC only).
