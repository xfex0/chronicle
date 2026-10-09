// Browser-only demo data (npm run dev). Never shipped behaviour: the desktop build uses Rust.

import type { Bootstrap, BridgeResult, Check, CivilizationState, DemoResult, EntityLabel, EventRow, Hoi4Civilization, OwnershipPeriod, Period, QueryResult, StellarisCompare, StellarisVocabulary } from "./types";

const registry: Bootstrap["registry"] = {
  version: 1,
  chain: ["imperator", "ck3", "eu5", "victoria3", "hoi4", "modern_bridge", "stellaris"],
  games: {
    imperator: { key: "imperator", display_name: "Imperator: Rome", kind: "game", steam_app_id: 859580, status: "planned", era: { start: "-304", end: "-27" }, era_verified: false, save_formats: {}, save_extensions: [] },
    ck3: { key: "ck3", display_name: "Crusader Kings III", kind: "game", steam_app_id: 1158310, status: "mvp", era: { start: "867.1.1", end: "1453.1.1" }, era_verified: true, save_formats: {}, save_extensions: [] },
    eu5: { key: "eu5", display_name: "Europa Universalis V", kind: "game", steam_app_id: 3450310, status: "planned", era: { start: "1337.4.1", end: "1837.1.1" }, era_verified: true, save_formats: {}, save_extensions: [] },
    victoria3: { key: "victoria3", display_name: "Victoria 3", kind: "game", steam_app_id: 529340, status: "planned", era: { start: "1836.1.1", end: "1936.1.1" }, era_verified: true, save_formats: {}, save_extensions: [] },
    hoi4: { key: "hoi4", display_name: "Hearts of Iron IV", kind: "game", steam_app_id: 394360, status: "planned", era: { start: "1936.1.1", end: "1948.1.1" }, era_verified: true, save_formats: {}, save_extensions: [] },
    modern_bridge: { key: "modern_bridge", display_name: "Modern Era Bridge", kind: "virtual", steam_app_id: null, status: "planned", era: { start: "1948.1.1", end: "2200.1.1" }, era_verified: true, save_formats: {}, save_extensions: [] },
    stellaris: { key: "stellaris", display_name: "Stellaris", kind: "game", steam_app_id: 281990, status: "planned", era: { start: "2200.1.1", end: null }, era_verified: true, save_formats: {}, save_extensions: [] },
    eu4: { key: "eu4", display_name: "Europa Universalis IV", kind: "game", steam_app_id: 236850, status: "legacy", era: { start: "1444.11.11", end: "1821.1.2" }, era_verified: false, save_formats: {}, save_extensions: [] },
  },
  transitions: [
    { from: "imperator", to: "ck3", default_date: "-27", alternatives: [], custom_start_supported: false, note: null },
    { from: "ck3", to: "eu5", default_date: "1337.4.1", alternatives: ["1453.1.1"], custom_start_supported: false, note: null },
    { from: "eu5", to: "victoria3", default_date: "1836.1.1", alternatives: [], custom_start_supported: false, note: null },
    { from: "victoria3", to: "hoi4", default_date: "1936.1.1", alternatives: [], custom_start_supported: false, note: null },
    { from: "hoi4", to: "modern_bridge", default_date: "1948.1.1", alternatives: [], custom_start_supported: false, note: null },
    { from: "modern_bridge", to: "stellaris", default_date: "2200.1.1", alternatives: [], custom_start_supported: false, note: null },
  ],
};

export function mockBootstrap(): Bootstrap {
  return {
    version: "0.1.0-dev",
    language: null,
    dev_mode: true,
    registry,
    installations: [
      { game_key: "ck3", source: "steam", install_path: "D:/SteamLibrary/steamapps/common/Crusader Kings III", save_path: "C:/Users/me/Documents/Paradox Interactive/Crusader Kings III/save games", steam_app_id: 1158310, detected_version: "Steam build 0", fingerprint: "likely", install_manual: false, save_manual: false, last_scan: null },
      { game_key: "eu5", source: "missing", install_path: null, save_path: null, steam_app_id: 3450310, detected_version: null, fingerprint: "not_found", install_manual: false, save_manual: false, last_scan: null },
    ],
    campaigns: [],
    current_campaign: {
      id: "campaign_demo", is_demo: true, name: "My World", root: "C:/Users/me/Documents/Chronicle/My World",
      current_game: "ck3", current_date: "867.1.1", status: "active", event_count: 2, entity_count: 0,
      settings: { start_game: "ck3", transition_mode: "suggested", transition_dates: {}, transition_target_start: {}, confidence: { auto_min: 0.8, warn_min: 0.5 }, seed: 45819283, coefficient_profile: "balanced" },
    },
  };
}

export function mockEvents(minImportance: number): EventRow[] {
  const all: EventRow[] = [
    { event_id: 1, date: "867.1.1", date_to: null, date_precision: "day", game: "ck3", event_type: "campaign_started", actor_entity_id: null, target_entity_id: null, payload: { name: "My World" }, importance: 5, origin: "converter", evidence: "observed" },
    { event_id: 2, date: "1092.4", date_to: null, date_precision: "month", game: "ck3", event_type: "civil_war", actor_entity_id: "chronicle_country_000001", target_entity_id: null, payload: {}, importance: 3, origin: "save", evidence: "observed" },
    { event_id: 3, date: "1200", date_to: "1205", date_precision: "range", game: "ck3", event_type: "capital_changed", actor_entity_id: "chronicle_country_000002", target_entity_id: null, payload: {}, importance: 3, origin: "snapshot_diff", evidence: "inferred" },
  ];
  return all.filter((e) => e.importance >= minImportance);
}

const C = (n: number) => `chronicle_country_${String(n).padStart(6, "0")}`;
const T = (n: number) => `chronicle_territory_${String(n).padStart(6, "0")}`;

export const mockDev = {
  demo: (): DemoResult => ({
    demo: { countries: 3, territories: 6, ownership_changes: 12, periods: 9, events: 13, semantic_values: 5, geo_areas: 1 },
    campaign: mockBootstrap().current_campaign!,
  }),
  checks: (): Check[] => [
    { name: "sqlite_integrity", ok: true, detail: "ok" },
    { name: "foreign_keys", ok: true, detail: "0 broken references" },
    { name: "ownership_no_overlap", ok: true, detail: "0 overlapping ownership periods" },
    { name: "low_confidence_needs_review", ok: true, detail: "0 values below 50% applied without review" },
  ],
  tables: (): [string, number][] => [["entities", 16], ["events", 13], ["territory_ownership", 12], ["entity_periods", 9]],
  entities: (kind: string | null): EntityLabel[] =>
    [
      { id: C(1), kind: "country", name: "Kingdom of Ruthenia" },
      { id: C(2), kind: "country", name: "Empire of the East" },
      { id: C(3), kind: "country", name: "Duchy of the West March" },
      { id: T(1), kind: "territory", name: "Kyiv" },
      { id: T(2), kind: "territory", name: "Halych" },
    ].filter((e) => !kind || e.kind === kind),
  history: (territory: string): OwnershipPeriod[] => [
    { territory_id: territory, owner_id: C(2), controller_id: null, from: "867.1.1", to: "1199", game: "ck3", source: "demo" },
    { territory_id: territory, owner_id: C(1), controller_id: null, from: "1199", to: "1240.12.6", game: "ck3", source: "demo" },
    { territory_id: territory, owner_id: C(3), controller_id: null, from: "1240.12.6", to: "1302", game: "ck3", source: "demo" },
    { territory_id: territory, owner_id: C(1), controller_id: null, from: "1302", to: null, game: "ck3", source: "demo" },
  ],
  periods: (entity: string): Period[] => [
    { entity_id: entity, aspect: "government", value: "principality", value_entity_id: null, from: "1021.6.1", to: "1205", game: "ck3", source: "demo" },
    { entity_id: entity, aspect: "government", value: "feudal_monarchy", value_entity_id: null, from: "1205", to: null, game: "ck3", source: "demo" },
    { entity_id: entity, aspect: "dynasty", value: null, value_entity_id: "chronicle_dynasty_000001", from: "1021.6.1", to: null, game: "ck3", source: "demo" },
  ],
  query: (sql: string): QueryResult => ({ columns: ["sql"], rows: [[sql]], truncated: false }),
};

const democracy: CivilizationState = {
  blocs: 1, ideology: "democracy", known_indicators: 14,
  values: {
    authoritarianism: 0.1, social_equality: 0.9, xenophobia: 0.1, international_cooperation: 0.9, religious_influence: 0.2,
    militarism: 0.2, global_wars: 0.05, nuclear_weapons: 0.4, industrialization: 0.7, technology: 0.6, space_program: 0.1,
    economic_planning: 0.3, environmental_policy: 0.6, planetary_unification: 0.5,
  },
};

export const mockBridge = {
  hoi4: (): Hoi4Civilization => ({
    state: { ...democracy, known_indicators: 12, blocs: 2, ideology: "fascism" },
    summary: {
      version: "Operation Postern v1.19.3.0.c01a (5632)", date: "1940.1.3.14", player: "GER",
      world_factories: 1713, countries_with_industry: 87,
      top_powers: [["USA", 0.096], ["GER", 0.086], ["SOV", 0.079]], factions: [["Axis", 0.159], ["Allies", 0.156]],
      countries_at_war: ["CHI", "JAP"], missing: ["religious_influence", "environmental_policy"],
    },
  }),
  compare: (): StellarisCompare => ({
    empire: {
      name: "Юґґот", name_is_literal: true, game_version: "Circinus v3.14.15", date: "2404.11.09", ironman: true,
      country_id: "0", government_type: "gov_star_empire",
      raw_authority: "auth_imperial", raw_ethics: ["ethic_militarist", "ethic_fanatic_spiritualist"],
      raw_civics: ["civic_mining_guilds", "civic_nationalistic_zeal"], raw_origin: "origin_necrophage",
      authority: "imperial", ethics: ["militarist", "fanatic_spiritualist"], civics: ["mining_guilds", "nationalistic_zeal"], origin: "necrophage",
    },
    comparison: { authority_matches: false, shared_ethics: ["militarist"], shared_civics: ["nationalistic_zeal"], origin_matches: false, score: 0.21 },
  }),
  presets: (): [string, CivilizationState][] => [["global_democracy", democracy]],
  vocabulary: (): StellarisVocabulary => ({
    verified: false,
    authorities: { democratic: "Democratic", oligarchic: "Oligarchic", dictatorial: "Dictatorial", imperial: "Imperial" },
    ethics: { egalitarian: "Egalitarian", xenophile: "Xenophile", materialist: "Materialist", authoritarian: "Authoritarian",
      xenophobe: "Xenophobe", militarist: "Militarist", pacifist: "Pacifist", spiritualist: "Spiritualist" },
    civics: { beacon_of_liberty: "Beacon of Liberty", technocracy: "Technocracy" },
    origins: { prosperous_unification: "Prosperous Unification" },
  }),
  result: (state: CivilizationState, seed: number): BridgeResult => ({
    seed, start: state, problems: [],
    run: {
      final_state: state, flags: ["climate_crisis", "earth_unified"], history: [],
      events: [{ year: 1978, kind: "space_orbit" }, { year: 2018, kind: "space_moon" }, { year: 2028, kind: "climate_crisis" },
        { year: 2078, kind: "space_mars" }, { year: 2088, kind: "earth_unified" }, { year: 2188, kind: "space_interstellar" }],
    },
    design: {
      authority: "democratic", ethics: ["egalitarian", "xenophile", "materialist"], civics: ["beacon_of_liberty", "technocracy"],
      origin: "prosperous_unification", vocabulary_verified: false,
      axes: { authoritarian_egalitarian: -0.8, xenophobe_xenophile: -0.78, militarist_pacifist: -0.24, spiritualist_materialist: -0.76 },
      decisions: [
        { part: "authority", choice: ["democratic"], reasons: [{ code: "authority.democratic", value: 0.18, detail: null }], alternative: null, confidence: 0.8, review_state: "auto" },
        { part: "ethics", choice: ["egalitarian", "xenophile", "materialist"], reasons: [{ code: "ethic.axis", value: -0.8, detail: "authoritarian_egalitarian" }], alternative: null, confidence: 0.75, review_state: "warning" },
        { part: "civics", choice: ["beacon_of_liberty", "technocracy"], reasons: [{ code: "civic.score", value: 0.83, detail: "beacon_of_liberty" }], alternative: null, confidence: 0.6, review_state: "warning" },
        { part: "origin", choice: ["prosperous_unification"], reasons: [{ code: "origin.default", value: null, detail: null }], alternative: null, confidence: 0.7, review_state: "warning" },
      ],
    },
  }),
};
