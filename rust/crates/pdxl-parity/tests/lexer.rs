//! Differential parity test: the Rust lexer vs the Go oracle.
//!
//! For every shared fixture under `testdata/`, this runs the Go token-dump tool
//! (`go run ./tools/lexdump <file>`) and the Rust lexer, then asserts the two
//! token streams are byte-for-byte identical in `<kind>\t<start>\t<end>` form.
//!
//! The Go implementation is the oracle. If the `go` toolchain is unavailable the
//! test is skipped (not failed), so `cargo test` still works in Rust-only
//! environments — but parity is only *demonstrated* when Go is present.

use std::path::{Path, PathBuf};
use std::process::Command;

use pdxl_parity::dump_tokens as rust_dump;
use pdxl_testutil::go_available;

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

/// Collects fixture files: `testdata/*.txt` plus `*.txt` in `testdata/ck3` and
/// `testdata/lint`, sorted for determinism.
fn fixtures(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let dirs = [
        root.join("testdata"),
        root.join("testdata/ck3"),
        root.join("testdata/lint"),
        // Rust-side stress fixtures (malformed UTF-8, lone operators) kept out of
        // the Go test globs but still compared against the Go oracle here.
        root.join("rust/crates/pdxl-lexer/testdata"),
    ];
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("txt") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Go oracle token dump for `file`, via `go run ./tools/lexdump`.
fn go_dump(root: &Path, file: &Path) -> String {
    let output = Command::new("go")
        .current_dir(root)
        .args(["run", "./tools/lexdump"])
        .arg(file)
        .output()
        .expect("failed to spawn `go run ./tools/lexdump`");
    assert!(
        output.status.success(),
        "go lexdump failed for {}:\n{}",
        file.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("go lexdump produced non-UTF-8 output")
}

#[test]
fn lexer_matches_go_oracle() {
    let root = repo_root();

    if !go_available() {
        eprintln!("warning: `go` toolchain not found — skipping lexer differential parity test");
        return;
    }

    let fixtures = fixtures(&root);
    assert!(
        !fixtures.is_empty(),
        "no fixtures found under {}/testdata",
        root.display()
    );

    let mut compared = 0usize;
    for file in &fixtures {
        let src = std::fs::read(file).expect("read fixture");
        let rust = rust_dump(&src);
        let go = go_dump(&root, file);

        if rust != go {
            // Produce a focused diff of the first mismatching line.
            let rust_lines: Vec<&str> = rust.lines().collect();
            let go_lines: Vec<&str> = go.lines().collect();
            let mut detail = String::new();
            let max = rust_lines.len().max(go_lines.len());
            for i in 0..max {
                let r = rust_lines.get(i).copied().unwrap_or("<none>");
                let g = go_lines.get(i).copied().unwrap_or("<none>");
                if r != g {
                    detail = format!("token {i}: rust={r:?} go={g:?}");
                    break;
                }
            }
            panic!(
                "token mismatch in {}:\n  {}\n  (rust {} tokens, go {} tokens)",
                file.display(),
                detail,
                rust_lines.len(),
                go_lines.len()
            );
        }
        compared += 1;
    }

    eprintln!("lexer parity: {compared} fixtures byte-identical to Go oracle");
}
