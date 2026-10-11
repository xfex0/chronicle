# Victoria 3 save format — verified facts

Source: two real saves, **Victoria 3 1.5.7** (Prussia, 1885.4.13, 31 MB) and **1.5.12**
("United States of Europe", 52 MB). Re-check on every major patch.

| Fact | Value |
|---|---|
| Header | `SAV01` + kind + 8 hex + 8 hex metadata length, e.g. `SAV0103da6d73a800000296` (0x296 = 662 bytes of metadata before the ZIP) or `SAV0105670cbe1900000000` (no metadata; `meta` is a ZIP entry) |
| Container | ZIP with `gamestate` (159–233 MB unpacked) and, in the second layout, `meta` |
| Encoding | token-encoded **binary** in both saves → not readable by Chronicle (token tables may not be distributed) |
| Readable without tokens | plain strings in the metadata: game version (`1.5.12`), the save label (player's country name), country rank (`great_power`), DLC names, game rules |
| Date | not readable without tokens; Chronicle takes it from the file name when present (`prussia_1885_04_13.v3`) |

Text saves: the Pdx-Unlimiter documentation says that for Victoria 3 only the game's own debug
mode or settings can produce unencrypted saves; the exact switch is not verified yet, so
Chronicle does not offer it in Settings → Save format.
