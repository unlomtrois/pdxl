//! Parser regression tests — golden snapshots.
//!
//! Historical note: these were byte-differential tests against the Go oracle
//! (`go run ./tools/parsedump`), verified byte-identical before the Go
//! implementation was removed. The last parity-verified tree dumps are
//! pinned as golden files. To accept an intentional change, regenerate with
//! `UPDATE_GOLDENS=1 cargo test -p pdxl-parser --test golden`
//! and review the diff like any other code change.

use std::path::{Path, PathBuf};

use pdxl_ast::NodeKind;
use pdxl_parser::{Parse, parse, validate_tree};

/// The dump schema version. Bump on any format change.
const DUMP_VERSION: u32 = 1;

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

/// Deterministic, normalized structured dump of a parse result: JSON with one
/// node and one diagnostic per line, so a line diff pinpoints the first
/// divergent node. `operator` is the token name only for `Field`; every other
/// node reports `"invalid"`. Filenames and source text are omitted (ranges plus
/// equal source bytes already prove node-text equality).
fn dump_json(parse: &Parse) -> String {
    let tree = parse.tree();
    let nodes = tree.nodes();
    let child_ids = tree.child_index();
    let diags = parse.diagnostics();

    let mut out = String::new();
    out.push('{');
    out.push_str("\"version\":");
    out.push_str(&DUMP_VERSION.to_string());
    out.push_str(",\"source_length\":");
    out.push_str(&tree.source().len().to_string());
    out.push_str(",\"nodes\":[");
    if !nodes.is_empty() {
        out.push('\n');
        for (i, node) in nodes.iter().enumerate() {
            let operator = if node.kind == NodeKind::Field {
                node.operator.as_str()
            } else {
                "invalid"
            };
            out.push_str("{\"id\":");
            out.push_str(&i.to_string());
            out.push_str(",\"kind\":\"");
            out.push_str(node.kind.as_str());
            out.push_str("\",\"start\":");
            out.push_str(&node.range.start.to_string());
            out.push_str(",\"end\":");
            out.push_str(&node.range.end.to_string());
            out.push_str(",\"operator\":\"");
            out.push_str(operator);
            out.push_str("\",\"child_start\":");
            out.push_str(&node.child_start.to_string());
            out.push_str(",\"child_end\":");
            out.push_str(&node.child_end.to_string());
            out.push('}');
            if i + 1 < nodes.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("],\"child_ids\":[");
    for (i, id) in child_ids.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&id.raw().to_string());
    }
    out.push_str("],\"doc_comments\":[");
    for (i, r) in tree.doc_comments().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("[{},{}]", r.start, r.end));
    }
    out.push_str("],\"diagnostics\":[");
    if !diags.is_empty() {
        out.push('\n');
        for (i, d) in diags.iter().enumerate() {
            out.push_str("{\"offset\":");
            out.push_str(&d.offset.to_string());
            out.push_str(",\"severity\":\"");
            out.push_str(d.severity.as_str());
            out.push_str("\",\"message\":\"");
            push_json_escaped(&mut out, &d.message);
            out.push_str("\"}");
            if i + 1 < diags.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("]}\n");
    out
}

/// Appends `s` to `out` with minimal JSON string escaping.
fn push_json_escaped(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
}

/// Every game's fixtures plus the malformed-input corner and the crate-local
/// stress fixtures. The parser is game-agnostic — the grammar is shared — so it
/// deliberately walks all of them.
fn fixtures(root: &Path) -> Vec<PathBuf> {
    let mut dirs = pdxl_testutil::shared_fixture_dirs(root);
    dirs.push(root.join("crates/pdxl-lexer/testdata"));
    pdxl_testutil::collect_fixtures(&dirs)
}

#[test]
fn parser_matches_goldens() {
    let root = repo_root();
    let goldens_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/goldens/parser");
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
             UPDATE_GOLDENS=1 cargo test -p pdxl-parser --test golden"
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
