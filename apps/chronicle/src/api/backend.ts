// The only module that talks to the Rust host. Outside Tauri (`npm run dev` in a browser)
// it serves mock data so the UI can be developed without building Rust.

import type {
  Bootstrap, CampaignSettings, CampaignSummary, Check, DemoResult, EntityLabel, EventRow, InstallationRow,
  Language, NewCampaignInput, OwnershipPeriod, Period, QueryResult, SaveInfo, SteamInstall, TargetStart,
  BridgeParams, BridgeResult, TransitionRoute, ConverterRow, InstalledConverter, SaveFormatRow, SaveModeChange, CivilizationState, Ck3Import, Hoi4Civilization, Hoi4Import, Readiness, TransitionDone, TransitionPlan, TransitionStep, SaveFileInfo, StellarisCompare, StellarisTransition, StellarisVocabulary,
} from "./types";
import { mockBootstrap, mockBridge, mockDev, mockEvents, mockTransition } from "./mock";

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
  saveFormatStatus: () =>
    call<SaveFormatRow[]>("save_format_status", {}, () => [
      { game: "hoi4", verified: true, status: { settings_file: "C:/Users/me/Documents/Paradox Interactive/Hearts of Iron IV/settings.txt", exists: true, mode: "binary" } },
    ]),
  setSaveFormat: (game: string, text: boolean) =>
    call<SaveModeChange>("set_save_format", { game, text }, () => ({
      settings_file: "C:/Users/me/Documents/Paradox Interactive/Hearts of Iron IV/settings.txt",
      backup: "…/settings.txt.chronicle-1791700000.bak", mode: text ? "text" : "binary", changed: true,
    })),
  listSaves: (game: string) =>
    call<SaveFileInfo[]>("list_saves", { game }, () =>
      game === "ck3" ? [{ path: "C:/Users/me/Downloads/Sobstvennyj_stsenarij_na_923.ck3", name: "Sobstvennyj_stsenarij_na_923.ck3", size: 12106161, modified: 1791580000 }] : []),
  resetDetection: (game: string) => call<void>("reset_game_detection", { game }, () => undefined),
  steamLog: () => call<string[]>("steam_log", {}, () => ["✓ candidate root: C:/Program Files (x86)/Steam"]),
  openFolder: (path: string) => call<void>("open_folder", { path }, () => undefined),
  launchGame: (game: string) => call<void>("launch_game", { game }, () => undefined),

  createCampaign: (input: NewCampaignInput) =>
    call<CampaignSummary>("create_campaign", { input }, () => mockBootstrap().current_campaign!),
  openCampaign: (path: string) => call<CampaignSummary>("open_campaign", { path }, () => mockBootstrap().current_campaign!),
  updateSettings: (settings: CampaignSettings) =>
    call<CampaignSummary>("update_campaign_settings", { settings }, () => ({ ...mockBootstrap().current_campaign!, settings })),
  events: (minImportance: number, game: string | null = null) =>
    call<EventRow[]>("campaign_events", { minImportance, game }, () => mockEvents(minImportance).filter((e) => !game || e.game === game)),
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
  bridgeRun: (state: CivilizationState, seed: number, startYear: number) =>
    call<BridgeResult>("bridge_run", { state, seed, startYear }, () => ({ ...mockBridge.result(state, seed), start_year: startYear })),
  bridgeCommit: (civ: CivilizationState, seed: number, startYear: number) =>
    call<number>("bridge_commit", { civ, seed, startYear }, () => 7),
  bridgeExport: (civ: CivilizationState, seed: number, startYear: number, folder: string, language: Language) =>
    call<string>("bridge_export", { civ, seed, startYear, folder, language }, () => `${folder}/chronicle_stellaris_empire_${seed}.md`),
  stellarisCreateMod: (civ: CivilizationState, seed: number, startYear: number, empireName: string, adjective: string) =>
    call<StellarisTransition>("stellaris_create_mod", { civ, seed, startYear, empireName, adjective }, () => ({
      result: {
        file: "C:/Users/me/Documents/Paradox Interactive/Stellaris/user_empire_designs_v3.4.txt",
        backup: "C:/Users/me/Documents/Paradox Interactive/Stellaris/user_empire_designs_v3.4.txt.chronicle-1791700000.bak",
        empire_name: empireName || "Terran Federation", replaced: false, warnings: [],
      },
      events_added: 7, campaign: null,
    })),
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
  importHoi4: (path: string) =>
    call<Hoi4Import>("import_hoi4_save", { path }, () => ({
      campaign: mockBootstrap().current_campaign!,
      report: {
        game_version: "1.19.3", version_verified: true, date: "1940.1.3", player: "GER", countries: 87, states: 1081,
        wars: 18, events_added: 19, ownership_changes: 1081, semantic_values: 261, new_entities: 1168, stored_copy: "original/…", warnings: [],
      },
    })),
  converterCatalog: () =>
    call<ConverterRow[]>("converter_catalog", {}, () => [
      { route: "victoria3->hoi4", name: "Victoria 3 → Hearts of Iron IV", repo: "ParadoxGameConverters/Vic3ToHoI4", program: null, version: null },
    ]),
  installConverter: (route: string) =>
    call<InstalledConverter>("install_converter", { route }, () => ({
      route, program: "C:/Users/me/AppData/Roaming/chronicle/converters/Vic3ToHoI4/1.0/ConverterFrontend.exe", version: "1.0",
      release_url: "https://github.com/ParadoxGameConverters/Vic3ToHoI4/releases",
    })),
  getConverters: () => call<Record<string, string>>("get_converters", {}, () => ({})),
  setConverter: (route: string, path: string) => call<void>("set_converter", { route, path }, () => undefined),
  launchConverter: (route: string) => call<string>("launch_converter", { route }, () => "ConverterFrontend.exe"),
  transitionReadiness: () =>
    call<Readiness>("transition_readiness", {}, () => ({
      route: { from: "hoi4", to: "stellaris", via: "modern_bridge", kind: "hoi4_to_stellaris", skipped: [] },
      newest_save: { path: "C:/saves/autosave.hoi4", name: "autosave.hoi4", size: 59_000_000, modified: 1791580000 },
      save_date: "1940.1.3", transition_date: "1948.1.1", reached: false, support: "full",
    })),
  transitionTargets: () =>
    call<TransitionRoute[]>("transition_targets", {}, () => [
      { from: "hoi4", to: "stellaris", via: "modern_bridge", kind: "hoi4_to_stellaris", skipped: [] },
    ]),
  transitionPrepare: (savePath: string | null, bridge: BridgeParams, target: string | null) =>
    call<TransitionPlan>("transition_prepare", { savePath, bridge, target }, () => mockTransition(false)),
  transitionGenerate: () =>
    call<TransitionDone>("transition_generate", {}, () => ({ plan: mockTransition(true), campaign: mockBootstrap().current_campaign! })),
  /** Live progress of a running transition (desktop only). Returns an unsubscribe function. */
  onTransitionStep: async (cb: (s: TransitionStep) => void): Promise<() => void> => {
    if (!inTauri) return () => {};
    const { listen } = await import("@tauri-apps/api/event");
    return listen<TransitionStep>("transition-step", (e) => cb(e.payload));
  },
  hoi4ReadWorld: (path: string) => call<Hoi4Civilization>("hoi4_read_world", { path }, mockBridge.hoi4),
  stellarisReadEmpire: (path: string, civ: CivilizationState | null, seed: number | null, startYear: number | null) =>
    call<StellarisCompare>("stellaris_read_empire", { path, civ, seed, startYear }, mockBridge.compare),

  pickFolder: () => pick(true),
  pickFile: () => pick(false),
};

export function errorCode(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
