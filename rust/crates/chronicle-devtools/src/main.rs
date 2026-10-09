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
  registry                            validated game registry and transition dates";

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
