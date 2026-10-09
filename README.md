# Chronicle — Paradox Mega Campaign Manager

One world, many eras: Imperator → Crusader Kings III → Europa Universalis V → Victoria 3 →
Hearts of Iron IV → Modern Era Bridge → Stellaris. Chronicle keeps the world's history in a
campaign database and projects it into each next game. It is not a save-to-save converter.

Interface: English and Ukrainian. Windows installer for everyone (no developer tools needed).

## Get the installer

- **GitHub (no tools)**: push this repo → Actions → *build-windows* → Run workflow → download
  `chronicle-windows` from Artifacts.
- **Locally**: install Node.js 20, Rust, VS Build Tools (C++), then
  `powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1`.

## What version 0.1 does (Phase 1)

- Finds Steam and all its libraries from local files; detects the supported games, their
  folders, save folders (incl. OneDrive-redirected Documents) and Steam build.
- Manual game/save folder linking that survives rescans; open folders; launch via Steam.
- Creates campaigns: folder layout, campaign database, stable Chronicle ids, event journal.
- Per-campaign settings: transition mode (manual/suggested/automatic), transition dates
  (default CK3→EU5 = 1337.4.1, validated so history only moves forward), confidence thresholds.
- Dashboard with the campaign chain, Timeline, database backups, read-only save inspector.

Not yet: reading CK3 saves into the database (Phase 2), GeoCore/map (Phase 3), conversion.

## Develop

```bash
cd rust && cargo test --workspace          # core
cd apps/chronicle && npm install && npm run dev        # UI in a browser with mock data
cd apps/chronicle && npm run tauri dev                 # desktop app
```

Developer mode (demo world, self-test, time queries, SQL console) — in the app under
Settings, or from a terminal: `cd rust && cargo run -p chronicle-devtools -- demo ../demo && cargo run -p chronicle-devtools -- check ../demo`.
See `docs/DEVELOPER_MODE.md`.

Docs: `docs/CHRONICLE.md` (decisions), `docs/DEVELOPER_MODE.md`, `docs/BACKLOG.md`, `docs/OPEN_QUESTIONS.md`.
