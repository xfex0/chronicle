//! Era transition — the centre of a Chronicle campaign.
//!
//! ```text
//! final save of the current game ──► Chronicle database ──► next game's world
//! ```
//!
//! Fifteen steps in two phases. `prepare` runs steps 1–11 and stops at a preview; nothing is
//! generated for the next game until the player confirms. `generate` runs steps 12–15.
//! The old era is never deleted: its save is copied (read-only), its history stays in the
//! database, and it is marked completed.
//!
//! Supported routes today:
//! - **HoI4 → (Modern Era Bridge) → Stellaris**: every step (GeoCore is not needed: one planet).
//! - **CK3 → EU5**: steps 1–7 (final import, snapshot, history, semantics); building the EU5
//!   world waits for GeoCore and the EU5 adapter, so the preview is shown as blocked.
//! - others: blocked at the import step until their adapters exist.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chronicle_bridge::{BridgeResult, CivilizationState, DesignsResult, EmpireDesign};
use chronicle_core::registry::GameRegistry;
use chronicle_core::{EventOrigin, PartialDate};
use chronicle_db::{BackupKind, CampaignDb, DbError, NewEvent};
use chronicle_steam::SaveFileInfo;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod vic3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepId {
    FindSave,
    CheckSave,
    Backup,
    ImportFinal,
    FinalSnapshot,
    DiffHistory,
    Semantics,
    GeoCore,
    Coefficients,
    BuildWorld,
    Preview,
    Generate,
    Validate,
    CloseEra,
    NextEra,
}

pub const STEPS: [StepId; 15] = [
    StepId::FindSave,
    StepId::CheckSave,
    StepId::Backup,
    StepId::ImportFinal,
    StepId::FinalSnapshot,
    StepId::DiffHistory,
    StepId::Semantics,
    StepId::GeoCore,
    StepId::Coefficients,
    StepId::BuildWorld,
    StepId::Preview,
    StepId::Generate,
    StepId::Validate,
    StepId::CloseEra,
    StepId::NextEra,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Done,
    /// Not needed for this route (e.g. GeoCore for HoI4 → Stellaris).
    Skipped,
    /// Not possible yet: an adapter or subsystem is missing. `note` says which.
    Blocked,
    Warning,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: StepId,
    pub status: StepStatus,
    /// Numbers to show ("countries": 428, …).
    pub counts: BTreeMap<String, i64>,
    /// A code the UI translates (e.g. "transition_date_not_reached") or a short text.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteKind {
    Hoi4ToStellaris,
    Ck3ToEu5,
    /// Any game → any later game through an external converter (e.g. Paradox Game Converters'
    /// Victoria 3 → HoI4): Chronicle imports what it can read, keeps the save, launches the
    /// converter, checks its mod and records the era change (games in between: skipped).
    External,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub from: String,
    pub to: String,
    /// Intermediate stage that is simulated, not played (the Modern Era Bridge).
    pub via: Option<String>,
    pub kind: RouteKind,
    /// Games of the chain jumped over by this transition (marked skipped afterwards).
    #[serde(default)]
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GameDirs {
    pub save_dir: Option<PathBuf>,
    pub install_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeParams {
    pub seed: u64,
    /// None = the save's own year.
    pub start_year: Option<i32>,
    pub empire_name: String,
    pub adjective: String,
}

impl Default for BridgeParams {
    fn default() -> Self {
        Self { seed: 45_819_283, start_year: None, empire_name: String::new(), adjective: "Terran".into() }
    }
}

pub struct Context<'a> {
    pub registry: &'a GameRegistry,
    pub dirs: BTreeMap<String, GameDirs>,
    pub bridge: BridgeParams,
    /// External converters by route ("victoria3->hoi4" → path to the converter program).
    pub converters: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Preview {
    pub source_countries: usize,
    /// None when the target world cannot be built yet.
    pub target_countries: Option<usize>,
    pub merged: usize,
    pub split: usize,
    pub vanished: usize,
    pub created: usize,
    pub low_confidence: usize,
    pub geocore_warnings: usize,
    pub critical_errors: usize,
    pub notes: Vec<String>,
    pub design: Option<EmpireDesign>,
    pub start_year: Option<i32>,
    pub bridge_events: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransitionPlan {
    pub route: Route,
    pub save: Option<String>,
    pub save_date: Option<PartialDate>,
    pub transition_date: Option<PartialDate>,
    pub steps: Vec<Step>,
    pub preview: Option<Preview>,
    pub can_generate: bool,
    pub blocked_reason: Option<String>,
    pub conversion_id: Option<i64>,
    pub snapshot_id: Option<i64>,
    /// Where Earth's empire was written (the player's own Stellaris empire designs).
    pub generated: Option<DesignsResult>,
    pub finished: bool,
    /// Kept for `generate` (HoI4 route): the civilization read from the final save.
    pub civ: Option<CivilizationState>,
    pub bridge: BridgeParams,
    /// Unix time when steps 1–11 ran (external converter output must be newer).
    pub prepared_at: u64,
    /// External converter for this route, when one is used.
    pub converter: Option<String>,
    /// The converter's output found in step 12 (HoI4 mod descriptor).
    pub external_output: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Readiness {
    pub route: Option<Route>,
    pub newest_save: Option<SaveFileInfo>,
    pub save_date: Option<PartialDate>,
    pub transition_date: Option<PartialDate>,
    pub reached: bool,
    /// full | partial | none
    pub support: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("there is no next game after {0} in the campaign chain")]
    NoNextGame(String),
    #[error("nothing to generate: {0}")]
    NotReady(String),
}

// ------------------------------------------------------------------------------- routing

/// Route from `from` to a later game `to` (None if `to` is not after `from` in the chain).
pub fn route_to(registry: &GameRegistry, from: &str, to: &str) -> Option<Route> {
    let chain = &registry.chain;
    let i = chain.iter().position(|g| g == from)?;
    let j = chain.iter().position(|g| g == to)?;
    if j <= i || registry.game(to).is_some_and(|g| g.kind == chronicle_core::registry::GameKind::Virtual) {
        return None;
    }
    let between: Vec<String> = chain[i + 1..j].to_vec();
    let (kind, via, skipped) = match (from, to) {
        ("hoi4", "stellaris") => (RouteKind::Hoi4ToStellaris, Some("modern_bridge".to_string()), Vec::new()),
        ("ck3", "eu5") => (RouteKind::Ck3ToEu5, None, Vec::new()),
        _ => (RouteKind::External, None, between),
    };
    Some(Route { from: from.to_string(), to: to.to_string(), via, kind, skipped })
}

/// Every later game a campaign in `current` can move to, nearest first.
pub fn targets(registry: &GameRegistry, current: &str) -> Vec<Route> {
    let Some(i) = registry.chain.iter().position(|g| g == current) else { return Vec::new() };
    registry.chain[i + 1..].iter().filter_map(|to| route_to(registry, current, to)).collect()
}

/// The default route: to the next playable game.
pub fn route_from(registry: &GameRegistry, current: &str) -> Option<Route> {
    targets(registry, current).into_iter().next()
}

/// The transition date for the first leg (settings first, then the registry default).
pub fn transition_date(db: &CampaignDb, registry: &GameRegistry, route: &Route) -> Result<Option<PartialDate>, DbError> {
    let next = route.via.clone().or_else(|| route.skipped.first().cloned()).unwrap_or_else(|| route.to.clone());
    let settings = db.summary()?.settings;
    Ok(settings
        .transition_dates
        .get(&format!("{}->{}", route.from, next))
        .copied()
        .or_else(|| registry.transition(&route.from, &next).map(|t| t.default_date)))
}

/// In-game date of a save from its first bytes (cheap: no full parse).
pub fn quick_save_date(path: &Path, game: &str) -> Option<PartialDate> {
    use std::io::Read;
    let mut head = vec![0u8; 256 * 1024];
    let n = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    let text = String::from_utf8_lossy(&head[..n]);
    let after = |key: &str| -> Option<String> {
        let i = text.find(key)? + key.len();
        Some(text[i..].trim_start_matches('"').split(|c: char| c == '"' || c.is_whitespace()).next()?.to_string())
    };
    match game {
        "ck3" => after("meta_date=")?.parse().ok(),
        "hoi4" => chronicle_bridge::hoi4_import::hoi4_date(&after("date=")?),
        _ => after("date=").and_then(|d| d.split('.').take(3).collect::<Vec<_>>().join(".").parse().ok()),
    }
}

fn readable_save(path: &Path, game: &str) -> bool {
    use std::io::Read;
    let mut head = [0u8; 8];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    if f.read_exact(&mut head).is_err() {
        return false;
    }
    match game {
        "ck3" => head.starts_with(b"SAV"),
        "hoi4" => head.starts_with(b"HOI4txt"),
        "victoria3" => head.starts_with(b"SAV"),
        _ => true,
    }
}

fn save_extensions(registry: &GameRegistry, game: &str) -> Vec<String> {
    registry.game(game).map(|g| g.save_extensions.clone()).unwrap_or_default()
}

/// "Recommended" mode: is the campaign ready for its next transition?
pub fn readiness(db: &CampaignDb, ctx: &Context<'_>) -> Result<Readiness, DbError> {
    let current = db.summary()?.current_game;
    let Some(route) = route_from(ctx.registry, &current) else {
        return Ok(Readiness { route: None, newest_save: None, save_date: None, transition_date: None, reached: false, support: "none".into() });
    };
    let tdate = transition_date(db, ctx.registry, &route)?;
    let newest = ctx
        .dirs
        .get(&route.from)
        .and_then(|d| d.save_dir.as_ref())
        .map(|dir| chronicle_steam::list_save_files(dir, &save_extensions(ctx.registry, &route.from), 30))
        .unwrap_or_default()
        .into_iter()
        .find(|s| readable_save(&s.path, &route.from));
    let save_date = newest.as_ref().and_then(|s| quick_save_date(&s.path, &route.from));
    let reached = matches!((save_date, tdate), (Some(s), Some(t)) if s >= t);
    let support = match route.kind {
        RouteKind::Hoi4ToStellaris => "full",
        RouteKind::Ck3ToEu5 => "partial",
        RouteKind::External => "external",
        RouteKind::Unsupported => "none",
    };
    Ok(Readiness { route: Some(route), newest_save: newest, save_date, transition_date: tdate, reached, support: support.into() })
}

// ------------------------------------------------------------------------------- pipeline

fn new_plan(route: Route, bridge: BridgeParams) -> TransitionPlan {
    TransitionPlan {
        route,
        save: None,
        save_date: None,
        transition_date: None,
        steps: STEPS.iter().map(|&id| Step { id, status: StepStatus::Pending, counts: BTreeMap::new(), note: None }).collect(),
        preview: None,
        can_generate: false,
        blocked_reason: None,
        conversion_id: None,
        snapshot_id: None,
        generated: None,
        finished: false,
        civ: None,
        bridge,
        prepared_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        converter: None,
        external_output: None,
    }
}

struct Run<'p, 'f> {
    plan: &'p mut TransitionPlan,
    on_step: &'f mut dyn FnMut(&Step),
}

impl Run<'_, '_> {
    fn set(&mut self, id: StepId, status: StepStatus, counts: &[(&str, i64)], note: Option<String>) {
        if let Some(s) = self.plan.steps.iter_mut().find(|s| s.id == id) {
            s.status = status;
            s.counts = counts.iter().map(|(k, v)| (k.to_string(), *v)).collect();
            s.note = note;
            (self.on_step)(s);
        }
    }
    /// Mark every still-pending step from `from` on as blocked with one reason.
    fn block_rest(&mut self, from: StepId, reason: &str) {
        let start = STEPS.iter().position(|s| *s == from).unwrap_or(0);
        for id in &STEPS[start..] {
            if self.plan.steps.iter().any(|s| s.id == *id && s.status == StepStatus::Pending) {
                self.set(*id, StepStatus::Blocked, &[], Some(reason.to_string()));
            }
        }
        self.plan.blocked_reason.get_or_insert_with(|| reason.to_string());
    }
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let bytes = std::fs::read(path)?;
    Ok(Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

enum Imported {
    Ck3 { realms: usize, report: chronicle_ck3::ImportReport },
    Hoi4 { world: Box<chronicle_bridge::Hoi4World>, report: chronicle_bridge::Hoi4ImportReport },
}

/// Steps 1–11. Stops at the preview: nothing is generated for the next game yet.
pub fn prepare(
    db: &mut CampaignDb,
    ctx: &Context<'_>,
    save_override: Option<PathBuf>,
    target: Option<&str>,
    on_step: &mut dyn FnMut(&Step),
) -> Result<TransitionPlan, TransitionError> {
    let summary = db.summary()?;
    let route = match target {
        Some(to) => route_to(ctx.registry, &summary.current_game, to),
        None => route_from(ctx.registry, &summary.current_game),
    }
    .ok_or_else(|| TransitionError::NoNextGame(summary.current_game.clone()))?;
    let mut plan = new_plan(route.clone(), ctx.bridge.clone());
    plan.transition_date = transition_date(db, ctx.registry, &route)?;
    let mut run = Run { plan: &mut plan, on_step };

    // 1. find the newest readable save of the current game
    let save = save_override.or_else(|| {
        ctx.dirs
            .get(&route.from)
            .and_then(|d| d.save_dir.as_ref())
            .map(|dir| chronicle_steam::list_save_files(dir, &save_extensions(ctx.registry, &route.from), 30))
            .unwrap_or_default()
            .into_iter()
            .find(|s| readable_save(&s.path, &route.from))
            .map(|s| s.path)
    });
    let Some(save) = save else {
        run.set(StepId::FindSave, StepStatus::Failed, &[], Some("no_save".into()));
        run.block_rest(StepId::CheckSave, "no_save");
        return Ok(plan);
    };
    run.plan.save = Some(save.display().to_string());
    run.set(StepId::FindSave, StepStatus::Done, &[], save.file_name().map(|n| n.to_string_lossy().into_owned()));
    if route.kind == RouteKind::External {
        prepare_external(db, ctx, &route, &save, &summary, &mut run)?;
        return Ok(plan);
    }

    // 2. read + check the save
    #[allow(clippy::type_complexity)]
    let (date, imported_world, ck3_world): (
        Option<PartialDate>,
        Option<Box<chronicle_bridge::Hoi4World>>,
        Option<Box<chronicle_ck3::Ck3World>>,
    ) = match route.from.as_str() {
        "ck3" => match chronicle_ck3::read_ck3(&save) {
            Ok(w) => (w.date, None, Some(Box::new(w))),
            Err(e) => {
                run.set(StepId::CheckSave, StepStatus::Failed, &[], Some(e.to_string()));
                run.block_rest(StepId::Backup, "save_unreadable");
                return Ok(plan);
            }
        },
        "hoi4" => match chronicle_bridge::read_hoi4(&save) {
            Ok(w) => (w.date.as_deref().and_then(chronicle_bridge::hoi4_import::hoi4_date), Some(Box::new(w)), None),
            Err(e) => {
                let note = if matches!(e, chronicle_bridge::Hoi4Error::Binary) { "hoi4_binary".to_string() } else { e.to_string() };
                run.set(StepId::CheckSave, StepStatus::Failed, &[], Some(note));
                run.block_rest(StepId::Backup, "save_unreadable");
                return Ok(plan);
            }
        },
        _ => {
            run.set(StepId::CheckSave, StepStatus::Blocked, &[], Some("source_adapter_missing".into()));
            run.block_rest(StepId::Backup, "source_adapter_missing");
            return Ok(plan);
        }
    };
    let Some(date) = date else {
        run.set(StepId::CheckSave, StepStatus::Failed, &[], Some("save_has_no_date".into()));
        run.block_rest(StepId::Backup, "save_unreadable");
        return Ok(plan);
    };
    run.plan.save_date = Some(date);
    if summary.current_date.is_some_and(|c| date < c) {
        run.set(StepId::CheckSave, StepStatus::Failed, &[], Some("older_than_campaign".into()));
        run.block_rest(StepId::Backup, "older_than_campaign");
        return Ok(plan);
    }
    let reached = run.plan.transition_date.is_none_or(|t| date >= t);
    run.set(
        StepId::CheckSave,
        if reached { StepStatus::Done } else { StepStatus::Warning },
        &[("year", i64::from(date.year))],
        (!reached).then(|| "transition_date_not_reached".to_string()),
    );

    // 3. backup of campaign.db (the save is copied read-only by the importer)
    let backup = db.backup(BackupKind::Transition, &format!("before {} to {}", route.from, route.to))?;
    run.set(StepId::Backup, StepStatus::Done, &[], Some(backup.display().to_string()));

    // 4. import the final state
    let imported = match route.from.as_str() {
        "ck3" => {
            let world = ck3_world.expect("ck3 world read in step 2");
            match chronicle_ck3::import_world(db, &world, &save) {
                Ok(r) => Imported::Ck3 { realms: world.realms.len(), report: r },
                Err(e) => {
                    run.set(StepId::ImportFinal, StepStatus::Failed, &[], Some(e.to_string()));
                    run.block_rest(StepId::FinalSnapshot, "import_failed");
                    return Ok(plan);
                }
            }
        }
        _ => {
            let world = imported_world.expect("hoi4 world read in step 2");
            match chronicle_bridge::import_hoi4(db, &world, &save) {
                Ok(r) => Imported::Hoi4 { world, report: r },
                Err(e) => {
                    run.set(StepId::ImportFinal, StepStatus::Failed, &[], Some(e.to_string()));
                    run.block_rest(StepId::FinalSnapshot, "import_failed");
                    return Ok(plan);
                }
            }
        }
    };
    let (warnings, semantic_values, changed, events, source_countries) = match &imported {
        Imported::Ck3 { realms, report } => {
            run.set(
                StepId::ImportFinal,
                if report.warnings.is_empty() { StepStatus::Done } else { StepStatus::Warning },
                &[
                    ("countries", *realms as i64),
                    ("territories", report.counties as i64),
                    ("rulers", report.rulers as i64),
                    ("dynasties", report.dynasties as i64),
                    ("cultures", report.cultures as i64),
                    ("faiths", report.faiths as i64),
                ],
                report.warnings.first().cloned(),
            );
            (report.warnings.clone(), report.semantic_values, report.territories_changed, report.events_added, *realms)
        }
        Imported::Hoi4 { report, .. } => {
            run.set(
                StepId::ImportFinal,
                if report.warnings.is_empty() { StepStatus::Done } else { StepStatus::Warning },
                &[("countries", report.countries as i64), ("territories", report.states as i64), ("wars", report.wars as i64)],
                report.warnings.first().cloned(),
            );
            (report.warnings.clone(), report.semantic_values, report.territories_changed, report.events_added, report.countries)
        }
    };

    // 5. final snapshot of this era
    let sha = sha256_file(&save)?;
    let stored = match &imported {
        Imported::Ck3 { report, .. } => report.stored_copy.clone(),
        Imported::Hoi4 { report, .. } => report.stored_copy.clone(),
    };
    let snapshot = db.record_snapshot(&route.from, date, "final", &stored, &sha, None)?;
    run.plan.snapshot_id = Some(snapshot);
    run.set(StepId::FinalSnapshot, StepStatus::Done, &[("snapshot", snapshot)], None);

    // 6. history: compared with the previous save of this game
    run.set(StepId::DiffHistory, StepStatus::Done, &[("territories_changed", changed as i64), ("events_added", events as i64)], None);

    // 7. semantic values
    let pending = db.pending_review_count(date)?;
    run.set(
        StepId::Semantics,
        if pending > 0 { StepStatus::Warning } else { StepStatus::Done },
        &[("values", semantic_values as i64), ("pending_review", pending)],
        None,
    );

    let adapters = serde_json::json!({ "chronicle": chronicle_core::CHRONICLE_VERSION, "source": route.from, "target": route.to });
    match route.kind {
        RouteKind::Hoi4ToStellaris => {
            let Imported::Hoi4 { world, report } = &imported else { unreachable!("hoi4 route") };
            // 8–10
            run.set(StepId::GeoCore, StepStatus::Skipped, &[], Some("one_planet".into()));
            let civ = chronicle_bridge::to_civilization(world, chronicle_bridge::Hoi4Mapping::builtin());
            run.set(
                StepId::Coefficients,
                StepStatus::Done,
                &[("indicators_from_save", civ.state.coverage().0 as i64), ("indicators", civ.state.coverage().1 as i64)],
                Some("config/bridge.yaml + games/hoi4/semantic_mapping.yaml".into()),
            );
            let start = ctx.bridge.start_year.unwrap_or(date.year).clamp(chronicle_bridge::MIN_START_YEAR, 2190);
            let result: BridgeResult = match chronicle_bridge::run_from(&civ.state, ctx.bridge.seed, Some(start)) {
                Ok(r) => r,
                Err(e) => {
                    run.set(StepId::BuildWorld, StepStatus::Failed, &[], Some(e.to_string()));
                    run.block_rest(StepId::Preview, "build_failed");
                    return Ok(plan);
                }
            };
            run.set(
                StepId::BuildWorld,
                StepStatus::Done,
                &[("from_year", i64::from(start)), ("simulated_events", result.run.events.len() as i64)],
                None,
            );
            // 11. preview
            let low = result.design.decisions.iter().filter(|d| d.review_state != "auto").count();
            let mut notes: Vec<String> = warnings.clone();
            if !result.design.vocabulary_verified {
                notes.push("stellaris_keys_partly_unverified".into());
            }
            let stellaris_dirs = ctx.dirs.get("stellaris").cloned().unwrap_or_default();
            let mut critical = result.problems.len();
            if stellaris_dirs.save_dir.is_none() {
                notes.push("stellaris_save_folder_unknown".into());
                critical += 1;
            }
            if stellaris_dirs.install_dir.is_none() {
                notes.push("stellaris_install_unknown_builtin_template".into());
            }
            let preview = Preview {
                source_countries: report.countries,
                target_countries: Some(1),
                merged: report.countries,
                split: 0,
                vanished: 0,
                created: 1,
                low_confidence: low,
                geocore_warnings: 0,
                critical_errors: critical,
                notes,
                design: Some(result.design.clone()),
                start_year: Some(start),
                bridge_events: result.run.events.len(),
            };
            let can = critical == 0;
            run.set(
                StepId::Preview,
                if can { StepStatus::Done } else { StepStatus::Warning },
                &[("source_countries", source_countries as i64), ("target_countries", 1), ("low_confidence", low as i64), ("critical", critical as i64)],
                None,
            );
            let conv = db.record_conversion(&route.from, &route.to, snapshot, &sha, date, &adapters, "bridge-v1", ctx.bridge.seed)?;
            for p in &result.problems {
                db.add_conversion_warning(conv, "critical", None, p)?;
            }
            run.plan.conversion_id = Some(conv);
            run.plan.preview = Some(preview);
            run.plan.civ = Some(civ.state);
            run.plan.can_generate = can;
            if !can {
                run.plan.blocked_reason = Some("critical_errors".into());
            }
        }
        RouteKind::Ck3ToEu5 => {
            run.set(StepId::GeoCore, StepStatus::Blocked, &[], Some("geocore_not_ready".into()));
            run.set(StepId::Coefficients, StepStatus::Blocked, &[], Some("eu5_adapter_research".into()));
            run.set(StepId::BuildWorld, StepStatus::Blocked, &[], Some("eu5_adapter_research".into()));
            let preview = Preview {
                source_countries,
                target_countries: None,
                low_confidence: pending as usize,
                notes: vec!["eu5_world_not_buildable_yet".into()],
                ..Default::default()
            };
            run.set(StepId::Preview, StepStatus::Blocked, &[("source_countries", source_countries as i64)], Some("eu5_adapter_research".into()));
            let conv = db.record_conversion(&route.from, &route.to, snapshot, &sha, date, &adapters, "none", ctx.bridge.seed)?;
            db.add_conversion_warning(conv, "error", None, "EU5 world cannot be built yet: GeoCore and the EU5 adapter are in development")?;
            run.plan.conversion_id = Some(conv);
            run.plan.preview = Some(preview);
            run.block_rest(StepId::Generate, "eu5_adapter_research");
        }
        // Vic3 → HoI4 is fully handled by `prepare_external` before this point.
        RouteKind::Unsupported | RouteKind::External => {
            run.block_rest(StepId::GeoCore, "target_adapter_missing");
        }
    }
    Ok(plan)
}

/// Copy the save into the campaign (read-only, content-addressed) → (stored path, sha256).
fn keep_save(db: &CampaignDb, save: &Path) -> Result<(PathBuf, String), TransitionError> {
    let sha = sha256_file(save)?;
    let name = save.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "save".into());
    let dir = db.root().join("original");
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{}_{name}", &sha[..12]));
    if !dest.exists() {
        std::fs::copy(save, &dest)?;
        let mut perm = std::fs::metadata(&dest)?.permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&dest, perm)?;
    }
    Ok((dest, sha))
}

/// Any route through an external converter. Chronicle imports the final world when it can read
/// the save (CK3, HoI4 text saves), otherwise keeps the save and records the era in the journal
/// (Victoria 3 and other binary saves), then waits for the converter's mod (`generate`).
fn prepare_external(
    db: &mut CampaignDb,
    ctx: &Context<'_>,
    route: &Route,
    save: &Path,
    summary: &chronicle_db::CampaignSummary,
    run: &mut Run<'_, '_>,
) -> Result<(), TransitionError> {
    // 2. what can be read from the save
    enum Source {
        Ck3(Box<chronicle_ck3::Ck3World>),
        Hoi4(Box<chronicle_bridge::Hoi4World>),
        Opaque { version: Option<String>, label: Option<String>, binary: bool, date: Option<PartialDate> },
    }
    let opaque_from_name = || Source::Opaque { version: None, label: None, binary: true, date: vic3::date_from_file_name(save) };
    let source = match route.from.as_str() {
        "ck3" => match chronicle_ck3::read_ck3(save) {
            Ok(w) => Source::Ck3(Box::new(w)),
            Err(_) => opaque_from_name(),
        },
        "hoi4" => match chronicle_bridge::read_hoi4(save) {
            Ok(w) => Source::Hoi4(Box::new(w)),
            Err(_) => opaque_from_name(),
        },
        _ => {
            let info = vic3::inspect(save)?;
            Source::Opaque { version: info.version, label: info.label, binary: info.binary, date: info.date_from_name }
        }
    };
    let read_date = match &source {
        Source::Ck3(w) => w.date,
        Source::Hoi4(w) => w.date.as_deref().and_then(chronicle_bridge::hoi4_import::hoi4_date),
        Source::Opaque { date, .. } => *date,
    };
    let date = read_date.or(run.plan.transition_date).or(summary.current_date).unwrap_or(PartialDate::ymd(1936, 1, 1));
    if summary.current_date.is_some_and(|c| date < c) && read_date.is_some() {
        run.set(StepId::CheckSave, StepStatus::Failed, &[], Some("older_than_campaign".into()));
        run.block_rest(StepId::Backup, "older_than_campaign");
        return Ok(());
    }
    run.plan.save_date = Some(date);
    let check_note = match &source {
        Source::Opaque { binary: true, .. } => Some("binary_converter_reads"),
        Source::Opaque { .. } => Some("text_reader_planned"),
        _ => None,
    };
    run.set(
        StepId::CheckSave,
        if check_note.is_some() { StepStatus::Warning } else { StepStatus::Done },
        &[("year", i64::from(date.year))],
        check_note.map(str::to_string),
    );

    // 3. backup
    let backup = db.backup(BackupKind::Transition, &format!("before {} to {}", route.from, route.to))?;
    run.set(StepId::Backup, StepStatus::Done, &[], Some(backup.display().to_string()));

    // 4. import what can be read
    let mut imported = false;
    match &source {
        Source::Ck3(w) => match chronicle_ck3::import_world(db, w, save) {
            Ok(r) => {
                imported = true;
                run.set(
                    StepId::ImportFinal,
                    StepStatus::Done,
                    &[("countries", r.realms as i64), ("territories", r.counties as i64), ("rulers", r.rulers as i64)],
                    None,
                );
                run.set(
                    StepId::DiffHistory,
                    StepStatus::Done,
                    &[("territories_changed", r.territories_changed as i64), ("events_added", r.events_added as i64)],
                    None,
                );
                run.set(StepId::Semantics, StepStatus::Done, &[("values", r.semantic_values as i64)], None);
                run.plan.preview.get_or_insert_with(Preview::default).source_countries = r.realms;
            }
            Err(e) => run.set(StepId::ImportFinal, StepStatus::Warning, &[], Some(e.to_string())),
        },
        Source::Hoi4(w) => match chronicle_bridge::import_hoi4(db, w, save) {
            Ok(r) => {
                imported = true;
                run.set(
                    StepId::ImportFinal,
                    StepStatus::Done,
                    &[("countries", r.countries as i64), ("territories", r.states as i64), ("wars", r.wars as i64)],
                    None,
                );
                run.set(
                    StepId::DiffHistory,
                    StepStatus::Done,
                    &[("territories_changed", r.territories_changed as i64), ("events_added", r.events_added as i64)],
                    None,
                );
                run.set(StepId::Semantics, StepStatus::Done, &[("values", r.semantic_values as i64)], None);
                run.plan.preview.get_or_insert_with(Preview::default).source_countries = r.countries;
            }
            Err(e) => run.set(StepId::ImportFinal, StepStatus::Warning, &[], Some(e.to_string())),
        },
        Source::Opaque { .. } => run.set(StepId::ImportFinal, StepStatus::Skipped, &[], Some("import_not_available".into())),
    }

    // 5. final snapshot (+ read-only copy of the save)
    let (stored, sha) = keep_save(db, save)?;
    let version = match &source {
        Source::Ck3(w) => w.version.clone(),
        Source::Hoi4(w) => w.version.clone(),
        Source::Opaque { version, .. } => version.clone(),
    };
    let stored_s = stored.display().to_string();
    let save_id = db.record_save_file(&route.from, &save.display().to_string(), &stored_s, &sha, Some(date), version.as_deref())?;
    let snapshot = db.record_snapshot(&route.from, date, "final", &stored_s, &sha, Some(save_id))?;
    run.plan.snapshot_id = Some(snapshot);
    run.set(StepId::FinalSnapshot, StepStatus::Done, &[("snapshot", snapshot)], None);

    // 6–7 for saves Chronicle cannot read: the era still gets its place in the journal
    if !imported {
        let key = format!("{}:final:{sha}", route.from);
        let mut added = 0;
        if !db.event_exists("world_snapshot", date, &key)? {
            let (label, binary) = match &source {
                Source::Opaque { label, binary, .. } => (label.clone(), *binary),
                _ => (None, true),
            };
            db.add_event(&NewEvent {
                game_version: version.as_deref(),
                payload: serde_json::json!({
                    "key": key, "save_label": label, "game_version": version, "binary": binary,
                    "date_source": if read_date.is_some() { "save" } else { "transition date" },
                }),
                importance: 4,
                ..NewEvent::simple(date, &route.from, "world_snapshot", EventOrigin::Save)
            })?;
            added += 1;
        }
        run.set(StepId::DiffHistory, StepStatus::Skipped, &[("events_added", added)], Some("needs_import".into()));
        run.set(StepId::Semantics, StepStatus::Skipped, &[], Some("needs_import".into()));
    }
    run.set(StepId::GeoCore, StepStatus::Skipped, &[], Some("done_by_external_converter".into()));
    run.set(StepId::Coefficients, StepStatus::Skipped, &[], Some("done_by_external_converter".into()));

    // 10–11
    let converter = ctx.converters.get(&format!("{}->{}", route.from, route.to)).map(|p| p.display().to_string());
    let target_known = ctx.dirs.get(&route.to).and_then(|d| d.save_dir.as_ref()).is_some();
    run.plan.converter = converter.clone();
    run.set(
        StepId::BuildWorld,
        if converter.is_some() { StepStatus::Warning } else { StepStatus::Blocked },
        &[],
        Some(if converter.is_some() { "run_external_converter" } else { "converter_not_set" }.into()),
    );
    let preview = run.plan.preview.get_or_insert_with(Preview::default);
    preview.notes.push("external_converter_builds_world".into());
    if !imported {
        preview.notes.push("history_not_imported_binary".into());
    }
    if !route.skipped.is_empty() {
        preview.notes.push("eras_skipped".into());
    }
    if !target_known {
        preview.notes.push("target_save_folder_unknown".into());
        preview.critical_errors += 1;
    }
    let critical = preview.critical_errors;
    run.set(
        StepId::Preview,
        if critical == 0 { StepStatus::Done } else { StepStatus::Warning },
        &[("critical", critical as i64), ("skipped_eras", route.skipped.len() as i64)],
        None,
    );
    let adapters = serde_json::json!({
        "chronicle": chronicle_core::CHRONICLE_VERSION, "source": route.from, "target": route.to,
        "external_converter": converter, "source_version": version, "skipped": route.skipped,
    });
    let conv = db.record_conversion(&route.from, &route.to, snapshot, &sha, date, &adapters, "external", 0)?;
    run.plan.conversion_id = Some(conv);
    run.plan.can_generate = critical == 0;
    if critical > 0 {
        run.plan.blocked_reason = Some("target_save_folder_unknown".into());
    }
    Ok(())
}

/// Newest `*.mod` descriptor in the target game's mod folder written after `since` (unix seconds).
fn newest_mod_since(mod_dir: &Path, since: u64) -> Option<PathBuf> {
    std::fs::read_dir(mod_dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("mod")))
        .filter_map(|e| {
            let t = e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
            (t + 120 >= since).then_some((t, e.path()))
        })
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// Steps 12–15 for an external converter: find its HoI4 mod, check it, close the era.
fn generate_external(db: &mut CampaignDb, ctx: &Context<'_>, run: &mut Run<'_, '_>) -> Result<(), TransitionError> {
    let route = run.plan.route.clone();
    let date = run.plan.save_date.unwrap_or(PartialDate::ymd(1936, 1, 1));
    let docs = ctx
        .dirs
        .get(&route.to)
        .and_then(|d| d.save_dir.as_ref())
        .map(|d| chronicle_steam::docs_dir(d))
        .ok_or_else(|| TransitionError::NotReady("target_save_folder_unknown".into()))?;
    let Some(descriptor) = newest_mod_since(&docs.join("mod"), run.plan.prepared_at) else {
        run.set(StepId::Generate, StepStatus::Failed, &[], Some("converter_output_not_found".into()));
        run.block_rest(StepId::Validate, "converter_output_not_found");
        return Ok(());
    };
    run.set(StepId::Generate, StepStatus::Done, &[], descriptor.file_name().map(|n| n.to_string_lossy().into_owned()));

    // 13. the descriptor points to a folder with HoI4 content
    let text = std::fs::read_to_string(&descriptor).unwrap_or_default();
    let folder = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("path").map(|r| r.trim_start_matches([' ', '=']).trim().trim_matches('"').to_string()))
        .map(PathBuf::from)
        .map(|p| if p.is_absolute() { p } else { docs.join(p) });
    let valid = folder.as_ref().is_some_and(|f| f.join("history").is_dir() || f.join("common").is_dir());
    run.set(StepId::Validate, if valid { StepStatus::Done } else { StepStatus::Failed }, &[], folder.as_ref().map(|f| f.display().to_string()));
    if !valid {
        run.block_rest(StepId::CloseEra, "validation_failed");
        return Ok(());
    }
    run.plan.external_output = Some(descriptor.display().to_string());

    // 14–15
    let key = format!("era:{}:{}:{}", route.from, route.to, date);
    if !db.event_exists("era_transition", date, &key)? {
        db.add_event(&NewEvent {
            payload: serde_json::json!({ "key": key, "from": route.from, "to": route.to, "converter": run.plan.converter,
                                         "hoi4_mod": descriptor.display().to_string() }),
            importance: 5,
            ..NewEvent::simple(date, &route.from, "era_transition", EventOrigin::Converter)
        })?;
    }
    db.set_era(&route.from, "completed", date, run.plan.snapshot_id, run.plan.conversion_id)?;
    if let Some(c) = run.plan.conversion_id {
        db.set_conversion_status(c, "generated")?;
    }
    run.set(StepId::CloseEra, StepStatus::Done, &[], None);
    // games jumped over: kept in the chain as skipped, with a journal entry each
    for g in &route.skipped {
        db.set_era(g, "skipped", date, None, run.plan.conversion_id)?;
        let key = format!("era:{}:skipped:{}", g, run.plan.conversion_id.unwrap_or(0));
        if !db.event_exists("era_skipped", date, &key)? {
            db.add_event(&NewEvent {
                payload: serde_json::json!({ "key": key, "from": route.from, "to": route.to }),
                importance: 3,
                ..NewEvent::simple(date, g, "era_skipped", EventOrigin::Converter)
            })?;
        }
    }
    let start = ctx.registry.game(&route.to).map(|g| g.era.start).unwrap_or(PartialDate::ymd(1936, 1, 1));
    let start = if date > start { date } else { start };
    db.set_era(&route.to, "active", start, None, run.plan.conversion_id)?;
    db.set_current(&route.to, start)?;
    run.set(StepId::NextEra, StepStatus::Done, &[], Some(route.to.clone()));
    run.plan.finished = true;
    Ok(())
}

/// Write the simulated bridge history into the journal (idempotent per seed and start year).
pub fn write_bridge_history(db: &CampaignDb, r: &BridgeResult) -> Result<usize, DbError> {
    let step = chronicle_bridge::BridgeConfig::builtin().step_years;
    let mut added = 0;
    for e in &r.run.events {
        let key = format!("bridge:{}:{}:{}:{}", r.seed, r.start_year, e.year, e.kind);
        if db.event_exists(&e.kind, PartialDate::year(e.year), &key)? {
            continue;
        }
        let importance = match e.kind.as_str() {
            "nuclear_war" | "ecological_collapse" | "earth_unified" => 5,
            "conquest" | "space_interstellar" => 4,
            _ => 3,
        };
        db.add_event(&NewEvent {
            date_to: Some(PartialDate::year(e.year + step - 1)),
            game_version: Some(chronicle_core::CHRONICLE_VERSION),
            payload: serde_json::json!({ "key": key, "simulated": true, "seed": r.seed, "start_year": r.start_year }),
            importance,
            ..NewEvent::simple(PartialDate::year(e.year), "modern_bridge", &e.kind, EventOrigin::Converter)
        })?;
        added += 1;
    }
    let key = format!("bridge:{}:{}:design", r.seed, r.start_year);
    if !db.event_exists("stellaris_empire_designed", PartialDate::ymd(2200, 1, 1), &key)? {
        db.add_event(&NewEvent {
            payload: serde_json::json!({ "key": key, "simulated": true, "seed": r.seed, "design": &r.design }),
            importance: 5,
            ..NewEvent::simple(PartialDate::ymd(2200, 1, 1), "stellaris", "stellaris_empire_designed", EventOrigin::Converter)
        })?;
        added += 1;
    }
    Ok(added)
}

/// Steps 12–15, after the player confirmed the preview.
pub fn generate(
    db: &mut CampaignDb,
    ctx: &Context<'_>,
    plan: &TransitionPlan,
    on_step: &mut dyn FnMut(&Step),
) -> Result<TransitionPlan, TransitionError> {
    if !plan.can_generate || !matches!(plan.route.kind, RouteKind::Hoi4ToStellaris | RouteKind::External) {
        return Err(TransitionError::NotReady(plan.blocked_reason.clone().unwrap_or_else(|| "route not supported".into())));
    }
    if plan.route.kind == RouteKind::External {
        let mut plan = plan.clone();
        let mut run = Run { plan: &mut plan, on_step };
        generate_external(db, ctx, &mut run)?;
        return Ok(plan);
    }
    let civ = plan.civ.clone().ok_or_else(|| TransitionError::NotReady("no civilization state".into()))?;
    let date = plan.save_date.ok_or_else(|| TransitionError::NotReady("save has no date".into()))?;
    let mut plan = plan.clone();
    let mut run = Run { plan: &mut plan, on_step };

    // 12. add Earth's empire to the player's own Stellaris empire designs (no mod needed)
    let start = run.plan.preview.as_ref().and_then(|p| p.start_year);
    let result = chronicle_bridge::run_from(&civ, run.plan.bridge.seed, start).map_err(|e| TransitionError::NotReady(e.to_string()))?;
    let dirs = ctx.dirs.get("stellaris").cloned().unwrap_or_default();
    let save_dir = dirs.save_dir.clone().ok_or_else(|| TransitionError::NotReady("stellaris_save_folder_unknown".into()))?;
    let docs = chronicle_bridge::stellaris_docs_from_save_dir(&save_dir);
    let (default_name, _) = chronicle_bridge::suggested_name(&result.design);
    let empire_name =
        if run.plan.bridge.empire_name.trim().is_empty() { default_name } else { run.plan.bridge.empire_name.trim().to_string() };
    let adjective = if run.plan.bridge.adjective.trim().is_empty() { "Terran".to_string() } else { run.plan.bridge.adjective.trim().to_string() };
    let written = match chronicle_bridge::write_user_design(&docs, &result.design, chronicle_bridge::Vocabulary::builtin(), &empire_name, &adjective) {
        Ok(w) => w,
        Err(e) => {
            run.set(StepId::Generate, StepStatus::Failed, &[], Some(e.to_string()));
            run.block_rest(StepId::Validate, "generate_failed");
            return Ok(plan);
        }
    };
    run.set(
        StepId::Generate,
        if written.warnings.is_empty() { StepStatus::Done } else { StepStatus::Warning },
        &[],
        Some(written.file.display().to_string()),
    );

    // 13. validate: the designs file parses and contains our empire with 3 ethic points
    let valid = std::fs::read(&written.file)
        .ok()
        .and_then(|b| {
            let body = b.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&b);
            let p = paradox_parser::parse(body).ok()?;
            let block = p.root.get(&empire_name)?.as_container()?;
            let ethic_points: usize = block
                .get_all("ethic")
                .filter_map(|v| v.as_scalar())
                .map(|s| if s.as_str().contains("fanatic") { 2 } else { 1 })
                .sum();
            Some(block.get("authority").is_some() && block.get("government").is_some() && ethic_points == 3)
        })
        .unwrap_or(false);
    run.set(StepId::Validate, if valid { StepStatus::Done } else { StepStatus::Failed }, &[], None);
    if !valid {
        run.block_rest(StepId::CloseEra, "validation_failed");
        return Ok(plan);
    }

    // 14. close the era (history stays; the save copy stays)
    let added = write_bridge_history(db, &result)?;
    let key = format!("era:{}:{}:{}", run.plan.route.from, run.plan.route.to, date);
    if !db.event_exists("era_transition", date, &key)? {
        db.add_event(&NewEvent {
            payload: serde_json::json!({
                "key": key, "from": run.plan.route.from, "to": run.plan.route.to, "via": run.plan.route.via,
                "designs_file": written.file.display().to_string(), "empire": empire_name,
            }),
            importance: 5,
            ..NewEvent::simple(date, &run.plan.route.from, "era_transition", EventOrigin::Converter)
        })?;
    }
    db.set_era(&run.plan.route.from, "completed", date, run.plan.snapshot_id, run.plan.conversion_id)?;
    if let Some(via) = &run.plan.route.via {
        db.set_era(via, "completed", PartialDate::ymd(2200, 1, 1), None, run.plan.conversion_id)?;
    }
    if let Some(c) = run.plan.conversion_id {
        db.set_conversion_status(c, "generated")?;
    }
    run.set(StepId::CloseEra, StepStatus::Done, &[("journal_events", added as i64 + 1)], None);

    // 15. the next game becomes the current era
    let start = PartialDate::ymd(2200, 1, 1);
    db.set_era(&run.plan.route.to, "active", start, None, run.plan.conversion_id)?;
    db.set_current(&run.plan.route.to, start)?;
    run.set(StepId::NextEra, StepStatus::Done, &[], Some(run.plan.route.to.clone()));
    run.plan.generated = Some(written);
    run.plan.finished = true;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronicle_core::CampaignSettings;

    const HOI4: &[u8] = include_bytes!("../../../../tests/fixtures/hoi4/1.19.3/mini_save.hoi4.txt");
    const CK3: &[u8] = include_bytes!("../../../../tests/fixtures/ck3/1.0.2/mini_gamestate.txt");

    fn campaign(start_game: &str) -> (tempfile::TempDir, CampaignDb, GameRegistry) {
        let tmp = tempfile::tempdir().unwrap();
        let reg = GameRegistry::builtin().unwrap();
        let db = CampaignDb::create(&tmp.path().join("camp"), "T", &CampaignSettings::new(start_game, 1), &reg).unwrap();
        (tmp, db, reg)
    }

    #[test]
    fn routes_follow_the_chain() {
        let reg = GameRegistry::builtin().unwrap();
        let r = route_from(&reg, "hoi4").unwrap();
        assert_eq!((r.to.as_str(), r.via.as_deref(), r.kind), ("stellaris", Some("modern_bridge"), RouteKind::Hoi4ToStellaris));
        assert_eq!(route_from(&reg, "ck3").unwrap().kind, RouteKind::Ck3ToEu5);
        assert_eq!(route_from(&reg, "eu5").unwrap().kind, RouteKind::External);
        assert!(route_from(&reg, "stellaris").is_none());
    }

    #[test]
    fn hoi4_to_stellaris_end_to_end() {
        let (tmp, mut db, reg) = campaign("hoi4");
        let hoi4_saves = tmp.path().join("hoi4");
        std::fs::create_dir_all(&hoi4_saves).unwrap();
        std::fs::write(hoi4_saves.join("autosave.hoi4"), HOI4).unwrap();
        let stellaris_saves = tmp.path().join("Stellaris/save games");
        std::fs::create_dir_all(&stellaris_saves).unwrap();
        let mut dirs = BTreeMap::new();
        dirs.insert("hoi4".to_string(), GameDirs { save_dir: Some(hoi4_saves), install_dir: None });
        dirs.insert("stellaris".to_string(), GameDirs { save_dir: Some(stellaris_saves), install_dir: None });
        let ctx = Context { registry: &reg, dirs, bridge: BridgeParams { empire_name: "Terran Union".into(), ..Default::default() }, converters: BTreeMap::new() };

        let ready = readiness(&db, &ctx).unwrap();
        assert_eq!(ready.save_date, Some(PartialDate::ymd(1940, 1, 3)));
        assert!(!ready.reached, "default HoI4 → bridge date is 1948");

        let mut seen = Vec::new();
        let plan = prepare(&mut db, &ctx, None, None, &mut |s| seen.push(s.id)).unwrap();
        let status = |id| plan.steps.iter().find(|s| s.id == id).unwrap().status;
        assert_eq!(status(StepId::CheckSave), StepStatus::Warning, "1940 < 1948: allowed in manual mode, with a warning");
        assert_eq!(status(StepId::ImportFinal), StepStatus::Done);
        assert_eq!(status(StepId::GeoCore), StepStatus::Skipped);
        assert_eq!(status(StepId::Preview), StepStatus::Done);
        assert!(plan.can_generate);
        let p = plan.preview.as_ref().unwrap();
        assert_eq!((p.source_countries, p.target_countries, p.critical_errors), (5, Some(1), 0));
        assert_eq!(p.start_year, Some(1940));
        assert!(seen.contains(&StepId::BuildWorld));

        let done = generate(&mut db, &ctx, &plan, &mut |_| {}).unwrap();
        assert!(done.finished);
        assert!(done.steps.iter().all(|s| matches!(s.status, StepStatus::Done | StepStatus::Skipped | StepStatus::Warning)), "{:?}", done.steps);
        let written = done.generated.as_ref().unwrap();
        assert!(written.file.exists() && written.file.ends_with("Stellaris/user_empire_designs_v3.4.txt"));
        assert!(std::fs::read_to_string(&written.file).unwrap().contains("\"Terran Union\"="));
        let s = db.summary().unwrap();
        assert_eq!((s.current_game.as_str(), s.current_date), ("stellaris", Some(PartialDate::ymd(2200, 1, 1))));
        let eras = db.eras().unwrap();
        assert!(eras.contains(&("hoi4".to_string(), "completed".to_string())));
        assert!(eras.contains(&("stellaris".to_string(), "active".to_string())));
        assert!(db.events(0, 1000).unwrap().iter().any(|e| e.event_type == "era_transition"));
        // the old era's data is still there
        assert!(db.events_for(0, Some("hoi4"), 1000).unwrap().iter().any(|e| e.event_type == "war_started"));
    }

    #[test]
    fn ck3_to_eu5_imports_but_stops_before_building_eu5() {
        let (tmp, mut db, reg) = campaign("ck3");
        let dir = tmp.path().join("ck3");
        std::fs::create_dir_all(&dir).unwrap();
        let mut file = b"SAV0102e36d7ab200000000\n".to_vec();
        file.extend_from_slice(CK3);
        std::fs::write(dir.join("autosave.ck3"), &file).unwrap();
        let mut dirs = BTreeMap::new();
        dirs.insert("ck3".to_string(), GameDirs { save_dir: Some(dir), install_dir: None });
        let ctx = Context { registry: &reg, dirs, bridge: BridgeParams::default(), converters: BTreeMap::new() };
        let plan = prepare(&mut db, &ctx, None, None, &mut |_| {}).unwrap();
        let status = |id| plan.steps.iter().find(|s| s.id == id).unwrap().status;
        assert_eq!(status(StepId::ImportFinal), StepStatus::Done);
        assert_eq!(status(StepId::FinalSnapshot), StepStatus::Done);
        assert_eq!(status(StepId::GeoCore), StepStatus::Blocked);
        assert_eq!(status(StepId::Generate), StepStatus::Blocked);
        assert!(!plan.can_generate);
        assert_eq!(plan.blocked_reason.as_deref(), Some("eu5_adapter_research"));
        assert!(generate(&mut db, &ctx, &plan, &mut |_| {}).is_err());
        assert_eq!(db.summary().unwrap().current_game, "ck3", "nothing changes without a buildable target");
    }

    #[test]
    fn vic3_to_hoi4_through_an_external_converter() {
        let (tmp, mut db, reg) = campaign("victoria3");
        let vic3_saves = tmp.path().join("Victoria 3/save games");
        std::fs::create_dir_all(&vic3_saves).unwrap();
        let mut save = b"SAV0103da6d73a800000296\n".to_vec();
        save.extend_from_slice(b"\xadU\x01\x00\x0f\x00\x06\x001.5.12E2\x01\x00\x0f\x00\x17\x00United States of Europe\x01\x00PK\x03\x04zip");
        std::fs::write(vic3_saves.join("prussia_1885_04_13.v3"), &save).unwrap();
        let hoi4_docs = tmp.path().join("Hearts of Iron IV");
        std::fs::create_dir_all(hoi4_docs.join("save games")).unwrap();
        let mut dirs = BTreeMap::new();
        dirs.insert("victoria3".to_string(), GameDirs { save_dir: Some(vic3_saves), install_dir: None });
        dirs.insert("hoi4".to_string(), GameDirs { save_dir: Some(hoi4_docs.join("save games")), install_dir: None });
        let mut converters = BTreeMap::new();
        converters.insert("victoria3->hoi4".to_string(), PathBuf::from("C:/Converters/Vic3ToHoI4/ConverterFrontend.exe"));
        let ctx = Context { registry: &reg, dirs, bridge: BridgeParams::default(), converters };

        let plan = prepare(&mut db, &ctx, None, None, &mut |_| {}).unwrap();
        assert_eq!(plan.route.kind, RouteKind::External);
        let status = |p: &TransitionPlan, id| p.steps.iter().find(|s| s.id == id).unwrap().status;
        assert_eq!(status(&plan, StepId::FinalSnapshot), StepStatus::Done);
        assert_eq!(status(&plan, StepId::BuildWorld), StepStatus::Warning, "waiting for the converter");
        assert_eq!(plan.save_date, Some(PartialDate::ymd(1885, 4, 13)), "date from the file name");
        assert!(plan.can_generate);
        let ev = db.events_for(0, Some("victoria3"), 100).unwrap();
        assert_eq!(ev.iter().filter(|e| e.event_type == "world_snapshot").count(), 1, "Victoria 3 appears in the journal");

        // no converter output yet → step 12 fails, nothing changes
        let early = generate(&mut db, &ctx, &plan, &mut |_| {}).unwrap();
        assert_eq!(status(&early, StepId::Generate), StepStatus::Failed);
        assert_eq!(db.summary().unwrap().current_game, "victoria3");

        // the converter writes its mod
        let mod_dir = hoi4_docs.join("mod");
        std::fs::create_dir_all(mod_dir.join("prussia/common")).unwrap();
        std::fs::write(mod_dir.join("prussia.mod"), "name = \"Converted - prussia\"\npath = \"mod/prussia/\"\n").unwrap();
        let done = generate(&mut db, &ctx, &plan, &mut |_| {}).unwrap();
        assert!(done.finished, "{:?}", done.steps);
        assert!(done.external_output.as_deref().unwrap().ends_with("prussia.mod"));
        let s = db.summary().unwrap();
        assert_eq!((s.current_game.as_str(), s.current_date), ("hoi4", Some(PartialDate::ymd(1936, 1, 1))));
        assert!(db.eras().unwrap().contains(&("victoria3".to_string(), "completed".to_string())));
    }

    #[test]
    fn any_later_game_is_a_target() {
        let reg = GameRegistry::builtin().unwrap();
        let t = targets(&reg, "ck3");
        let to: Vec<&str> = t.iter().map(|r| r.to.as_str()).collect();
        assert_eq!(to, ["eu5", "victoria3", "hoi4", "stellaris"], "the bridge is simulated, never a target");
        let hoi = t.iter().find(|r| r.to == "hoi4").unwrap();
        assert_eq!((hoi.kind, hoi.skipped.clone()), (RouteKind::External, vec!["eu5".to_string(), "victoria3".to_string()]));
        assert!(route_to(&reg, "hoi4", "ck3").is_none(), "history only moves forward");
        assert!(route_to(&reg, "hoi4", "modern_bridge").is_none());
    }

    #[test]
    fn ck3_to_hoi4_imports_ck3_and_marks_skipped_eras() {
        let (tmp, mut db, reg) = campaign("ck3");
        let ck3_dir = tmp.path().join("ck3");
        std::fs::create_dir_all(&ck3_dir).unwrap();
        let mut file = b"SAV0102e36d7ab200000000\n".to_vec();
        file.extend_from_slice(CK3);
        std::fs::write(ck3_dir.join("final.ck3"), &file).unwrap();
        let hoi4_docs = tmp.path().join("Hearts of Iron IV");
        std::fs::create_dir_all(hoi4_docs.join("save games")).unwrap();
        let mut dirs = BTreeMap::new();
        dirs.insert("ck3".to_string(), GameDirs { save_dir: Some(ck3_dir), install_dir: None });
        dirs.insert("hoi4".to_string(), GameDirs { save_dir: Some(hoi4_docs.join("save games")), install_dir: None });
        let ctx = Context { registry: &reg, dirs, bridge: BridgeParams::default(), converters: BTreeMap::new() };

        let plan = prepare(&mut db, &ctx, None, Some("hoi4"), &mut |_| {}).unwrap();
        assert_eq!(plan.route.kind, RouteKind::External);
        let status = |p: &TransitionPlan, id| p.steps.iter().find(|s| s.id == id).unwrap().status;
        assert_eq!(status(&plan, StepId::ImportFinal), StepStatus::Done, "CK3 is readable: its final world is imported");
        assert_eq!(status(&plan, StepId::BuildWorld), StepStatus::Blocked, "no converter chosen yet");
        assert!(plan.can_generate, "the converter can still be run by hand");

        let mod_dir = hoi4_docs.join("mod");
        std::fs::create_dir_all(mod_dir.join("converted/history")).unwrap();
        std::fs::write(mod_dir.join("converted.mod"), "name=\"Converted\"\npath=\"mod/converted\"\n").unwrap();
        let done = generate(&mut db, &ctx, &plan, &mut |_| {}).unwrap();
        assert!(done.finished, "{:?}", done.steps);
        let s = db.summary().unwrap();
        assert_eq!(s.current_game, "hoi4");
        assert_eq!(s.eras.get("eu5").map(String::as_str), Some("skipped"));
        assert_eq!(s.eras.get("victoria3").map(String::as_str), Some("skipped"));
        assert_eq!(s.eras.get("ck3").map(String::as_str), Some("completed"));
        assert!(db.events_for(0, Some("victoria3"), 10).unwrap().iter().any(|e| e.event_type == "era_skipped"));
    }

    #[test]
    fn no_save_is_reported_not_crashed() {
        let (_tmp, mut db, reg) = campaign("hoi4");
        let ctx = Context { registry: &reg, dirs: BTreeMap::new(), bridge: BridgeParams::default(), converters: BTreeMap::new() };
        let plan = prepare(&mut db, &ctx, None, None, &mut |_| {}).unwrap();
        assert_eq!(plan.steps[0].status, StepStatus::Failed);
        assert_eq!(plan.blocked_reason.as_deref(), Some("no_save"));
    }
}
