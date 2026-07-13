//! The `pdxl` command-line tool (Rust port).
//!
//! Ports the Go CLI incrementally (M7 milestone: `lex`, `parse`, `check`);
//! output formats mirror `cmd/pdxl` so the two binaries can be snapshot-diffed.
//! Deviations, deliberate and documented:
//! - No `pdxl.toml` loading yet — the built-in defaults equal Go's
//!   `config.Default()` (same ignore lists, same cache dir/size).
//! - No Proton path resolution (the project references mods by local path).
//! - `parse --json` is not ported (its shape is Go's internal struct encoding).

mod check;
mod lex;
mod parse;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "pdxl", version, about = "Toolkit for Paradox scripting files")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Tokenize a Paradox scripting file
    Lex {
        file: String,
        /// show token tag alongside each value
        #[arg(long)]
        tags: bool,
        /// show filename and position alongside each value
        #[arg(long = "show-pos")]
        show_pos: bool,
    },
    /// Parse a Paradox scripting file and print the AST
    Parse {
        file: String,
        /// output AST as a labelled node tree
        #[arg(long)]
        tree: bool,
    },
    /// Index project definitions and resolve references across game+mod
    Check {
        /// report only this file's unresolved references
        file: Option<String>,
        /// path to vanilla game directory
        #[arg(long)]
        game: Option<String>,
        /// path to mod directory or .mod file
        #[arg(long = "mod")]
        mod_: Option<String>,
        /// disable parse cache
        #[arg(long = "no-cache")]
        no_cache: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Lex {
            file,
            tags,
            show_pos,
        } => lex::run(&file, tags, show_pos),
        Command::Parse { file, tree } => parse::run(&file, tree),
        Command::Check {
            file,
            game,
            mod_,
            no_cache,
        } => check::run(file.as_deref(), game.as_deref(), mod_.as_deref(), no_cache),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}
