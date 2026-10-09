// The only module that talks to the Rust host. Outside Tauri (`npm run dev` in a browser)
// it serves mock data so the UI can be developed without building Rust.

import type {
  Bootstrap, CampaignSettings, CampaignSummary, Check, DemoResult, EntityLabel, EventRow, InstallationRow,
  Language, NewCampaignInput, OwnershipPeriod, Period, QueryResult, SaveInfo, SteamInstall, TargetStart,
  BridgeResult, CivilizationState, Ck3Import, Hoi4Civilization, StellarisCompare, StellarisVocabulary,
} from "./types";
import { mockBootstrap, mockBridge, mockDev, mockEvents } from "./mock";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args: Record<string, unknown> = {}, mock?: () => T): Promise<T> {
  if (!inTauri) {
    if (mock) return mock();
    throw new Error("desktop_only");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

async function pick(directory: boolean): Promise<string | null> {
  if (!inTauri) return directory ? "C:/Users/me/Documents/Chronicle" : "C:/saves/autosave.ck3";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ multiple: false, directory });
  return typeof picked === "string" ? picked : null;
}

export const backend = {
  bootstrap: () => call<Bootstrap>("get_bootstrap", {}, mockBootstrap),
  setLanguage: (language: Language) => call<void>("set_language", { language }, () => undefined),

  scanGames: () =>
    call<{ steam: SteamInstall | null; installations: InstallationRow[] }>("scan_games", {}, () => ({
      steam: { root: "C:/Program Files (x86)/Steam", libraries: ["C:/Program Files (x86)/Steam", "D:/SteamLibrary"], log: [] },
      installations: mockBootstrap().installations,
    })),
  linkGameFolder: (game: string, path: string) =>
    call<InstallationRow[]>("link_game_folder", { game, path }, () => mockBootstrap().installations),
  linkSaveFolder: (game: string, path: string) =>
    call<InstallationRow[]>("link_save_folder", { game, path }, () => mockBootstrap().installations),
  resetDetection: (game: string) => call<void>("reset_game_detection", { game }, () => undefined),
  steamLog: () => call<string[]>("steam_log", {}, () => ["✓ candidate root: C:/Program Files (x86)/Steam"]),
  openFolder: (path: string) => call<void>("open_folder", { path }, () => undefined),
  launchGame: (game: string) => call<void>("launch_game", { game }, () => undefined),

  createCampaign: (input: NewCampaignInput) =>
    call<CampaignSummary>("create_campaign", { input }, () => mockBootstrap().current_campaign!),
  openCampaign: (path: string) => call<CampaignSummary>("open_campaign", { path }, () => mockBootstrap().current_campaign!),
  updateSettings: (settings: CampaignSettings) =>
    call<CampaignSummary>("update_campaign_settings", { settings }, () => ({ ...mockBootstrap().current_campaign!, settings })),
  events: (minImportance: number) => call<EventRow[]>("campaign_events", { minImportance }, () => mockEvents(minImportance)),
  backup: () => call<string>("backup_campaign", {}, () => "backups/campaign-manual-0.db"),
  checkTransitionDate: (from: string, to: string, date: string, targetStart: TargetStart) =>
    call<void>("check_transition_date", { from, to, date, targetStart }, () => undefined),
  inspectSave: (path: string, game: string | null) =>
    call<SaveInfo>("inspect_save", { path, game }, () => ({
      path, size: 0, container: "plain", encoding: "binary", format: "binary", support: "unsupported",
      header: "HOI4bin", version_hint: "1.19.3", sections: [],
    })),

  // developer mode
  setDevMode: (enabled: boolean) => call<void>("set_dev_mode", { enabled }, () => undefined),
  devSeedDemo: () => call<DemoResult>("dev_seed_demo", {}, mockDev.demo),
  devCreateDemoCampaign: (folder: string) => call<DemoResult>("dev_create_demo_campaign", { folder }, mockDev.demo),
  devSelfTest: () => call<Check[]>("dev_self_test", {}, mockDev.checks),
  devTables: () => call<[string, number][]>("dev_tables", {}, mockDev.tables),
  devEntities: (kind: string | null) => call<EntityLabel[]>("dev_entities", { kind }, () => mockDev.entities(kind)),
  devOwnerAt: (territory: string, date: string) =>
    call<OwnershipPeriod | null>("dev_owner_at", { territory, date }, () => mockDev.history(territory)[1] ?? null),
  devOwnershipHistory: (territory: string) =>
    call<OwnershipPeriod[]>("dev_ownership_history", { territory }, () => mockDev.history(territory)),
  devPeriods: (entity: string, aspect: string | null) =>
    call<Period[]>("dev_periods", { entity, aspect }, () => mockDev.periods(entity)),
  devQuery: (sql: string) => call<QueryResult>("dev_query", { sql }, () => mockDev.query(sql)),

  // Modern Era Bridge
  bridgePresets: () => call<[string, CivilizationState][]>("bridge_presets", {}, mockBridge.presets),
  bridgeRun: (state: CivilizationState, seed: number) =>
    call<BridgeResult>("bridge_run", { state, seed }, () => mockBridge.result(state, seed)),
  bridgeCommit: (civ: CivilizationState, seed: number) => call<number>("bridge_commit", { civ, seed }, () => 7),
  bridgeExport: (civ: CivilizationState, seed: number, folder: string, language: Language) =>
    call<string>("bridge_export", { civ, seed, folder, language }, () => `${folder}/chronicle_stellaris_empire_${seed}.md`),
  bridgeVocabulary: () => call<StellarisVocabulary>("bridge_vocabulary", {}, mockBridge.vocabulary),
  importCk3: (path: string) =>
    call<Ck3Import>("import_ck3_save", { path }, () => ({
      campaign: mockBootstrap().current_campaign!,
      report: {
        game_version: "1.0.2", version_verified: true, date: "923.4.28", realms: 292, counties: 2548, rulers: 292,
        dynasties: 270, cultures: 120, faiths: 60, new_entities: 3500, ownership_changes: 2548, events_added: 2900,
        semantic_values: 876, player_realm: "Гуджарат", stored_copy: "original/…", snapshot_id: 1, warnings: [],
      },
    })),
  hoi4ReadWorld: (path: string) => call<Hoi4Civilization>("hoi4_read_world", { path }, mockBridge.hoi4),
  stellarisReadEmpire: (path: string, civ: CivilizationState | null, seed: number | null) =>
    call<StellarisCompare>("stellaris_read_empire", { path, civ, seed }, mockBridge.compare),

  pickFolder: () => pick(true),
  pickFile: () => pick(false),
};

export function errorCode(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
