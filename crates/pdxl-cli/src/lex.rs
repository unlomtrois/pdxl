//! `pdxl lex` — dump the token stream, mirroring Go's `cmd/pdxl/lex.go` output:
//! one token value per line; `--tags` prefixes the kind name (`%-17s`);
//! `--show-pos` prefixes `[basename:line:col]\t`; invalid tokens print as
//! `basename:line:col: invalid "text"` unconditionally.

use std::io::{self, Write};
use std::process::ExitCode;

use pdxl_lexer::Lexer;

pub fn run(file: &str, tags: bool, show_pos: bool) -> io::Result<ExitCode> {
    let data = std::fs::read(file)
        .map_err(|e| io::Error::new(e.kind(), format!("reading {file}: {e}")))?;
    let basename = std::path::Path::new(file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.to_string());

    let stdout = io::stdout();
    let mut w = io::BufWriter::new(stdout.lock());
    let mut lexer = Lexer::init(&data);
    while let Some(tok) = lexer.next_token() {
        let (line, col) = pdxl_src::line_col(&data, tok.range.start);
        let value = String::from_utf8_lossy(tok.value(&data));
        if tok.is_invalid() {
            // Go: fmt.Printf("%s: invalid %q\n", pos, value)
            writeln!(w, "{basename}:{line}:{col}: invalid {value:?}")?;
            continue;
        }
        if show_pos {
            write!(w, "[{basename}:{line}:{col}]\t")?;
        }
        if tags {
            // Go: fmt.Fprintf(&sb, "%-17s", tag)
            write!(w, "{:<17}", tok.kind.as_str())?;
        }
        writeln!(w, "{value}")?;
    }
    w.flush()?;
    Ok(ExitCode::SUCCESS)
}
