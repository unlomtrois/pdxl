//! The shared facts-golden harness.
//!
//! **The Go oracle is retired for the analysis layer** as of the landed-titles
//! schema (`ANALYSIS_VERSION` 2): the Rust schema has grown past the Go
//! implementation, so byte-comparison against `tools/factsdump` is no longer
//! meaningful. Regressions are pinned instead by golden files capturing the
//! canonical dump of every fixture under every directory persona.
//!
//! This lives here rather than inlined per game crate so the dump format has
//! exactly one definition. A format change bumps [`FACTS_DUMP_VERSION`] once
//! and every game's goldens regenerate together; a new game target writes a
//! ~15-line `tests/facts.rs` that calls [`run`] with its own schema, personas
//! and goldens directory, and inherits the rest.

use std::path::PathBuf;

use pdxl_analysis::{FileFacts, Schema, Symbol, extract_facts};

/// Facts dump schema version. Bump on any format change.
pub const FACTS_DUMP_VERSION: u32 = 3; // 3: soft/doc-reference channel (+calls)
// 2: file-local script constants (+constants/+constant_refs)

/// One game's facts-golden run.
pub struct FactsGoldens<'a> {
    /// The calling crate's `CARGO_MANIFEST_DIR`; goldens land under
    /// `<manifest>/testdata/goldens/facts/`.
    pub manifest_dir: &'a str,
    /// Which game's fixtures to walk — see [`crate::GAMES`].
    pub game: &'a str,
    /// Extra fixture directories beyond `testdata/<game>/`, repo-relative.
    pub extra_dirs: &'a [&'a str],
    /// Directory paths the fixture is analyzed *as*, one per def rule worth
    /// exercising plus at least one that matches nothing.
    pub personas: &'a [&'a str],
    /// The game's schema.
    pub schema: &'a Schema,
    /// Shown in the "regenerate with…" message on failure.
    pub crate_name: &'a str,
}

impl FactsGoldens<'_> {
    /// Compares every fixture's dump against its golden, or rewrites them all
    /// when `UPDATE_GOLDENS` is set. Panics with the first differing line.
    pub fn run(&self) {
        let root = crate::repo_root(self.manifest_dir);
        let goldens_dir = PathBuf::from(self.manifest_dir).join("testdata/goldens/facts");
        let update = std::env::var_os("UPDATE_GOLDENS").is_some();
        if update {
            std::fs::create_dir_all(&goldens_dir).unwrap();
        }

        let mut dirs = crate::game_fixture_dirs(&root, self.game);
        dirs.extend(self.extra_dirs.iter().map(|d| root.join(d)));
        let fixtures = crate::collect_fixtures(&dirs);
        assert!(!fixtures.is_empty(), "no fixtures found for {}", self.game);

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
            for persona in self.personas {
                let facts = extract_facts(parsed.tree(), persona, &rel_file, self.schema, None);
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
                             If intentional, regenerate: UPDATE_GOLDENS=1 cargo test -p {} --test facts",
                            self.crate_name
                        );
                    }
                }
            }
            compared += 1;
        }
        if update {
            eprintln!("facts goldens regenerated for {} fixtures", fixtures.len());
        } else {
            assert!(compared > 0, "no goldens compared");
        }
    }
}

/// Canonical dump of one extraction run: defs, aliases, and refs as JSON, one
/// per line, so a line diff pinpoints the first divergence.
fn dump_facts(facts: &FileFacts, rel_path: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n\"version\":");
    out.push_str(&FACTS_DUMP_VERSION.to_string());
    out.push_str(",\n\"rel_path\":\"");
    push_escaped(&mut out, rel_path);
    out.push_str("\",\n\"defs\":[");
    push_symbols(&mut out, &facts.defs);
    out.push_str("],\n\"aliases\":[");
    push_symbols(&mut out, &facts.aliases);
    out.push_str("],\n\"refs\":[");
    push_refs(&mut out, &facts.refs);
    out.push_str("],\n\"constants\":[");
    push_symbols(&mut out, &facts.constants);
    out.push_str("],\n\"constant_refs\":[");
    push_refs(&mut out, &facts.constant_refs);
    // The soft channel: smart-doc references (and, in the project layer, gui
    // and call-by-name refs). Navigable and counted, never diagnosed.
    out.push_str("],\n\"calls\":[");
    push_refs(&mut out, &facts.calls);
    out.push_str("]\n}\n");
    out
}

fn push_refs(out: &mut String, refs: &[pdxl_analysis::Ref]) {
    if refs.is_empty() {
        return;
    }
    out.push('\n');
    for (i, r) in refs.iter().enumerate() {
        out.push_str("{\"kind\":\"");
        out.push_str(r.kind.name());
        out.push_str("\",\"name\":\"");
        push_escaped(out, &r.name);
        out.push_str("\",\"file\":\"");
        push_escaped(out, &r.file);
        out.push_str("\",\"start\":");
        out.push_str(&r.start.to_string());
        out.push_str(",\"end\":");
        out.push_str(&r.end.to_string());
        out.push('}');
        if i + 1 < refs.len() {
            out.push(',');
        }
        out.push('\n');
    }
}

fn push_symbols(out: &mut String, symbols: &[Symbol]) {
    if symbols.is_empty() {
        return;
    }
    out.push('\n');
    for (i, s) in symbols.iter().enumerate() {
        out.push_str("{\"name\":\"");
        push_escaped(out, &s.name);
        out.push_str("\",\"kind\":\"");
        out.push_str(s.kind.name());
        out.push_str("\",\"file\":\"");
        push_escaped(out, &s.file);
        out.push_str("\",\"offset\":");
        out.push_str(&s.offset.to_string());
        out.push_str(",\"end_offset\":");
        out.push_str(&s.end_offset.to_string());
        out.push_str(",\"params\":[");
        for (j, p) in s.params.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push('"');
            push_escaped(out, p);
            out.push('"');
        }
        out.push_str("]}");
        if i + 1 < symbols.len() {
            out.push(',');
        }
        out.push('\n');
    }
}

fn push_escaped(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
}
