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
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub from: String,
    pub to: String,
    /// Intermediate stage that is simulated, not played (the Modern Era Bridge).
    pub via: Option<String>,
    pub kind: RouteKind,
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

pub fn route_from(registry: &GameRegistry, current: &str) -> Option<Route> {
    let chain = &registry.chain;
    let i = chain.iter().position(|g| g == current)?;
    let next = chain.get(i + 1)?.clone();
    let (to, via) = if next == "modern_bridge" { (chain.get(i + 2)?.clone(), Some(next)) } else { (next, None) };
    let kind = match (current, to.as_str()) {
        ("hoi4", "stellaris") => RouteKind::Hoi4ToStellaris,
        ("ck3", "eu5") => RouteKind::Ck3ToEu5,
        _ => RouteKind::Unsupported,
    };
    Some(Route { from: current.to_string(), to, via, kind })
}

/// The transition date for the first leg (settings first, then the registry default).
pub fn transition_date(db: &CampaignDb, registry: &GameRegistry, route: &Route) -> Result<Option<PartialDate>, DbError> {
    let next = route.via.clone().unwrap_or_else(|| route.to.clone());
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
    on_step: &mut dyn FnMut(&Step),
) -> Result<TransitionPlan, TransitionError> {
    let summary = db.summary()?;
    let route = route_from(ctx.registry, &summary.current_game).ok_or_else(|| TransitionError::NoNextGame(summary.current_game.clone()))?;
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
        RouteKind::Unsupported => {
            run.block_rest(StepId::GeoCore, "target_adapter_missing");
        }
    }
    Ok(plan)
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
    if !plan.can_generate || plan.route.kind != RouteKind::Hoi4ToStellaris {
        return Err(TransitionError::NotReady(plan.blocked_reason.clone().unwrap_or_else(|| "route not supported".into())));
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
        assert_eq!(route_from(&reg, "eu5").unwrap().kind, RouteKind::Unsupported);
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
        let ctx = Context { registry: &reg, dirs, bridge: BridgeParams { empire_name: "Terran Union".into(), ..Default::default() } };

        let ready = readiness(&db, &ctx).unwrap();
        assert_eq!(ready.save_date, Some(PartialDate::ymd(1940, 1, 3)));
        assert!(!ready.reached, "default HoI4 → bridge date is 1948");

        let mut seen = Vec::new();
        let plan = prepare(&mut db, &ctx, None, &mut |s| seen.push(s.id)).unwrap();
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
        let ctx = Context { registry: &reg, dirs, bridge: BridgeParams::default() };
        let plan = prepare(&mut db, &ctx, None, &mut |_| {}).unwrap();
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
    fn no_save_is_reported_not_crashed() {
        let (_tmp, mut db, reg) = campaign("hoi4");
        let ctx = Context { registry: &reg, dirs: BTreeMap::new(), bridge: BridgeParams::default() };
        let plan = prepare(&mut db, &ctx, None, &mut |_| {}).unwrap();
        assert_eq!(plan.steps[0].status, StepStatus::Failed);
        assert_eq!(plan.blocked_reason.as_deref(), Some("no_save"));
    }
}
