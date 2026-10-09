// Chronicle desktop host. The UI never reads VDF/ACF files or the database itself: every
// operation is a command here, backed by the Rust crates in /rust/crates.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use chronicle_core::registry::{FormatSupport, GameRegistry, SaveFormat};
use chronicle_core::{CampaignSettings, ConfidencePolicy, PartialDate, TargetStart, TransitionMode};
use chronicle_db::dev::{self as devtools, Check, DemoSummary, EntityLabel, QueryResult};
use chronicle_db::{Aspect, AppDb, BackupKind, CampaignDb, CampaignIndexRow, CampaignSummary, EventRow, InstallationRow, OwnershipPeriod, Period};
use chronicle_steam::{detect_all, find_steam, verify_install_dir, SteamInstall};
use paradox_parser::container::{ContainerKind, Encoding, TextEncoding};
use paradox_parser::{detect, source, TopLevelIndex};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

struct AppState {
    db: Mutex<AppDb>,
    registry: GameRegistry,
    campaign: Mutex<Option<CampaignDb>>,
    steam: Mutex<Option<SteamInstall>>,
}

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------------------------------------------------------------- bootstrap & settings

#[derive(Serialize)]
struct Bootstrap {
    version: &'static str,
    language: Option<String>,
    dev_mode: bool,
    registry: GameRegistry,
    installations: Vec<InstallationRow>,
    campaigns: Vec<CampaignIndexRow>,
    current_campaign: Option<CampaignSummary>,
}

#[tauri::command]
fn get_bootstrap(state: State<'_, AppState>) -> CmdResult<Bootstrap> {
    let current = {
        let guard = state.campaign.lock().map_err(err)?;
        match guard.as_ref() {
            Some(c) => Some(c.summary().map_err(err)?),
            None => None,
        }
    };
    let db = state.db.lock().map_err(err)?;
    Ok(Bootstrap {
        version: chronicle_core::CHRONICLE_VERSION,
        language: db.get_setting("language").map_err(err)?,
        dev_mode: db.get_setting("dev_mode").map_err(err)?.as_deref() == Some("1"),
        registry: state.registry.clone(),
        installations: db.installations().map_err(err)?,
        campaigns: db.campaigns().map_err(err)?,
        current_campaign: current,
    })
}

#[tauri::command]
fn set_language(state: State<'_, AppState>, language: String) -> CmdResult<()> {
    if !matches!(language.as_str(), "en" | "uk") {
        return Err(format!("unsupported language {language}"));
    }
    state.db.lock().map_err(err)?.set_setting("language", &language).map_err(err)
}

// ---------------------------------------------------------------- games & Steam

#[derive(Serialize)]
struct ScanResult {
    steam: Option<SteamInstall>,
    installations: Vec<InstallationRow>,
}

#[tauri::command]
async fn scan_games(state: State<'_, AppState>) -> CmdResult<ScanResult> {
    let registry = state.registry.clone();
    let (steam, detections) = tauri::async_runtime::spawn_blocking(move || {
        let steam = find_steam();
        let detections = detect_all(&registry, steam.as_ref());
        (steam, detections)
    })
    .await
    .map_err(err)?;

    *state.steam.lock().map_err(err)? = steam.clone();
    let db = state.db.lock().map_err(err)?;
    for d in &detections {
        db.upsert_detected(&InstallationRow {
            game_key: d.game_key.clone(),
            source: d.source.as_str().into(),
            install_path: d.install_path.as_ref().map(|p| p.display().to_string()),
            save_path: d.save_path.as_ref().map(|p| p.display().to_string()),
            steam_app_id: d.steam_app_id,
            detected_version: d.build_id.as_ref().map(|b| format!("Steam build {b}")),
            fingerprint: d.fingerprint.as_str().into(),
            install_manual: false,
            save_manual: false,
            last_scan: None,
        })
        .map_err(err)?;
    }
    let installations = db.installations().map_err(err)?;
    Ok(ScanResult { steam, installations })
}

#[tauri::command]
fn link_game_folder(state: State<'_, AppState>, game: String, path: String) -> CmdResult<Vec<InstallationRow>> {
    let def = state.registry.game(&game).ok_or_else(|| format!("unknown game {game}"))?;
    let result = verify_install_dir(def, &PathBuf::from(&path));
    if !result.acceptable() {
        return Err(format!("not_a_game_folder:{}", result.as_str()));
    }
    let db = state.db.lock().map_err(err)?;
    db.set_manual_install(&game, &path, result.as_str()).map_err(err)?;
    db.installations().map_err(err)
}

#[tauri::command]
fn link_save_folder(state: State<'_, AppState>, game: String, path: String) -> CmdResult<Vec<InstallationRow>> {
    if !PathBuf::from(&path).is_dir() {
        return Err("folder_not_found".into());
    }
    let db = state.db.lock().map_err(err)?;
    db.set_manual_save(&game, &path).map_err(err)?;
    db.installations().map_err(err)
}

#[tauri::command]
fn reset_game_detection(state: State<'_, AppState>, game: String) -> CmdResult<()> {
    state.db.lock().map_err(err)?.reset_manual(&game).map_err(err)
}

#[tauri::command]
fn steam_log(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(state.steam.lock().map_err(err)?.as_ref().map(|s| s.log.clone()).unwrap_or_default())
}

#[tauri::command]
fn open_folder(app: tauri::AppHandle, path: String) -> CmdResult<()> {
    if !PathBuf::from(&path).exists() {
        return Err("folder_not_found".into());
    }
    app.opener().open_path(path, None::<&str>).map_err(err)
}

#[tauri::command]
fn launch_game(app: tauri::AppHandle, state: State<'_, AppState>, game: String) -> CmdResult<()> {
    let url = state
        .registry
        .game(&game)
        .and_then(|g| g.steam_launch_url())
        .ok_or_else(|| "no_steam_launch".to_string())?;
    app.opener().open_url(url, None::<&str>).map_err(err)
}

// ---------------------------------------------------------------- campaigns

#[derive(Deserialize)]
struct NewCampaignInput {
    name: String,
    folder: String,
    start_game: String,
    transition_mode: TransitionMode,
    #[serde(default)]
    transition_dates: BTreeMap<String, String>,
    #[serde(default)]
    transition_target_start: BTreeMap<String, TargetStart>,
    confidence: ConfidencePolicy,
    seed: Option<u64>,
}

fn parse_dates(raw: &BTreeMap<String, String>) -> CmdResult<BTreeMap<String, PartialDate>> {
    raw.iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| v.parse::<PartialDate>().map(|d| (k.clone(), d)).map_err(err))
        .collect()
}

fn auto_seed() -> u64 {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    (nanos % 1_000_000_000) as u64
}

fn activate(state: &AppState, db: CampaignDb) -> CmdResult<CampaignSummary> {
    let summary = db.summary().map_err(err)?;
    state
        .db
        .lock()
        .map_err(err)?
        .register_campaign(&summary.id, &summary.name, &summary.root)
        .map_err(err)?;
    *state.campaign.lock().map_err(err)? = Some(db);
    Ok(summary)
}

#[tauri::command]
fn create_campaign(state: State<'_, AppState>, input: NewCampaignInput) -> CmdResult<CampaignSummary> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("name_required".into());
    }
    let mut settings = CampaignSettings::new(&input.start_game, input.seed.unwrap_or_else(auto_seed));
    settings.transition_mode = input.transition_mode;
    settings.transition_dates = parse_dates(&input.transition_dates)?;
    settings.transition_target_start = input.transition_target_start;
    settings.confidence = input.confidence;
    let db = CampaignDb::create(&PathBuf::from(&input.folder), name, &settings, &state.registry).map_err(err)?;
    activate(&state, db)
}

#[tauri::command]
fn open_campaign(state: State<'_, AppState>, path: String) -> CmdResult<CampaignSummary> {
    let db = CampaignDb::open(&PathBuf::from(&path)).map_err(err)?;
    activate(&state, db)
}

#[tauri::command]
fn update_campaign_settings(state: State<'_, AppState>, settings: CampaignSettings) -> CmdResult<CampaignSummary> {
    let guard = state.campaign.lock().map_err(err)?;
    let db = guard.as_ref().ok_or("no_campaign")?;
    db.update_settings(&settings, &state.registry).map_err(err)?;
    db.summary().map_err(err)
}

#[tauri::command]
fn campaign_events(state: State<'_, AppState>, min_importance: u8) -> CmdResult<Vec<EventRow>> {
    let guard = state.campaign.lock().map_err(err)?;
    guard.as_ref().ok_or("no_campaign")?.events(min_importance, 5000).map_err(err)
}

#[tauri::command]
fn backup_campaign(state: State<'_, AppState>) -> CmdResult<String> {
    let guard = state.campaign.lock().map_err(err)?;
    let path = guard.as_ref().ok_or("no_campaign")?.backup(BackupKind::Manual, "manual").map_err(err)?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn check_transition_date(
    state: State<'_, AppState>,
    from: String,
    to: String,
    date: String,
    target_start: Option<TargetStart>,
) -> CmdResult<()> {
    let d: PartialDate = date.parse().map_err(err)?;
    state
        .registry
        .check_transition_date(&from, &to, d, target_start.unwrap_or_default())
        .map_err(err)
}

// ---------------------------------------------------------------- developer mode
//
// Every dev command works on the open campaign. Writes are limited to seeding demo data into
// an EMPTY campaign; the SQL console is read-only.

#[tauri::command]
fn set_dev_mode(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    state.db.lock().map_err(err)?.set_setting("dev_mode", if enabled { "1" } else { "0" }).map_err(err)
}

#[derive(Serialize)]
struct DemoResult {
    demo: DemoSummary,
    campaign: CampaignSummary,
}

#[tauri::command]
fn dev_seed_demo(state: State<'_, AppState>) -> CmdResult<DemoResult> {
    let mut guard = state.campaign.lock().map_err(err)?;
    let db = guard.as_mut().ok_or("no_campaign")?;
    let demo = devtools::seed_demo(db).map_err(err)?;
    Ok(DemoResult { demo, campaign: db.summary().map_err(err)? })
}

#[tauri::command]
fn dev_create_demo_campaign(state: State<'_, AppState>, folder: String) -> CmdResult<DemoResult> {
    let root = PathBuf::from(folder).join("Chronicle demo");
    let mut db = CampaignDb::create(&root, "Demo world", &CampaignSettings::new("ck3", 45_819_283), &state.registry)
        .map_err(err)?;
    let demo = devtools::seed_demo(&mut db).map_err(err)?;
    let campaign = activate(&state, db)?;
    Ok(DemoResult { demo, campaign })
}

fn with_campaign<T>(state: &AppState, f: impl FnOnce(&CampaignDb) -> Result<T, chronicle_db::DbError>) -> CmdResult<T> {
    let guard = state.campaign.lock().map_err(err)?;
    f(guard.as_ref().ok_or("no_campaign")?).map_err(err)
}

#[tauri::command]
fn dev_self_test(state: State<'_, AppState>) -> CmdResult<Vec<Check>> {
    with_campaign(&state, devtools::self_test)
}

#[tauri::command]
fn dev_tables(state: State<'_, AppState>) -> CmdResult<Vec<(String, i64)>> {
    with_campaign(&state, devtools::table_counts)
}

#[tauri::command]
fn dev_entities(state: State<'_, AppState>, kind: Option<String>) -> CmdResult<Vec<EntityLabel>> {
    with_campaign(&state, |db| devtools::entity_labels(db, kind.as_deref()))
}

#[tauri::command]
fn dev_owner_at(state: State<'_, AppState>, territory: String, date: String) -> CmdResult<Option<OwnershipPeriod>> {
    let d: PartialDate = date.parse().map_err(err)?;
    with_campaign(&state, |db| db.owner_at(&territory, d))
}

#[tauri::command]
fn dev_ownership_history(state: State<'_, AppState>, territory: String) -> CmdResult<Vec<OwnershipPeriod>> {
    with_campaign(&state, |db| db.ownership_history(&territory))
}

#[tauri::command]
fn dev_periods(state: State<'_, AppState>, entity: String, aspect: Option<String>) -> CmdResult<Vec<Period>> {
    let aspect = match aspect.as_deref() {
        Some(a) => Some(Aspect::parse(a).ok_or_else(|| format!("unknown aspect {a}"))?),
        None => None,
    };
    with_campaign(&state, |db| db.periods(&entity, aspect))
}

#[tauri::command]
fn dev_query(state: State<'_, AppState>, sql: String) -> CmdResult<QueryResult> {
    with_campaign(&state, |db| devtools::readonly_query(db, &sql, 500))
}

// ---------------------------------------------------------------- save inspection

#[derive(Serialize)]
struct SaveInfo {
    path: String,
    size: usize,
    container: &'static str,
    encoding: &'static str,
    /// plaintext | compressed | binary
    format: &'static str,
    /// What this Chronicle version can import for the given game (None if no game given).
    support: Option<FormatSupport>,
    sections: Vec<(String, usize)>,
}

fn inspect_impl(path: PathBuf, game: Option<chronicle_core::GameDef>) -> Result<SaveInfo, String> {
    let bytes = source::open(&path).map_err(err)?;
    let d = detect(&bytes);
    let container = match d.container {
        ContainerKind::Plain => "plain",
        ContainerKind::Zip => "zip",
        ContainerKind::Gzip => "gzip",
        ContainerKind::Zstd => "zstd",
    };
    let encoding = match d.encoding {
        Encoding::Text(TextEncoding::Utf8) => "utf8",
        Encoding::Text(TextEncoding::Windows1252) => "windows1252",
        Encoding::Binary => "binary",
        Encoding::Unknown => "unknown",
    };
    // A zip may hold text or binary (ironman); it is reported as "compressed" until opened.
    let format = match (d.container, d.encoding) {
        (ContainerKind::Plain, Encoding::Binary) => SaveFormat::Binary,
        (ContainerKind::Plain, _) => SaveFormat::Plaintext,
        _ => SaveFormat::Compressed,
    };
    let support = game.map(|g| g.format_support(format));
    let sections = if d.container == ContainerKind::Plain && d.encoding != Encoding::Binary {
        TopLevelIndex::build(&bytes[d.payload_offset..])
            .map_err(err)?
            .summary()
            .into_iter()
            .take(60)
            .map(|s| (s.key, s.total_bytes))
            .collect()
    } else {
        Vec::new()
    };
    Ok(SaveInfo {
        path: path.display().to_string(),
        size: bytes.len(),
        container,
        encoding,
        format: format.as_str(),
        support,
        sections,
    })
}

#[tauri::command]
async fn inspect_save(state: State<'_, AppState>, path: String, game: Option<String>) -> CmdResult<SaveInfo> {
    let def = game.and_then(|g| state.registry.game(&g).cloned());
    tauri::async_runtime::spawn_blocking(move || inspect_impl(PathBuf::from(path), def))
        .await
        .map_err(err)?
}

// ---------------------------------------------------------------- main

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let registry = GameRegistry::builtin()?;
            let db_path = app.path().app_data_dir()?.join("chronicle.db");
            let db = AppDb::open(&db_path)?;
            // Re-open the most recent campaign whose folder still exists.
            let campaign = db
                .campaigns()?
                .into_iter()
                .find_map(|c| CampaignDb::open(&PathBuf::from(&c.path)).ok());
            app.manage(AppState {
                db: Mutex::new(db),
                registry,
                campaign: Mutex::new(campaign),
                steam: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_bootstrap,
            set_language,
            scan_games,
            link_game_folder,
            link_save_folder,
            reset_game_detection,
            steam_log,
            open_folder,
            launch_game,
            create_campaign,
            open_campaign,
            update_campaign_settings,
            campaign_events,
            backup_campaign,
            check_transition_date,
            inspect_save,
            set_dev_mode,
            dev_seed_demo,
            dev_create_demo_campaign,
            dev_self_test,
            dev_tables,
            dev_entities,
            dev_owner_at,
            dev_ownership_history,
            dev_periods,
            dev_query
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Chronicle");
}
