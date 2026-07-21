//! Ranks the game directories the CK3 schema does not model yet — the
//! "what to model next" worklist.
//!
//! Usage:
//!   cargo run -p pdxl-ck3 --bin schema-gaps -- \
//!     --game "<steam dir>/Crusader Kings III/game" [--all] [--min-defs N]
//!
//! Uncovered directories are ranked by definition count, with a boost for
//! directories that carry a `_*.info` doc (the source each schema entity is
//! written from). `--all` also lists covered directories (coverage summary).

use std::path::PathBuf;
use std::process::ExitCode;

use pdxl_ck3::coverage::{Coverage, survey};

fn main() -> ExitCode {
    let mut game: Option<PathBuf> = None;
    let mut show_all = false;
    let mut min_defs = 5u32;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--game" => match args.next() {
                Some(v) => game = Some(PathBuf::from(v)),
                None => {
                    eprintln!("missing value for --game");
                    return ExitCode::from(2);
                }
            },
            "--min-defs" => match args.next().and_then(|v| v.parse().ok()) {
                Some(v) => min_defs = v,
                None => {
                    eprintln!("missing/invalid value for --min-defs");
                    return ExitCode::from(2);
                }
            },
            "--all" => show_all = true,
            other => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(game) = game else {
        eprintln!("usage: schema-gaps --game <game dir> [--all] [--min-defs N]");
        return ExitCode::from(2);
    };

    let schema = pdxl_ck3::schema();
    let mut reports = match survey(&game, &schema) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("schema-gaps: {e}");
            return ExitCode::from(1);
        }
    };

    let covered = reports
        .iter()
        .filter(|r| r.coverage == Coverage::Defs)
        .count();
    let context_only = reports
        .iter()
        .filter(|r| r.coverage == Coverage::ContextOnly)
        .count();
    let total = reports.len();
    println!(
        "coverage: {covered}/{total} directories harvested ({context_only} more context-only)\n"
    );

    reports.sort_by_key(|r| std::cmp::Reverse(r.score()));

    println!(
        "{:<52} {:>6} {:>7}  info docs",
        "next targets (uncovered)", "files", "defs"
    );
    for r in reports.iter().filter(|r| r.coverage == Coverage::None) {
        if r.defs < min_defs && r.info_files.is_empty() {
            continue;
        }
        println!(
            "{:<52} {:>6} {:>7}  {}",
            r.rel_dir,
            r.files,
            r.defs,
            r.info_files.join(", ")
        );
    }

    if show_all {
        println!(
            "\n{:<52} {:>6} {:>7}  coverage",
            "all directories", "files", "defs"
        );
        for r in &reports {
            let cov = match r.coverage {
                Coverage::Defs => "defs",
                Coverage::ContextOnly => "context",
                Coverage::None => "-",
            };
            println!("{:<52} {:>6} {:>7}  {}", r.rel_dir, r.files, r.defs, cov);
        }
    }
    ExitCode::SUCCESS
}
