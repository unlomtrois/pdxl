//! Differential parity test: the Rust parser vs the Go oracle.
//!
//! For every shared fixture (valid and malformed), this runs the Go structured
//! dump tool (`go run ./tools/parsedump <file>`) and the Rust parser, then
//! asserts the canonical dumps are byte-identical — proving exact parity of node
//! count, node ids and allocation order, kinds, byte ranges, normalized
//! operators, child ranges, the full child-index array, and diagnostics (order,
//! offset, severity, message).
//!
//! Self-skips with a warning if the `go` toolchain is unavailable, so the Rust
//! suite still runs in Rust-only environments — but parity is only demonstrated
//! when Go is present.

use std::path::{Path, PathBuf};
use std::process::Command;

use pdxl_syntax::{dump_json, parse, validate_tree};

fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("go.mod").is_file() {
            return dir;
        }
        assert!(dir.pop(), "could not locate repo root");
    }
}

fn go_available() -> bool {
    Command::new("go")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Fixtures: shared `testdata` (incl. ck3 + malformed lint) plus the Rust-side
/// lexer stress fixture (malformed UTF-8), all sorted for determinism.
fn fixtures(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let dirs = [
        root.join("testdata"),
        root.join("testdata/ck3"),
        root.join("testdata/lint"),
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

fn go_dump(root: &Path, file: &Path) -> String {
    let output = Command::new("go")
        .current_dir(root)
        .args(["run", "./tools/parsedump"])
        .arg(file)
        .output()
        .expect("failed to spawn `go run ./tools/parsedump`");
    assert!(
        output.status.success(),
        "go parsedump failed for {}:\n{}",
        file.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("go parsedump produced non-UTF-8 output")
}

#[test]
fn parser_matches_go_oracle() {
    let root = repo_root();
    if !go_available() {
        eprintln!("warning: `go` toolchain not found — skipping parser differential parity test");
        return;
    }

    let fixtures = fixtures(&root);
    assert!(!fixtures.is_empty(), "no fixtures found");

    let mut compared = 0;
    for file in &fixtures {
        let src = std::fs::read(file).expect("read fixture");
        let parsed = parse("input", src);
        // Invariants must hold for every fixture, including malformed ones.
        validate_tree(parsed.tree())
            .unwrap_or_else(|e| panic!("{}: invalid tree: {e:?}", file.display()));

        let rust = dump_json(&parsed);
        let go = go_dump(&root, file);

        if rust != go {
            let rust_lines: Vec<&str> = rust.lines().collect();
            let go_lines: Vec<&str> = go.lines().collect();
            let mut detail = String::from("(no line-level difference found)");
            let max = rust_lines.len().max(go_lines.len());
            for i in 0..max {
                let r = rust_lines.get(i).copied().unwrap_or("<none>");
                let g = go_lines.get(i).copied().unwrap_or("<none>");
                if r != g {
                    detail = format!("line {i}:\n  rust: {r}\n  go:   {g}");
                    break;
                }
            }
            panic!("dump mismatch in {}:\n{}", file.display(), detail);
        }
        compared += 1;
    }
    eprintln!("parser parity: {compared} fixtures byte-identical to Go oracle");
}
