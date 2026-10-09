// Mirrors the serde output of the Rust commands (apps/chronicle/src-tauri/src/main.rs).

export type Language = "en" | "uk";
export type TransitionMode = "manual" | "suggested" | "automatic";
export type TargetStart = "native" | "custom";
export type SaveFormat = "plaintext" | "compressed" | "binary";
export type FormatSupport = "supported" | "planned" | "unsupported";

export interface GameDef {
  key: string;
  display_name: string;
  kind: "game" | "virtual";
  steam_app_id: number | null;
  status: "mvp" | "planned" | "legacy";
  era: { start: string; end: string | null };
  era_verified: boolean;
  save_formats: Partial<Record<SaveFormat, FormatSupport>>;
  save_extensions: string[];
}

export interface TransitionDef {
  from: string;
  to: string;
  default_date: string;
  alternatives: string[];
  custom_start_supported: boolean;
  note: string | null;
}

export interface GameRegistry {
  version: number;
  games: Record<string, GameDef>;
  chain: string[];
  transitions: TransitionDef[];
}

export interface InstallationRow {
  game_key: string;
  source: "steam" | "gog" | "xbox" | "manual" | "other" | "missing";
  install_path: string | null;
  save_path: string | null;
  steam_app_id: number | null;
  detected_version: string | null;
  fingerprint: "verified" | "likely" | "unverified" | "mismatch" | "not_found" | "unknown";
  install_manual: boolean;
  save_manual: boolean;
  last_scan: string | null;
}

export interface ConfidencePolicy {
  auto_min: number;
  warn_min: number;
}

export interface CampaignSettings {
  start_game: string;
  transition_mode: TransitionMode;
  transition_dates: Record<string, string>;
  transition_target_start: Record<string, TargetStart>;
  confidence: ConfidencePolicy;
  seed: number;
  coefficient_profile: string;
}

export interface CampaignSummary {
  id: string;
  is_demo: boolean;
  name: string;
  root: string;
  current_game: string;
  current_date: string | null;
  status: string;
  settings: CampaignSettings;
  event_count: number;
  entity_count: number;
}

export interface CampaignIndexRow {
  id: string;
  name: string;
  path: string;
  created_at: string;
  last_opened: string | null;
}

export interface Bootstrap {
  version: string;
  language: Language | null;
  dev_mode: boolean;
  registry: GameRegistry;
  installations: InstallationRow[];
  campaigns: CampaignIndexRow[];
  current_campaign: CampaignSummary | null;
}

export interface SteamInstall {
  root: string;
  libraries: string[];
  log: string[];
}

export interface EventRow {
  event_id: number;
  date: string;
  date_to: string | null;
  date_precision: "day" | "month" | "year" | "range" | "unknown";
  game: string;
  event_type: string;
  actor_entity_id: string | null;
  target_entity_id: string | null;
  payload: unknown;
  importance: number;
  origin: "game" | "save" | "snapshot_diff" | "converter" | "user";
  evidence: "observed" | "inferred";
}

export interface NewCampaignInput {
  name: string;
  folder: string;
  start_game: string;
  transition_mode: TransitionMode;
  transition_dates: Record<string, string>;
  transition_target_start: Record<string, TargetStart>;
  confidence: ConfidencePolicy;
  seed: number | null;
}

export interface SaveInfo {
  path: string;
  size: number;
  container: string;
  encoding: string;
  format: SaveFormat;
  support: FormatSupport | null;
  header: string | null;
  version_hint: string | null;
  sections: [string, number][];
}

// ---- developer mode

export interface Check { name: string; ok: boolean; detail: string }
export interface DemoSummary {
  countries: number; territories: number; ownership_changes: number;
  periods: number; events: number; semantic_values: number; geo_areas: number;
}
export interface DemoResult { demo: DemoSummary; campaign: CampaignSummary }
export interface EntityLabel { id: string; kind: string; name: string | null }
export interface OwnershipPeriod {
  territory_id: string; owner_id: string | null; controller_id: string | null;
  from: string; to: string | null; game: string; source: string;
}
export type Aspect = "government" | "ruler" | "dynasty" | "capital" | "religion" | "primary_culture" | "overlord";
export interface Period {
  entity_id: string; aspect: Aspect; value: string | null; value_entity_id: string | null;
  from: string; to: string | null; game: string; source: string;
}
export interface QueryResult { columns: string[]; rows: unknown[][]; truncated: boolean }

// ---- Modern Era Bridge (HoI4 → Stellaris)

export type Ideology = "democracy" | "communism" | "fascism" | "monarchy" | "non_aligned";
export const INDICATORS = [
  "authoritarianism", "social_equality", "xenophobia", "international_cooperation", "religious_influence",
  "militarism", "global_wars", "nuclear_weapons", "industrialization", "technology", "space_program",
  "economic_planning", "environmental_policy", "planetary_unification",
] as const;
export type Indicator = (typeof INDICATORS)[number];

export interface CivilizationState {
  values: Record<Indicator, number>;
  blocs: number;
  ideology: Ideology;
  known_indicators: number;
}

export interface BridgeEvent { year: number; kind: string }
export interface Reason { code: string; value: number | null; detail: string | null }
export interface Decision {
  part: "authority" | "ethics" | "civics" | "origin";
  choice: string[];
  reasons: Reason[];
  alternative: string | null;
  confidence: number;
  review_state: "auto" | "warning" | "pending_review";
}
export interface EmpireDesign {
  authority: string;
  ethics: string[];
  civics: string[];
  origin: string;
  axes: Record<string, number>;
  decisions: Decision[];
  vocabulary_verified: boolean;
}
export interface BridgeResult {
  seed: number;
  start: CivilizationState;
  run: { final_state: CivilizationState; events: BridgeEvent[]; flags: string[]; history: [number, CivilizationState][] };
  design: EmpireDesign;
  problems: string[];
}
export interface StellarisVocabulary {
  verified: boolean;
  authorities: Record<string, string>;
  ethics: Record<string, string>;
  civics: Record<string, string>;
  origins: Record<string, string>;
}

export interface StellarisEmpire {
  name: string; name_is_literal: boolean; game_version: string | null; date: string | null; ironman: boolean | null;
  country_id: string; government_type: string | null;
  raw_authority: string | null; raw_ethics: string[]; raw_civics: string[]; raw_origin: string | null;
  authority: string | null; ethics: string[]; civics: string[]; origin: string | null;
}
export interface DesignComparison {
  authority_matches: boolean; shared_ethics: string[]; shared_civics: string[]; origin_matches: boolean; score: number;
}
export interface StellarisCompare { empire: StellarisEmpire; comparison: DesignComparison | null }

export interface Hoi4Summary {
  version: string | null; date: string | null; player: string | null;
  world_factories: number; countries_with_industry: number;
  top_powers: [string, number][]; factions: [string, number][]; countries_at_war: string[]; missing: string[];
}
export interface Hoi4Civilization { state: CivilizationState; summary: Hoi4Summary }

export interface Ck3ImportReport {
  game_version: string | null; version_verified: boolean; date: string | null;
  realms: number; counties: number; rulers: number; dynasties: number; cultures: number; faiths: number;
  new_entities: number; ownership_changes: number; events_added: number; semantic_values: number;
  player_realm: string | null; stored_copy: string; snapshot_id: number; warnings: string[];
}
export interface Ck3Import { report: Ck3ImportReport; campaign: CampaignSummary }

export interface SaveFileInfo { path: string; name: string; size: number; modified: number }
