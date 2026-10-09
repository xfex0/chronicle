# Crusader Kings III save format — verified facts

Source: a real save, **CK3 1.0.2** (save_game_version 3), bookmark 867.1.1, date 923.4.28,
12 MB on disk. Re-check on every major patch (current CK3 versions may differ).

| Fact | Value |
|---|---|
| Header line | `SAV` + `01` + `02` + 8 hex + 8 hex, e.g. `SAV0102e36d7ab2000075bc`; the last 8 hex digits are the metadata length (0x75bc = 30140 bytes) |
| Layout | header line → plain-text `meta_data={...}` (30 KB) → ZIP with one entry `gamestate` (70 MB text) |
| Encoding | plain text (this save was not ironman) |
| Top-level sections (44) | largest: `living` 23 MB, `coat_of_arms`, `opinions`, `dead_unprunable`, `dynasties`, `landed_titles`, `characters`, `provinces`, `culture_manager`, `wars`, `county_manager`, `religion`, … |
| Dates | `date=923.4.28`, `bookmark_date=867.1.1` |
| Player | `played_character={ character=<id> ... legacy={...} }`, `currently_played_characters={ <id> }` |
| Titles | `landed_titles.landed_titles.<id>={ key="k_gujarat" holder=<char> de_facto_liege=<title id> de_jure_liege name capital history={ <date>=<char> <date>={ type=created holder=<char> } <date>={ type=destroyed } } }` |
| Title count | 12 435 (8 515 baronies, 2 548 counties, 727 duchies, 189 kingdoms, 49 empires, 407 dynamic `x_`) |
| Counties | `county_manager.counties.<c_key>={ development county_control culture=<id> faith=<id> }` |
| Characters | `living.<id>={ first_name birth female culture faith dynasty_house landed_data={ domain={title ids} became_ruler_date laws={...} } }` |
| Houses | `dynasties.dynasty_house.<id>={ name="dynn_X" or localized_name, found_date, dynasty }`; `dynasties.dynasties.<id>={ key }` |
| Cultures | `culture_manager.cultures.<id>.culture_template` |
| Faiths | `religion.faiths.<id>.template`, `religion.religions.<id>.{template, family, faiths}` |
| Wars | `wars.active_wars.<id>={ attacker.participants defender.participants start_date casus_belli }` (`<id>=none` for ended) |
| Laws seen | `crown_authority_0/1`, `tribal_authority_0..3` |

Realms (independent rulers) are found by following `de_facto_liege` from every county to a
title without a liege; that title's holder rules the realm. In this save: 292 realms; the four
largest are Francia (248 counties), Arabia (153), Byzantium (145) and the Kirghiz Khanate (86).
Primary title: highest tier among the ruler's liege-less titles, ties by the order of
`landed_data.domain` (⚠️ assumption: the first entry is the primary title).

Fixture: `tests/fixtures/ck3/1.0.2/mini_gamestate.txt` (small, same structure; the real save
is not committed).
