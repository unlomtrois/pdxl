//! `pdxl fmt` — format Paradox scripting files (expand-every-block style).
//!
//! Default prints the formatted text to stdout; `--write` rewrites files in
//! place (the first file-writing subcommand — writes go through a temp file
//! and a rename in the same directory, so a crash never leaves a
//! half-written script); `--check` prints the names of unformatted files
//! and fails, for CI. Files with parse diagnostics are refused and left
//! untouched (formatting an error-recovered tree is destructive); other
//! files on the command line still proceed.

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

pub fn run(files: &[String], write: bool, check: bool) -> io::Result<ExitCode> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut failed = false;
    let mut unformatted = 0usize;

    for file in files {
        let src = match std::fs::read(file) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{file}: {e}");
                failed = true;
                continue;
            }
        };
        let formatted = match pdxl_fmt::format(file, &src) {
            Ok(s) => s,
            Err(pdxl_fmt::FmtError::ParseDiagnostics(diags)) => {
                for d in &diags {
                    let (line, col) = pdxl_src::line_col(&src, d.offset);
                    eprintln!("{}:{line}:{col}: {}", d.filename, d.message);
                }
                eprintln!("{file}: not formatted (fix parse errors first)");
                failed = true;
                continue;
            }
            Err(e @ (pdxl_fmt::FmtError::Verify { .. } | pdxl_fmt::FmtError::Unsupported)) => {
                eprintln!("{file}: {e}");
                failed = true;
                continue;
            }
        };
        let changed = formatted.as_bytes() != src.as_slice();

        if check {
            if changed {
                writeln!(out, "{file}")?;
                unformatted += 1;
            }
        } else if write {
            if changed {
                write_atomically(Path::new(file), formatted.as_bytes())?;
            }
        } else {
            out.write_all(formatted.as_bytes())?;
        }
    }
    out.flush()?;

    if failed || unformatted > 0 {
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

/// Writes via a temp file in the same directory + rename, so an interrupted
/// run never leaves a truncated script behind.
fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = path.file_name().map_or_else(
        || ".pdxl-fmt-tmp".to_string(),
        |n| format!(".{}.pdxl-fmt-tmp", n.to_string_lossy()),
    );
    tmp.push_str(&format!(".{}", std::process::id()));
    let tmp_path = dir.join(tmp);
    std::fs::write(&tmp_path, bytes)?;
    match std::fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}
