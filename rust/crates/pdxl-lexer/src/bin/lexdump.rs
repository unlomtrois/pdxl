//! Deterministic token-dump tool for lexer parity testing (Rust side).
//!
//! Mirrors `tools/lexdump/main.go`: tokenizes the file given as the single
//! argument with the Rust lexer and writes one token per line as
//! `<kind>\t<start>\t<end>`, including invalid tokens. The output is compared
//! byte-for-byte against the Go oracle by the differential parity test.

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use pdxl_lexer::Lexer;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: lexdump <file>");
        return ExitCode::from(2);
    }

    let data = match std::fs::read(&args[1]) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("reading {}: {e}", args[1]);
            return ExitCode::from(1);
        }
    };

    let stdout = io::stdout();
    let mut w = BufWriter::new(stdout.lock());

    let mut lexer = Lexer::init(&data);
    while let Some(tok) = lexer.next_token() {
        // Infallible barring a closed pipe; ignore write errors on shutdown.
        let _ = writeln!(
            w,
            "{}\t{}\t{}",
            tok.kind.as_str(),
            tok.range.start,
            tok.range.end
        );
    }
    let _ = w.flush();
    ExitCode::SUCCESS
}
