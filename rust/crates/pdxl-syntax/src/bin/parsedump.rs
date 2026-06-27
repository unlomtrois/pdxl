//! Structured parse-dump tool (Rust side) for parser differential testing.
//!
//! Mirrors `tools/parsedump/main.go`: parses the file given as the single
//! argument with the Rust parser v3 port and writes the canonical normalized
//! dump (see `pdxl_syntax::dump_json`) to stdout. Filenames are normalized out of
//! the dump, so different checkout paths do not cause false mismatches.

use std::io::{self, Write};
use std::process::ExitCode;

use pdxl_syntax::{dump_json, parse};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: parsedump <file>");
        return ExitCode::from(2);
    }

    let data = match std::fs::read(&args[1]) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("reading {}: {e}", args[1]);
            return ExitCode::from(1);
        }
    };

    // The dump intentionally omits the filename, so the value passed here does
    // not affect output; use a fixed placeholder.
    let parsed = parse("input", data);
    let dump = dump_json(&parsed);
    let _ = io::stdout().write_all(dump.as_bytes());
    ExitCode::SUCCESS
}
