//! FileFacts dump tool (Rust side) for facts differential testing.
//!
//! Mirrors `tools/factsdump/main.go`: parses the file once, extracts facts
//! under each given relpath persona with the CK3 schema, and writes one
//! canonical dump per relpath. The file argument is used verbatim as the
//! extraction full path so both implementations emit identical ref locations.

use std::io::{self, Write};
use std::process::ExitCode;

use pdxl_analysis::extract_facts;
use pdxl_parity::dump_facts;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: factsdump <file> <relpath> [<relpath>...]");
        return ExitCode::from(2);
    }
    let file = &args[1];
    let data = match std::fs::read(file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("reading {file}: {e}");
            return ExitCode::from(1);
        }
    };
    let parsed = pdxl_parser::parse(file.clone(), data);
    let schema = pdxl_ck3::schema();

    let stdout = io::stdout();
    let mut w = stdout.lock();
    for rel_path in &args[2..] {
        let facts = extract_facts(parsed.tree(), rel_path, file, &schema);
        let _ = w.write_all(dump_facts(&facts, rel_path).as_bytes());
    }
    ExitCode::SUCCESS
}
