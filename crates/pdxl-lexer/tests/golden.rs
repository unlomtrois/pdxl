//! Lexer regression tests — golden snapshots.
//!
//! Historical note: these were byte-differential tests against the Go oracle
//! (`go run ./tools/lexdump`), verified byte-identical before the Go
//! implementation was removed. The last parity-verified token streams are
//! pinned as golden files. To accept an intentional change, regenerate with
//! `UPDATE_GOLDENS=1 cargo test -p pdxl-lexer --test golden`
//! and review the diff like any other code change.

use std::path::{Path, PathBuf};

use pdxl_lexer::Lexer;

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

/// Canonical token dump: `<kind>\t<start>\t<end>`, one token per line. Every
/// token from `Lexer::next_token` is emitted, **including invalid ones**, so
/// invalid/partial-input behavior is covered.
fn dump_tokens(src: &[u8]) -> String {
    let mut lexer = Lexer::init(src);
    let mut out = String::new();
    while let Some(tok) = lexer.next_token() {
        out.push_str(tok.kind.as_str());
        out.push('\t');
        out.push_str(&tok.range.start.to_string());
        out.push('\t');
        out.push_str(&tok.range.end.to_string());
        out.push('\n');
    }
    out
}

/// Collects fixture files: `testdata/*.txt` plus `*.txt` in `testdata/ck3`,
/// `testdata/lint`, and the crate-local stress fixtures, sorted.
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
fn lexer_matches_goldens() {
    let root = repo_root();
    let goldens_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/goldens/lexer");
    let update = std::env::var_os("UPDATE_GOLDENS").is_some();
    if update {
        std::fs::create_dir_all(&goldens_dir).unwrap();
    }

    let fixtures = fixtures(&root);
    assert!(!fixtures.is_empty(), "no fixtures found");

    for file in &fixtures {
        let src = std::fs::read(file).expect("read fixture");
        let dump = dump_tokens(&src);
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
            "token dump changed for {stem}; if intentional, regenerate with \
             UPDATE_GOLDENS=1 cargo test -p pdxl-lexer --test golden"
        );
    }
}
