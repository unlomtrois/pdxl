//! Differential parity test: Rust fact extraction vs the Go oracle.
//!
//! Every fixture is extracted under several directory *personas* (the same
//! bytes mean different things under different rel_paths — location is
//! semantics in PDXScript), and the canonical dumps must match the Go tool
//! (`go run ./tools/factsdump`) byte-for-byte: defs (name, kind, file, offsets,
//! params), aliases, and refs (kind, name, byte range, file:line:col loc).
//!
//! Self-skips with a warning if `go` is unavailable.

use std::path::{Path, PathBuf};
use std::process::Command;

use pdxl_analysis::extract_facts;
use pdxl_parity::dump_facts;
use pdxl_testutil::go_available;

/// Directory personas: one per CK3 def rule, one gated (on_action), one that
/// matches nothing.
const PERSONAS: &[&str] = &[
    "common/scripted_triggers/f.txt",
    "common/scripted_effects/f.txt",
    "common/traits/f.txt",
    "common/decisions/f.txt",
    "common/on_action/f.txt",
    "events/f.txt",
    "history/characters/f.txt",
    "gfx/f.txt",
];

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let dirs = [
        root.join("testdata"),
        root.join("testdata/ck3"),
        root.join("testdata/lint"),
        root.join("rust/crates/pdxl-lexer/testdata"),
        root.join("rust/crates/pdxl-parity/testdata"), // facts stress fixtures
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

#[test]
fn facts_match_go_oracle() {
    let root = repo_root();
    if !go_available() {
        eprintln!("warning: `go` not found — skipping facts differential parity test");
        return;
    }

    let schema = pdxl_ck3::schema();
    let fixtures = fixtures(&root);
    assert!(!fixtures.is_empty(), "no fixtures found");

    let mut compared = 0;
    for file in &fixtures {
        // Both sides receive the same path string (relative to the repo root)
        // so ref `file`/`loc` fields agree without normalization.
        let rel_file = file
            .strip_prefix(&root)
            .expect("fixture under repo root")
            .to_string_lossy()
            .into_owned();

        // Rust side: parse once, extract under every persona (in-process).
        let src = std::fs::read(file).expect("read fixture");
        let parsed = pdxl_parser::parse(rel_file.clone(), src);
        let mut rust = String::new();
        for persona in PERSONAS {
            let facts = extract_facts(parsed.tree(), persona, &rel_file, &schema);
            rust.push_str(&dump_facts(&facts, persona));
        }

        // Go side: one invocation, all personas.
        let mut args: Vec<String> =
            vec!["run".into(), "./tools/factsdump".into(), rel_file.clone()];
        args.extend(PERSONAS.iter().map(|p| p.to_string()));
        let out = Command::new("go")
            .current_dir(&root)
            .args(&args)
            .output()
            .expect("spawn go factsdump");
        assert!(
            out.status.success(),
            "go factsdump failed for {rel_file}:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let go = String::from_utf8(out.stdout).expect("utf8");

        if rust != go {
            let rust_lines: Vec<&str> = rust.lines().collect();
            let go_lines: Vec<&str> = go.lines().collect();
            let mut detail = String::from("(no line-level difference found)");
            for i in 0..rust_lines.len().max(go_lines.len()) {
                let r = rust_lines.get(i).copied().unwrap_or("<none>");
                let g = go_lines.get(i).copied().unwrap_or("<none>");
                if r != g {
                    detail = format!("line {i}:\n  rust: {r}\n  go:   {g}");
                    break;
                }
            }
            panic!("facts dump mismatch for {rel_file}:\n{detail}");
        }
        compared += 1;
    }
    eprintln!(
        "facts parity: {compared} fixtures × {} personas byte-identical to Go oracle",
        PERSONAS.len()
    );
}
