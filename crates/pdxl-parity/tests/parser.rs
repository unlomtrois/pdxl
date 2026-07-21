//! Parser regression tests — golden snapshots.
//!
//! Historical note: these were byte-differential tests against the Go oracle
//! (`go run ./tools/parsedump`), verified byte-identical before the Go
//! implementation was removed. The last parity-verified tree dumps are
//! pinned as golden files. To accept an intentional change, regenerate with
//! `UPDATE_GOLDENS=1 cargo test -p pdxl-parity --test parser`
//! and review the diff like any other code change.

use std::path::{Path, PathBuf};

use pdxl_parity::dump_json;
use pdxl_parser::{parse, validate_tree};

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

/// Fixtures: shared `testdata` (incl. ck3 + malformed lint) plus the
/// crate-local stress fixtures, sorted.
fn fixtures(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let dirs = [
        root.join("testdata"),
        root.join("testdata/ck3"),
        root.join("testdata/lint"),
        root.join("crates/pdxl-lexer/testdata"),
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
fn parser_matches_goldens() {
    let root = repo_root();
    let goldens_dir = root.join("crates/pdxl-parity/testdata/goldens/parser");
    let update = std::env::var_os("UPDATE_GOLDENS").is_some();
    if update {
        std::fs::create_dir_all(&goldens_dir).unwrap();
    }

    let fixtures = fixtures(&root);
    assert!(!fixtures.is_empty(), "no fixtures found");

    for file in &fixtures {
        let src = std::fs::read(file).expect("read fixture");
        let parsed = parse("input", src);
        // Invariants must hold for every fixture, including malformed ones.
        validate_tree(parsed.tree())
            .unwrap_or_else(|e| panic!("{}: invalid tree: {e:?}", file.display()));

        let dump = dump_json(&parsed);
        let stem = file.file_stem().unwrap().to_string_lossy();
        let golden_path = goldens_dir.join(format!("{stem}.golden"));
        if update {
            std::fs::write(&golden_path, &dump).unwrap();
            continue;
        }
        let golden = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
            panic!("missing golden {golden_path:?} — run with UPDATE_GOLDENS=1")
        });
        assert_eq!(
            dump, golden,
            "tree dump changed for {stem}; if intentional, regenerate with \
             UPDATE_GOLDENS=1 cargo test -p pdxl-parity --test parser"
        );
    }
}

#[test]
fn dump_is_stable_and_normalized() {
    // Non-field nodes report operator "invalid"; a field reports its op name.
    let p = parse("test", &b"a = b"[..]);
    assert!(p.diagnostics().is_empty());
    let dump = dump_json(&p);
    assert!(dump.contains("\"kind\":\"file\",\"start\":0,\"end\":0,\"operator\":\"invalid\""));
    assert!(dump.contains("\"kind\":\"field\""));
    assert!(dump.contains("\"operator\":\"equal\""));
    assert!(dump.starts_with("{\"version\":1,"));
}
