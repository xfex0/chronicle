# Stellaris save format — verified facts

Source: a real ironman save, Stellaris **3.14.15 "Circinus"** (version_control_revision 26),
in-game date 2404.11.09, 23 DLCs. Re-check on every major patch.

| Fact | Value |
|---|---|
| Container | ZIP at offset 0 (no `SAV` header line), entries `gamestate` and `meta` |
| Encoding | **plain text even for ironman** (no token-encoded binary) |
| `meta` | `version="Circinus v3.14.15"`, `date="2404.11.09"`, `required_dlcs={...}`, `ironman=yes`, `meta_fleets`, `meta_planets`, `player_portrait`, `flag` |
| Size | 3.5 MB zipped → 51 MB `gamestate`, 104 top-level sections (largest: `planets`, `ships`, `pop`, `country`, `fleet`) |
| Player | `player={ { name="..." country=<id> } }` |
| Country | `country={ <id>={ ... } <id>=none }` |
| Ethics | `country.<id>.ethos={ ethic="ethic_militarist" ethic="ethic_fanatic_spiritualist" }` (repeated key) |
| Government | `country.<id>.government={ type="gov_star_empire" authority="auth_imperial" civics={ "civic_..." "civic_..." } origin="origin_..." }` |
| Name | `country.<id>.name={ key="..." literal=yes }` (literal) or a localisation key like `EMPIRE_DESIGN_humans1` |

Keys observed across the 69 countries of that save:

- authorities: `auth_democratic`, `auth_oligarchic`, `auth_dictatorial`, `auth_imperial`, `auth_corporate`, `auth_hive_mind`
- ethics: `ethic_<name>` and `ethic_fanatic_<name>` for all eight ethics, plus `ethic_gestalt_consciousness`
- origins (subset): `origin_default`, `origin_post_apocalyptic`, `origin_remnants`, `origin_necrophage`, `origin_syncretic_evolution`, `origin_life_seeded_ai_only`, …
- civics (subset): `civic_mining_guilds`, `civic_nationalistic_zeal`, `civic_aristocratic_elite`, `civic_idealistic_foundation`, `civic_parliamentary_system`, `civic_free_haven`, `civic_police_state`, `civic_environmentalist`, …

Not observed (still unverified, not necessarily wrong): `civic_technocracy`,
`civic_beacon_of_liberty`, `civic_distinguished_admiralty`, `civic_citizen_service`,
`civic_imperial_cult`, `civic_exalted_priesthood`, `civic_agrarian_idyll`,
`civic_inward_perfection`, `civic_corvee_system`, `civic_diplomatic_corps`,
`origin_doomsday`, `origin_mechanists`.

Fixture: `tests/fixtures/stellaris/3.14.15/mini_gamestate.txt` (small excerpt with the same
structure; the real save is not committed).
