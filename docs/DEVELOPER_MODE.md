# Developer mode / Режим розробника

Test Chronicle without real saves. Everything below works on a **demo campaign** — a small
fictional world. Demo data is always tagged (`campaigns.is_demo = 1`, `source = 'demo'`,
`"demo": true` in event payloads) and can only be written into an EMPTY campaign, so it never
mixes with real history.

## In the app

Settings → **Developer mode** → a **Developer** screen appears:

| Tool | What it tests |
|---|---|
| Demo world | creates/fills a campaign: 3 countries, 6 territories, 12 ownership changes, government/dynasty/capital/overlord periods, journal events, semantic values with provenance and confidence |
| Self-test | SQLite integrity, foreign keys, schema version, no overlapping periods, current owner = open ownership period, valid event dates, low-confidence values sent to review, every semantic value has a "Why?" |
| Who owned it when? | `owner_at(territory, date)` + full ownership history (try Halych at 1250, 1300, 1400) |
| State periods | government / ruler / dynasty / capital / overlord periods of a country |
| SQL console | any `SELECT` on a separate read-only connection (writes are rejected) |
| Tables | row count per table |

## In a terminal (no GUI build needed)

```bash
cd rust
cargo run -p chronicle-devtools -- demo  ../demo-campaign
cargo run -p chronicle-devtools -- check ../demo-campaign
cargo run -p chronicle-devtools -- owner ../demo-campaign Halych 1250
cargo run -p chronicle-devtools -- history ../demo-campaign Halych
cargo run -p chronicle-devtools -- periods ../demo-campaign "Kingdom of Ruthenia" dynasty
cargo run -p chronicle-devtools -- sql ../demo-campaign "SELECT * FROM entity_periods"
cargo run -p chronicle-devtools -- steam       # real Steam detection on this PC
cargo run -p chronicle-devtools -- registry    # transition dates, alternatives, custom-start support
cargo run -p chronicle-devtools -- backups ../demo-campaign
cargo run -p chronicle-devtools -- bridge military_dictatorship 45819283   # HoI4 → Stellaris
cargo run -p chronicle-devtools -- stellaris "C:\path\to\ironman.sav"     # read a real Stellaris save
cargo run -p chronicle-devtools -- hoi4 "C:\path\to\autosave.hoi4"        # HoI4 text save → Bridge → Stellaris
cargo run -p chronicle-devtools -- ck3 "C:\path\to\save.ck3"               # realms and the player in a CK3 save
cargo run -p chronicle-devtools -- import ../my-campaign "C:\path\to\save.ck3"  # import into a campaign
```

The demo + self-test also run in CI on Windows and Linux, and before every installer build.

## Expected demo answers

| Question | Answer |
|---|---|
| Halych owner in 1250 | Duchy of the West March (lost 6 Dec 1240, regained 1302) |
| Kyiv owner in 1000 / 1100 | Empire of the East / Kingdom of Ruthenia |
| Ruthenia government in 1100 / 1300 | principality / feudal_monarchy (from 1205) |
| Volodarids ruling Ruthenia by 1336 | 314 years |
| `merchant_power` review state | `pending_review` (confidence 0.43 < 0.50) |
| `regional_autonomy` review state | `pending_review` (0.90, but the validation gate is closed) |
| Empire capital change | inferred event, 1200–1205, origin `snapshot_diff` |
| Kyiv in GeoCore | 1 geo area → 70 % / 30 % across two demo CK3 provinces, 100 % into one EU5 location |
