//! Development tool: show how a save is packaged and which top-level sections it has.
//! This is how adapter authors discover real section names (we never guess them).
//!
//! Usage: paradox-inspect <file> [--entry NAME] [--top N]

use std::path::PathBuf;
use std::process::ExitCode;

use paradox_parser::container::{ContainerKind, Encoding};
use paradox_parser::{TopLevelIndex, detect, source};

struct Args {
    path: PathBuf,
    entry: Option<String>,
    top: usize,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let mut path = None;
    let mut entry = None;
    let mut top = 40;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--entry" => entry = Some(it.next().ok_or("--entry needs a value")?),
            "--top" => top = it.next().ok_or("--top needs a value")?.parse().map_err(|_| "bad --top")?,
            "-h" | "--help" => return Err(String::new()),
            _ if path.is_none() => path = Some(PathBuf::from(a)),
            _ => return Err(format!("unexpected argument {a}")),
        }
    }
    Ok(Args { path: path.ok_or("missing <file>")?, entry, top })
}

fn print_index(label: &str, bytes: &[u8], top: usize) -> Result<(), Box<dyn std::error::Error>> {
    let idx = TopLevelIndex::build(bytes)?;
    let summary = idx.summary();
    println!("\n[{label}] {} bytes, {} top-level entries, {} distinct keys", bytes.len(), idx.spans.len(), summary.len());
    println!("{:>14}  {:>6}  key", "bytes", "count");
    for s in summary.iter().take(top) {
        println!("{:>14}  {:>6}  {}", s.total_bytes, s.occurrences, s.key);
    }
    Ok(())
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = source::open(&args.path)?;
    let d = detect(&bytes);
    println!("file:      {}", args.path.display());
    println!("size:      {} bytes", bytes.len());
    println!("header:    {}", d.header_line.as_deref().unwrap_or("(none)"));
    println!("container: {:?}", d.container);
    println!("encoding:  {:?}", d.encoding);

    match d.container {
        ContainerKind::Plain => match d.encoding {
            Encoding::Binary => println!("binary save: needs a TokenResolver (not supported in MVP)"),
            _ => print_index("payload", &bytes[d.payload_offset..], args.top)?,
        },
        ContainerKind::Zip => {
            let at = d.archive_offset.unwrap_or(d.payload_offset);
            if at > d.payload_offset {
                print_index("plain prefix before archive", &bytes[d.payload_offset..at], args.top)?;
            }
            #[cfg(feature = "zip")]
            {
                use paradox_parser::container::{archive, sniff_encoding};
                println!("\narchive entries:");
                for (name, size) in archive::entries(&bytes[at..])? {
                    println!("  {size:>14}  {name}");
                }
                if let Some(name) = &args.entry {
                    let data = archive::read_entry(&bytes[at..], name)?;
                    match sniff_encoding(&data) {
                        Encoding::Binary => println!("entry '{name}' is binary: needs a TokenResolver"),
                        _ => print_index(&format!("entry {name}"), &data, args.top)?,
                    }
                }
            }
            #[cfg(not(feature = "zip"))]
            println!("built without `zip` feature");
        }
        other => println!("{other:?} payloads are not supported yet"),
    }
    Ok(())
}

fn main() -> ExitCode {
    match parse_args() {
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("error: {msg}");
            }
            eprintln!("usage: paradox-inspect <file> [--entry NAME] [--top N]");
            ExitCode::from(2)
        }
        Ok(args) => match run(args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
    }
}
