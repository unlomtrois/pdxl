//! Schema-coverage survey: which game directories the CK3 schema does *not*
//! model yet, ranked as modelling targets.
//!
//! Walks the definition roots of a game installation (`common/`, `events/`,
//! `history/`), and for every leaf directory holding `.txt` script files
//! reports: file and top-level definition counts, the `_*.info` docs Paradox
//! left there (the source material each schema entity is written from), and
//! whether the schema already covers the directory (a def rule matches, or a
//! structural-context root gives it a documented body). The `schema-gaps`
//! bin renders this as the "what to model next" worklist.

use std::io;
use std::path::{Path, PathBuf};

use pdxl_analysis::Schema;
use pdxl_ast::NodeKind;

/// What the schema knows about one directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    /// A definition rule matches — symbols are harvested here.
    Defs,
    /// No def rule, but a structural-context root documents the bodies.
    ContextOnly,
    /// The schema knows nothing about this directory.
    None,
}

/// The survey result for one leaf directory.
#[derive(Clone, Debug)]
pub struct DirReport {
    /// Overlay-style relative dir (lowercase, forward slashes, trailing `/`).
    pub rel_dir: String,
    /// Script files directly in this directory.
    pub files: u32,
    /// Top-level `NAME = { … }` / `NAME = value` fields across those files —
    /// an upper bound on harvestable definitions.
    pub defs: u32,
    /// `_*.info` documentation files Paradox left in the directory.
    pub info_files: Vec<String>,
    pub coverage: Coverage,
}

impl DirReport {
    /// The modelling-priority score: definitions dominate, documented
    /// directories (an `.info` exists) are strongly preferred because the
    /// schema entity can be written from it.
    pub fn score(&self) -> u64 {
        let info_boost = if self.info_files.is_empty() { 1 } else { 4 };
        u64::from(self.defs) * info_boost
    }
}

/// The definition roots worth surveying (gui is modeled by `pdxl-gui`,
/// localization by `pdxl-loc`).
const ROOTS: &[&str] = &["common", "events", "history"];

/// Surveys `game_root` against `schema` (+ the structural-context roots).
/// Returns one report per leaf directory containing `.txt` files, unsorted —
/// rank with [`DirReport::score`].
pub fn survey(game_root: &Path, schema: &Schema) -> io::Result<Vec<DirReport>> {
    let context_roots: Vec<&str> = crate::contexts::context_schema()
        .roots
        .iter()
        .map(|(prefix, _)| *prefix)
        .collect();
    let mut out = Vec::new();
    for root in ROOTS {
        let path = game_root.join(root);
        if path.is_dir() {
            walk(&path, root, schema, &context_roots, &mut out)?;
        }
    }
    Ok(out)
}

fn walk(
    dir: &Path,
    rel: &str,
    schema: &Schema,
    context_roots: &[&str],
    out: &mut Vec<DirReport>,
) -> io::Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut info_files: Vec<String> = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            if !name.starts_with('.') {
                walk(
                    &path,
                    &format!("{rel}/{}", name.to_lowercase()),
                    schema,
                    context_roots,
                    out,
                )?;
            }
        } else if name.ends_with(".txt") {
            files.push(path);
        } else if name.ends_with(".info") {
            info_files.push(name);
        }
    }
    if files.is_empty() && info_files.is_empty() {
        return Ok(());
    }

    let rel_dir = format!("{}/", rel.to_lowercase());
    let probe = format!("{rel_dir}x.txt");
    let coverage = if schema.rule_for(&probe).is_some() {
        Coverage::Defs
    } else if context_roots.iter().any(|r| probe.starts_with(r)) {
        Coverage::ContextOnly
    } else {
        Coverage::None
    };

    let mut defs = 0u32;
    for file in &files {
        let src = std::fs::read(file)?;
        let (tree, _) = pdxl_parser::parse(file.to_string_lossy().into_owned(), src).into_parts();
        for child in tree.children(tree.root()) {
            if tree.node(child).kind == NodeKind::Field {
                defs += 1;
            }
        }
    }

    out.push(DirReport {
        rel_dir,
        files: files.len() as u32,
        defs,
        info_files,
        coverage,
    });
    Ok(())
}
