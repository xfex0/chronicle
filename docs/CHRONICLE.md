# Chronicle — architecture decisions (v0.1)

Formula: GAME DATA → CAMPAIGN DATABASE → SEMANTIC HISTORY → TRANSLATION → TARGET PROJECTION.
Chronicle stores FACTS, HISTORY and SEMANTICS and projects them into the next game.

Markers: ✅ confirmed · ⚠️ to verify on real files · ❓ unknown, interface only.

## 1. Repository

```
apps/chronicle/          Tauri 2 + React + TypeScript desktop app (EN/UK UI)
  src/                   screens: Dashboard, Games, Campaign, Timeline, Settings
  src-tauri/             Rust host: every UI operation is a command here
rust/crates/
  chronicle-core         ids, partial dates, game registry, settings, confidence, transition state machine
  chronicle-db           SQLite: app DB (settings, installs, campaign list) + per-campaign DB
  chronicle-steam        local Steam discovery, fingerprints, save folders (no Steam Web API)
  paradox-parser         Clausewitz text parser + save container detection
  chronicle-rng          deterministic SplitMix64 streams (reproducibility)
  geocore                weighted area projection (prototype of GeoCore)
config/games_registry.yaml   every game fact, each with a `verified` flag
tools/research/          optional Python: formula/rules prototypes (not shipped)
```

Rust does all shipped work; Python is a research tool only. One binary, simple installer.

## 2. Transition dates and where the next game starts ✅

Two separate settings per transition:

- **transition date** — when the campaign leaves the source game;
- **target start** — `native` (the target game's own start date, recommended) or `custom`
  (the target world is built AT the transition date).

| Transition | Default (native) | Alternatives |
|---|---|---|
| Imperator → CK3 | end of Imperator (27 BC) | — (gap until 867) |
| **CK3 → EU5** | **1337.4.1** (EU5 starts 1 April 1337) | **1453.1.1**: finish CK3, EU5 world starts in 1453 (custom start, advanced) |
| EU5 → Victoria 3 | 1836.1.1 | |
| Victoria 3 → HoI4 | 1936.1.1 | |
| HoI4 → Modern Era Bridge | 1948.1.1 | |
| Bridge → Stellaris | 2200.1.1 | |
| CK3 → EU4 (legacy) | 1444.11.11 | |

Validation (`GameRegistry::check_transition_date`): the date must lie in the source era.
With a **native** start it must not be after the target's start (the target would begin
earlier than the world we hand it). With a **custom** start any date moves forward.
`custom_start_supported` per transition says whether the target adapter can do it yet
(EU5: not yet — the setting can be kept, conversion waits for adapter support).

## 3. EU5 facts used ✅

Released 4 Nov 2025; era 1 Apr 1337 – 1 Jan 1837; Steam app 3450310; also sold on GOG.
Save envelope (per `jomini` test cases): ironman = binary header before a zip; debug saves =
plain text without zip. Binary formats change between patches. EU5 has no mature converter
line yet → a dedicated **EU5 research phase** precedes the EU5 target adapter (see roadmap).

## 4. Confidence: components + gates ✅

```
confidence = data_coverage × mapping_reliability          (the number users see)
gates      = version_supported AND validation_passed      (closed gate ⇒ manual review)
```

Example: CK3 centralization needs 4 signals, 3 found (coverage 0.75), formula reliability
0.90 → 0.675 → "automatic with warning" under the default policy (80 / 50). Gates are used
instead of multiplying four factors because 0.9⁴ ≈ 0.66 would push healthy values into the
warning band. All components are stored on `provenance`, so "Why?" shows them. Thresholds are
a per-campaign setting edited in the UI.

### Save formats are capabilities, not prohibitions ✅

`games_registry.yaml → save_formats` lists, per game and for this Chronicle version, whether
`plaintext`, `compressed` and `binary` (ironman / binary autosaves) can be imported:
`supported | planned | unsupported`. The UI reports "Import: planned for this game's adapter"
instead of telling players to turn ironman off. Binary support depends on per-patch token
data (and its licence) and is tracked per adapter version.

### Install sources ✅

`InstallSource = steam | gog | xbox (Xbox app / Microsoft Store) | manual | other | missing`.
Auto-detection: Steam now; GOG and Xbox are reserved in data and UI. Platforms:
Windows (MVP), Linux, Steam Deck, macOS.

## 5. Historical facts are immutable; projections are mutable ✅

`manual_overrides` rows carry `target_game`: they change how an entity is PROJECTED into that
game (suggested vs selected). Events, snapshots, periods and semantic values are never
rewritten by overrides. Example: the database says Ruthenia centralization = 0.62; EU5
projection suggests government A; the user selects B → an override row, centralization stays.

## 6. Backups and integrity ✅

`CampaignDb::backup(kind, label)` → `backups/campaign_2026-10-09_215501_<kind>_<label>.db`
via `VACUUM INTO`, registered in the `backups` table, then `PRAGMA integrity_check`.

| Kind | When | Retention |
|---|---|---|
| auto | before imports, demo seeding | newest 10 kept |
| transition | before an era transition | never deleted automatically |
| migration | before a database upgrade | never deleted automatically |
| manual | user request | never deleted automatically |

Database upgrades also run an integrity check afterwards and refuse to continue on failure.

## 7. Steam detection

Root: registry `HKCU\Software\Valve\Steam\SteamPath`, then HKLM `InstallPath`, then default
folders; Linux/macOS/Flatpak paths too. Libraries: `steamapps/libraryfolders.vdf` (new format
with `"path"`) or the older flat format. Manifests: `appmanifest_<appid>.acf` → `installdir`.
Launch: `steam://run/<appid>`. Manual links survive rescans until "Reset auto-detection".
Fingerprints marked `verified: false` never reject a folder; they only lower the badge.

## 8. Transition state machine ✅ (watcher in Phase 3)

`Idle → Ready (manual/suggested) | WaitingForGameExit (automatic) → Converting → Done/Failed`.
Automatic mode converts only after the game process exits and the save is stable.

## 9. Time periods (migration 0002) ✅

- `territory_ownership`: who owned / controlled a territory in `[from, to)`.
- `entity_periods`: government form, ruler, dynasty, capital, religion, primary culture,
  overlord of an entity in `[from, to)`.

Dates also stored as `*_key = year*10000 + month*100 + day` for index range queries
(BC years sort correctly). Partial unique indexes allow only one open period per subject.
Writes through `set_owner` / `set_period` close the open period and open a new one, and
refuse dates before the current period's start (history only moves forward). Answers:
`owner_at`, `ownership_history`, `territories_of`, `period_at`, `years_in_current_period`
("how long has this dynasty ruled?").

Migrations are versioned (`schema_version`) and run once each; opening an older campaign
backs it up first, then upgrades it.

## 10. Journal: observed vs inferred ✅ (migration 0003)

Every event records `origin` (game | save | snapshot_diff | converter | user), `evidence`
(observed | inferred) and `date_precision` (day | month | year | range | unknown; `range`
requires `date_to`). Example: snapshots 1200 and 1205 disagree on a capital → one inferred
`capital_changed` event dated 1200–1205.

## 11. GeoCore ✅ schema (migration 0003), logic in MVP 3

GeoCore is a subsystem of its own: Map Importer → Raster Analyzer → Province Geometry →
Control Points → Map Alignment → Province Crosswalk → Overlap Calculator.

- `geo_datasets` — versioned Chronicle geography (`chronicle_geo_v1`, later `v2`); a new
  dataset never rewrites history.
- `geo_areas` — areas of a dataset. **Historical Territory** (`territories` + `territory_areas`)
  is made of areas; **Game Province** (`game_provinces`, per game *and version*) is a map unit.
- `province_area_overlap` — shares in both directions; `geo_control_points` — anchors that
  align a game's map with the dataset.

v0 uses the most detailed available map as canonical grid v1; the proof of concept covers one
region (Britain first: clear coastline, easy control points; Italy second as a stress test).

## 12. Modern Era Bridge: HoI4 → Stellaris ✅ (prototype)

Crate `chronicle-bridge`, screen **Bridge → Stellaris**, CLI `chronicle-dev bridge`.

1. **Input — civilization of Earth at the end of HoI4**: 14 indicators (authoritarianism,
   social equality, xenophobia, international cooperation, religious influence, militarism,
   wars, nuclear weapons, industrialization, technology, space program, planned economy,
   environmental policy, planetary unification) + number of power blocs + dominant ideology.
   Today: manual input or presets. Later: the HoI4 adapter fills the same indicators through
   the Semantic Registry (`games/hoi4/semantic_signals.yaml` lists what it must extract;
   `CivilizationState::from_semantics` lowers data coverage for anything missing).
2. **Simulation 1948 → 2200** in decades, deterministic per seed: technology/industry growth,
   space race or cooperative space program, slow social drift, nuclear war (needs nukes,
   rivalry and low cooperation), climate crisis or ecological collapse, conquest or peaceful
   merging of blocs, space milestones, Earth unification.
3. **Stellaris empire design**: authority, ethics (always exactly 3 points; fanatic when one
   axis is very strong), 2 civics (best scoring allowed by the rules), origin
   (Post-Apocalyptic after nuclear war, Doomsday after ecological collapse, Mechanists for a
   highly automated planned economy, otherwise Prosperous Unification). Every decision carries
   reasons, a close-call alternative and confidence (coverage × mapping reliability; the
   bridge is speculative, so civics sit at 60 % and are flagged for review).
4. **Output**: the timeline can be written into the campaign journal (origin `converter`,
   payload `simulated: true`, range dates per decade; a transition backup is made first), and
   an empire card (Markdown + JSON) is exported to recreate the empire in the Stellaris
   designer. Automatic Stellaris mod generation waits for the Stellaris research phase.

Balance: `config/bridge.yaml`. Rules and names: `games/stellaris/vocabulary.yaml`
(`verified: false` until checked against the game files). The research twin
`tools/research/mega_converter/era_bridge.py` produces bit-identical results; both test
suites assert the spec examples (global democracy → Democratic, Egalitarian/Materialist/
Xenophile, Technocracy + Beacon of Liberty, Prosperous Unification; military dictatorship →
Dictatorial, Authoritarian/Militarist/Xenophobe, Distinguished Admiralty + Nationalistic Zeal).

## 13. Developer mode

See `DEVELOPER_MODE.md`: demo world, self-test, time queries, read-only SQL console, and the
`chronicle-dev` command-line tool.

## 14. Roadmap

| Step | Content | State |
|---|---|---|
| MVP 0 | Steam + manual detection, campaign creation, database, backups, developer mode | done (awaits first compile on CI) |
| MVP 1 | CK3 save parser (via `jomini` evaluation), snapshots, snapshot diff, event journal | next |
| EU5 research | save format, mod structure, map/locations, countries, cultures, religions, markets, pops, laws, governments, tech, localisation, world overrides | **in parallel with MVP 1** |
| MVP 2 | Semantic Registry, confidence, provenance, "Why?" inspector | |
| MVP 3 | GeoCore proof of concept: one region (Britain) | |
| MVP 4 | EU5 target mod prototype | |
| MVP 5 | CK3 1337 → EU5 1337 | |
| Bridge | Modern Era Bridge + Stellaris empire design from manual input | **prototype done** |
| later | HoI4 save adapter → Bridge input; Stellaris mod generation; Save Watcher → **History Collector**; automatic transitions; Victoria 3; Imperator | |

Biggest risks: reverse engineering each game's formats, GeoCore, and keeping adapters alive
across patches — answered by golden tests on real saves of every supported version.

## 15. Open items

See `BACKLOG.md` (agreed, later) and `OPEN_QUESTIONS.md` (needs a decision).
