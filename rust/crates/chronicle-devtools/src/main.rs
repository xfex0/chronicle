//! chronicle-dev — test everything from a terminal.
//!
//!   cargo run -p chronicle-devtools -- demo ./demo-campaign
//!   cargo run -p chronicle-devtools -- check ./demo-campaign
//!   cargo run -p chronicle-devtools -- owner ./demo-campaign Halych 1250
//!   cargo run -p chronicle-devtools -- sql ./demo-campaign "SELECT * FROM entity_periods"

use std::path::PathBuf;
use std::process::ExitCode;

use chronicle_core::registry::GameRegistry;
use chronicle_core::{CampaignSettings, PartialDate};
use chronicle_db::dev::{entity_labels, readonly_query, seed_demo, self_test, table_counts};
use chronicle_db::{Aspect, CampaignDb};

const HELP: &str = "chronicle-dev <command>

  demo <folder>                       create a demo campaign (fictional world) in a new folder
  check <folder>                      run database self-test
  tables <folder>                     row count per table
  entities <folder> [kind]            list entities (country, territory, dynasty, ...)
  owner <folder> <territory> <date>   who owned a territory at a date (name or id)
  history <folder> <territory>        ownership periods of a territory
  periods <folder> <entity> [aspect]  government / ruler / dynasty / capital periods
  events <folder>                     the journal
  sql <folder> \"<SELECT ...>\"         read-only SQL
  steam                               detect Steam, libraries and supported games
  backups <folder>                    backup register (kind, path, time)
  registry                            validated game registry and transition dates
  bridge [preset] [seed]              Modern Era Bridge 1948→2200 and the Stellaris empire design
                                      presets: global_democracy, military_dictatorship, cold_war, planned_technocracy
  stellaris <save.sav>                read the player's empire from a Stellaris save
  hoi4 <save.hoi4> [seed]             HoI4 text save → civilization indicators → Bridge → Stellaris empire
  ck3 <save.ck3>                      read a CK3 save: realms, player, largest states
  import <folder> <save.ck3>          import a CK3 save into a campaign (creates the campaign if the folder is new)";

type R = Result<(), Box<dyn std::error::Error>>;

fn open(folder: &str) -> Result<CampaignDb, Box<dyn std::error::Error>> {
    Ok(CampaignDb::open(&PathBuf::from(folder))?)
}

/// Accept a Chronicle id or a display name.
fn resolve(db: &CampaignDb, needle: &str) -> Result<String, Box<dyn std::error::Error>> {
    if needle.starts_with("chronicle_") {
        return Ok(needle.to_string());
    }
    entity_labels(db, None)?
        .into_iter()
        .find(|e| e.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(needle)))
        .map(|e| e.id)
        .ok_or_else(|| format!("no entity named '{needle}'").into())
}

fn date(s: &str) -> Result<PartialDate, Box<dyn std::error::Error>> {
    Ok(s.parse::<PartialDate>()?)
}

fn show(p: &Option<PartialDate>) -> String {
    p.map_or("now".into(), |d| d.to_string())
}

fn run(args: &[String]) -> R {
    let a = |i: usize| args.get(i).map(String::as_str).ok_or("missing argument");
    match a(0)? {
        "demo" => {
            let reg = GameRegistry::builtin()?;
            let mut db = CampaignDb::create(&PathBuf::from(a(1)?), "Demo world", &CampaignSettings::new("ck3", 45_819_283), &reg)?;
            let s = seed_demo(&mut db)?;
            println!("demo campaign created in {}\n{}", a(1)?, serde_json::to_string_pretty(&s)?);
        }
        "check" => {
            let mut failed = 0;
            for c in self_test(&open(a(1)?)?)? {
                failed += usize::from(!c.ok);
                println!("{} {:<36} {}", if c.ok { "PASS" } else { "FAIL" }, c.name, c.detail);
            }
            if failed > 0 {
                return Err(format!("{failed} checks failed").into());
            }
        }
        "tables" => {
            for (t, n) in table_counts(&open(a(1)?)?)? {
                println!("{n:>8}  {t}");
            }
        }
        "entities" => {
            for e in entity_labels(&open(a(1)?)?, args.get(2).map(String::as_str))? {
                println!("{:<30} {:<12} {}", e.id, e.kind, e.name.unwrap_or_default());
            }
        }
        "owner" => {
            let db = open(a(1)?)?;
            let t = resolve(&db, a(2)?)?;
            match db.owner_at(&t, date(a(3)?)?)? {
                Some(p) => println!("{} (from {} to {})", p.owner_id.unwrap_or("unowned".into()), p.from, show(&p.to)),
                None => println!("no owner recorded at that date"),
            }
        }
        "history" => {
            let db = open(a(1)?)?;
            for p in db.ownership_history(&resolve(&db, a(2)?)?)? {
                println!("{:>10} → {:<10} {}", p.from.to_string(), show(&p.to), p.owner_id.unwrap_or("unowned".into()));
            }
        }
        "periods" => {
            let db = open(a(1)?)?;
            let aspect = match args.get(3) {
                Some(s) => Some(Aspect::parse(s).ok_or_else(|| format!("unknown aspect {s}"))?),
                None => None,
            };
            for p in db.periods(&resolve(&db, a(2)?)?, aspect)? {
                let v = p.value.or(p.value_entity_id).unwrap_or_default();
                println!("{:<16} {:>10} → {:<10} {}", p.aspect.as_str(), p.from.to_string(), show(&p.to), v);
            }
        }
        "events" => {
            for e in open(a(1)?)?.events(0, 10_000)? {
                let when = match e.date_to {
                    Some(to) => format!("{}–{}", e.date, to),
                    None => e.date.to_string(),
                };
                println!(
                    "{:>12}  imp {}  {:<9} {:<20} {}",
                    when,
                    e.importance,
                    e.evidence,
                    e.event_type,
                    e.actor_entity_id.unwrap_or_default()
                );
            }
        }
        "sql" => {
            let q = readonly_query(&open(a(1)?)?, a(2)?, 200)?;
            println!("{}", q.columns.join(" | "));
            for r in &q.rows {
                println!("{}", r.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" | "));
            }
            if q.truncated {
                println!("… (truncated)");
            }
        }
        "steam" => {
            let reg = GameRegistry::builtin()?;
            let steam = chronicle_steam::find_steam();
            match &steam {
                Some(s) => s.log.iter().for_each(|l| println!("{l}")),
                None => println!("Steam not found"),
            }
            for d in chronicle_steam::detect_all(&reg, steam.as_ref()) {
                println!(
                    "{:<10} {:<8} {:<11} {}  saves: {}",
                    d.game_key,
                    d.source.as_str(),
                    d.fingerprint.as_str(),
                    d.install_path.map(|p| p.display().to_string()).unwrap_or("-".into()),
                    d.save_path.map(|p| p.display().to_string()).unwrap_or("-".into())
                );
            }
        }
        "backups" => {
            for (kind, path, at) in open(a(1)?)?.backups()? {
                println!("{at}  {kind:<10} {path}");
            }
        }
        "bridge" => {
            let name = args.get(1).map(String::as_str).unwrap_or("global_democracy");
            let seed: u64 = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(45_819_283);
            let start = chronicle_bridge::presets()
                .into_iter()
                .find(|(n, _)| *n == name)
                .map(|(_, s)| s)
                .ok_or_else(|| format!("unknown preset {name}"))?;
            let r = chronicle_bridge::run(&start, seed)?;
            println!("Modern Era Bridge — preset {name}, seed {seed}");
            for e in &r.run.events {
                println!("  {}  {}", e.year, e.kind);
            }
            let d = &r.design;
            println!("\nStellaris empire");
            println!("  authority: {}", d.authority);
            println!("  ethics:    {}", d.ethics.join(", "));
            println!("  civics:    {}", d.civics.join(", "));
            println!("  origin:    {}", d.origin);
            for dec in &d.decisions {
                println!(
                    "  {:<9} confidence {:.0}% ({}){}",
                    dec.part,
                    dec.confidence * 100.0,
                    dec.review_state,
                    dec.alternative.as_ref().map(|a| format!(", close call: {a}")).unwrap_or_default()
                );
            }
            if !r.problems.is_empty() {
                return Err(format!("invalid design: {:?}", r.problems).into());
            }
        }
        "stellaris" => {
            let e = chronicle_bridge::read_empire(&PathBuf::from(a(1)?))?;
            println!("{} ({}), {}, ironman: {:?}", e.name, e.game_version.unwrap_or_default(), e.date.unwrap_or_default(), e.ironman);
            println!("  authority: {}", e.raw_authority.unwrap_or_default());
            println!("  ethics:    {}", e.raw_ethics.join(", "));
            println!("  civics:    {}", e.raw_civics.join(", "));
            println!("  origin:    {}", e.raw_origin.unwrap_or_default());
            println!("  government type: {}", e.government_type.unwrap_or_default());
        }
        "hoi4" => {
            let world = chronicle_bridge::read_hoi4(&PathBuf::from(a(1)?))?;
            let civ = chronicle_bridge::to_civilization(&world, chronicle_bridge::Hoi4Mapping::builtin());
            let s = &civ.summary;
            println!("HoI4 {} — {} (player {})", s.version.clone().unwrap_or_default(), s.date.clone().unwrap_or_default(), s.player.clone().unwrap_or_default());
            println!("  world factories: {} in {} countries", s.world_factories, s.countries_with_industry);
            println!("  top powers: {}", s.top_powers.iter().map(|(t, w)| format!("{t} {:.0}%", w * 100.0)).collect::<Vec<_>>().join(", "));
            println!("  factions:   {}", s.factions.iter().map(|(n, w)| format!("{n} {:.0}%", w * 100.0)).collect::<Vec<_>>().join(", "));
            println!("  at war:     {}", s.countries_at_war.join(" "));
            println!("  indicators ({} of 14 from the save; missing: {}):", civ.state.coverage().0, s.missing.join(", "));
            for (k, v) in &civ.state.values {
                println!("    {k:<26} {:>5.1}%", v * 100.0);
            }
            println!("  blocs: {}, ideology: {:?}", civ.state.blocs, civ.state.ideology);
            let seed: u64 = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(45_819_283);
            let r = chronicle_bridge::run(&civ.state, seed)?;
            let d = &r.design;
            println!("\nStellaris empire (seed {seed}): {}; {}; {}; {}", d.authority, d.ethics.join(", "), d.civics.join(", "), d.origin);
        }
        "ck3" => {
            let w = chronicle_ck3::read_ck3(&PathBuf::from(a(1)?))?;
            println!(
                "CK3 {} — {} (bookmark {}), player: {}",
                w.version.clone().unwrap_or_default(),
                w.date.map(|d| d.to_string()).unwrap_or_default(),
                w.bookmark.map(|d| d.to_string()).unwrap_or_default(),
                w.player_name.clone().unwrap_or_default()
            );
            println!("  titles {}, counties {}, realms {}", w.titles.len(), w.counties.len(), w.realms.len());
            for r in w.realms.iter().take(12) {
                let t = &w.titles[&r.primary_title];
                let ruler = w.characters.get(&r.ruler).map(|c| c.name.clone()).unwrap_or_default();
                let mark = if w.player.as_deref() == Some(r.ruler.as_str()) { "  ← player" } else { "" };
                println!("  {:>4} counties  {:<24} {:<24} {}{mark}", r.counties.len(), t.key, t.name, ruler);
            }
        }
        "import" => {
            let folder = PathBuf::from(a(1)?);
            let save = PathBuf::from(a(2)?);
            let mut db = if CampaignDb::db_path(&folder).exists() {
                CampaignDb::open(&folder)?
            } else {
                CampaignDb::create(&folder, "CK3 campaign", &CampaignSettings::new("ck3", 45_819_283), &GameRegistry::builtin()?)?
            };
            let r = chronicle_ck3::import_save(&mut db, &save)?;
            println!("{}", serde_json::to_string_pretty(&r)?);
        }
        "registry" => {
            let reg = GameRegistry::builtin()?;
            println!("chain: {}", reg.chain.join(" → "));
            for t in &reg.transitions {
                let alts: Vec<String> = t.alternatives.iter().map(ToString::to_string).collect();
                println!(
                    "{:>14} → {:<14} {:<11} alternatives: [{}]  custom start supported: {}",
                    t.from,
                    t.to,
                    t.default_date.to_string(),
                    alts.join(", "),
                    t.custom_start_supported
                );
            }
        }
        _ => return Err(HELP.into()),
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
