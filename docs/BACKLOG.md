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


## On paper: Chronicle's own Victoria 3 → HoI4 world builder (replaces the external converter)

Prerequisites
- Victoria 3 text saves (find and verify the game's switch) and a Victoria 3 adapter:
  countries, states and their owners, laws, ideologies/interest groups, pops, GDP, buildings,
  army/navy, technologies, diplomatic pacts, wars — into the campaign database.
- GeoCore dataset covering both maps; Vic3 state ↔ HoI4 state overlaps (raster method,
  control points per region); manual corrections file per game version.

Projection rules (config, not code)
- Countries: largest Vic3 countries keep HoI4 tags where history matches, new tags for the
  rest (dynamic tag table); subjects → HoI4 puppets/autonomy levels.
- Ideology: ruling interest groups + laws → HoI4 party popularity (democratic / communism /
  fascism / neutrality) and ruling party; elections from Vic3 voting laws.
- Leaders: Vic3 heads of state / IG leaders → HoI4 country leaders with traits.
- Industry: Vic3 buildings by type (arms, munitions, steel, tools) → HoI4 civilian/military
  factories and dockyards per state, normalised to the HoI4 world total.
- Resources and infrastructure from Vic3 state resources and railways.
- Technology: Vic3 techs → HoI4 1936 tech tree buckets.
- Armies: Vic3 battalions/flotillas → HoI4 divisions/fleets (templates per tech era).
- Diplomacy: alliances/defensive pacts → factions; wars at the transition date → HoI4 wars
  or claims; occupied territory → cores/claims.
- Population → manpower; culture/religion → HoI4 cores.

Output and checks
- HoI4 mod (history/countries, history/states, common/countries, localisation, flags),
  validated against the game files of the player's HoI4 version (like the Stellaris designs),
  plus a preview: countries kept/merged/split, unmatched states, low-confidence values.
- Golden tests on real Victoria 3 saves of every supported version.
