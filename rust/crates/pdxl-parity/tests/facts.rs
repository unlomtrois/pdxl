//! Facts extraction regression tests — golden snapshots.
//!
//! **The Go oracle is retired for the analysis layer** as of the landed-titles
//! schema (`ANALYSIS_VERSION` 2): the Rust schema has grown past the Go
//! implementation, so byte-comparison against `tools/factsdump` is no longer
//! meaningful. Regressions are pinned instead by golden files capturing the
//! canonical dump of every fixture under every directory persona.
//!
//! To accept an intentional behavior change, regenerate with:
//! `UPDATE_GOLDENS=1 cargo test -p pdxl-parity --test facts`
//! and review the golden diff like any other code change.

use std::path::{Path, PathBuf};

use pdxl_analysis::extract_facts;
use pdxl_parity::dump_facts;

/// Directory personas: one per CK3 def rule (incl. the nested landed-titles
/// rule), one gated (on_action), one that matches nothing.
const PERSONAS: &[&str] = &[
    "common/scripted_triggers/f.txt",
    "common/scripted_effects/f.txt",
    "common/traits/f.txt",
    "common/decisions/f.txt",
    "common/on_action/f.txt",
    "common/landed_titles/f.txt",
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
        root.join("rust/crates/pdxl-parity/testdata"),
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
fn facts_match_goldens() {
    let root = repo_root();
    let goldens_dir = root.join("rust/crates/pdxl-parity/testdata/goldens/facts");
    let update = std::env::var_os("UPDATE_GOLDENS").is_some();
    if update {
        std::fs::create_dir_all(&goldens_dir).unwrap();
    }

    let schema = pdxl_ck3::schema();
    let fixtures = fixtures(&root);
    assert!(!fixtures.is_empty(), "no fixtures found");

    let mut compared = 0;
    for file in &fixtures {
        // Repo-relative path keeps goldens machine-independent.
        let rel_file = file
            .strip_prefix(&root)
            .expect("fixture under repo root")
            .to_string_lossy()
            .into_owned();

        let src = std::fs::read(file).expect("read fixture");
        let parsed = pdxl_parser::parse(rel_file.clone(), src);
        let mut dump = String::new();
        for persona in PERSONAS {
            let facts = extract_facts(parsed.tree(), persona, &rel_file, &schema, None);
            dump.push_str(&dump_facts(&facts, persona));
        }

        let stem = file.file_stem().unwrap().to_string_lossy();
        let golden_path = goldens_dir.join(format!("{stem}.golden"));
        if update {
            std::fs::write(&golden_path, &dump).unwrap();
            continue;
        }
        let golden = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
            panic!("missing golden {golden_path:?} — run with UPDATE_GOLDENS=1")
        });
        if dump != golden {
            let d: Vec<&str> = dump.lines().collect();
            let g: Vec<&str> = golden.lines().collect();
            for i in 0..d.len().max(g.len()) {
                let (a, b) = (
                    d.get(i).copied().unwrap_or("<none>"),
                    g.get(i).copied().unwrap_or("<none>"),
                );
                if a != b {
                    panic!(
                        "facts dump changed for {stem} at line {i}:\n  now:    {a}\n  golden: {b}\n\
                         If intentional, regenerate: UPDATE_GOLDENS=1 cargo test -p pdxl-parity --test facts"
                    );
                }
            }
        }
        compared += 1;
    }
    if update {
        eprintln!("facts goldens regenerated for {} fixtures", fixtures.len());
    } else {
        eprintln!(
            "facts goldens: {compared} fixtures × {} personas match",
            PERSONAS.len()
        );
    }
}
