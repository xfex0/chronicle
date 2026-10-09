# Hearts of Iron IV save format — verified facts

Source: a real autosave from **HoI4 1.19.3 "Operation Postern"** (`v1.19.3.0.c01a`, build
5632), Germany, 1936-01-01, 30.7 MB. Re-check on every major patch.

| Fact | Value |
|---|---|
| Header | 7-byte magic **`HOI4bin`** (no `SAV` line, no ZIP) |
| Encoding | token-encoded binary: `u16 key token`, `0x0001` (=), typed values |
| Value types seen | `0x000f` quoted string (u16 length + bytes), `0x0017` unquoted string, `0x000c` i32, `0x0003` / `0x0004` block open / close |
| Readable without tokens | strings and numbers, e.g. the player tag `GER`, ideology `fascism`, the version string `Operation Postern v1.19.3.0.c01a (5632)` |
| Not readable without tokens | every key name (they are 16-bit IDs) |
| Date | the first i32 field is consistent with Clausewitz hours-since-5000-BC encoding (60759371 = 1936.1.1, hour 12) — inference, not used by code |

**Token tables are not public and must not be distributed** (the maintainer of the open-source
`hoi4save` crate states this is per Paradox's counsel). Chronicle therefore does not decode
HoI4 binary saves; it detects them, shows the version and asks for a text save.

## Text saves (`HOI4txt`) — verified on two real 1.19.3 saves (1940.1.1 and 1940.1.3)

59 MB, CRLF line endings, 76 top-level sections (largest: `countries` 45 MB,
`character_manager` 7 MB). Header lines: `player`, `ideology`, `date="1940.1.3.14"`,
`version`, `save_version=33`, `player_countries={ GER={ user=... } }`.

| Data | Path |
|---|---|
| Countries | `countries={ GER={...} ENG={...} }` — 439 entries incl. non-existent tags (no `politics`) |
| Ruling party / popularity | `countries.<TAG>.politics.ruling_party`, `politics.parties.<democratic/communism/fascism/neutrality>.popularity` (percent) |
| Laws and national spirits | `politics.ideas={ war_economy extensive_conscription ... }` (economy, conscription, trade law names verified) |
| Stability / war support | `stability=0.7`, `war_support=0.75` |
| Nukes | `nukes={ { amount=0 nukes_ready=0 } ... }` |
| Technologies | `technology.technologies.<tech>={ level research_points date }` |
| Wars | `diplomacy.active_relations.<TAG>.war_relation={ first second start_date casualties war_score_... }` (stored on one side only) |
| Factories | `states.<id>.owner` + `buildings.{industrial_complex, arms_factory, dockyard}.level` |
| Factions | top-level `faction={ name ideology members={...} }`, repeated |

The 1940 save in question started from the 1939 bookmark (flags dated 1939.8.14).
Chronicle turns this into the 14 Bridge indicators (`games/hoi4/semantic_mapping.yaml`);
religion and environment are not modelled by HoI4 and stay neutral (coverage 12/14).

## How to get a text save (community-sourced, confirm on your install)

1. Close the game.
2. Open `Documents/Paradox Interactive/Hearts of Iron IV/settings.txt`.
3. Change `save_as_binary=yes` to `save_as_binary=no`.
4. Start the game, load the campaign, save again. The new file starts with `HOI4txt`.

Ironman campaigns also write binary saves; the same setting is reported to work for them.
Confirmed by the player: after the change the game wrote `HOI4txt` saves.

## Privacy

Saves contain the player's platform user name. Real saves are never committed to the
repository; fixtures are synthetic.
