//! Ranks the game directories the compiled-in schema does not model yet —
//! the "what to model next" worklist. Game-agnostic: the schema, survey
//! roots, and context roots come through the `pdxl-game` facade, so the
//! build feature picks the game:
//!
//!   cargo run --release -p pdxl-cli --features ck3 --bin schema-gaps -- \
//!     --game "<game dir>" [--all] [--min-defs N]
//!
//! Uncovered directories are ranked by definition count, with a boost for
//! directories that carry a `_*.info` doc (the source each schema entity is
//! written from). `--all` also lists covered directories (coverage summary).

use std::path::PathBuf;
use std::process::ExitCode;

use pdxl_project::coverage::{Coverage, survey};

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

    let schema = pdxl_game::schema();
    let context_roots: Vec<&str> = pdxl_game::contexts::context_schema()
        .roots
        .iter()
        .map(|(prefix, _)| *prefix)
        .collect();
    let mut reports = match survey(
        &game,
        &schema,
        pdxl_game::coverage::SURVEY_ROOTS,
        &context_roots,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("schema-gaps: {e}");
            return ExitCode::from(1);
        }
    };

    let covered = reports
        .iter()
        .filter(|r| r.coverage != Coverage::None)
        .count();
    let total = reports.len();
    eprintln!(
        "[{}] {covered}/{total} directories covered (defs or documented bodies)",
        pdxl_game::GAME
    );

    reports.sort_by_key(|r| std::cmp::Reverse(r.score()));
    for r in &reports {
        let show = match r.coverage {
            Coverage::None => r.defs >= min_defs,
            _ => show_all,
        };
        if !show {
            continue;
        }
        let mark = match r.coverage {
            Coverage::Defs => "[defs]",
            Coverage::ContextOnly => "[body]",
            Coverage::None => "      ",
        };
        let info = if r.info_files.is_empty() {
            String::new()
        } else {
            format!("  ({})", r.info_files.join(", "))
        };
        println!(
            "{mark} {:>6} defs {:>4} files  {}{info}",
            r.defs, r.files, r.rel_dir
        );
    }
    ExitCode::SUCCESS
}
