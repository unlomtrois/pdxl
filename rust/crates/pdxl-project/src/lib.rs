//! Whole-project analysis over a [`FileSet`]: gather per-file facts, build the
//! symbol table, resolve references, and update incrementally.
//!
//! Port of `internal/validate`'s `Analyze`/`gatherFacts`/`Project`. The
//! incremental model is deliberately simple and is preserved exactly:
//!
//! ```text
//! edit one file
//!     ↓
//! re-parse only that file            (update / update_source)
//!     ↓
//! replace that file's FileFacts
//!     ↓
//! rebuild the whole table from in-memory facts   (merge_and_resolve — pure)
//! ```
//!
//! The whole-table rebuild sounds expensive but is cheap: every *other* file's
//! facts are already in memory, and facts are tiny. Do not "optimize" this into
//! partial table mutation without measuring — the rebuild is what keeps
//! incrementality correct and simple.
//!
//! Deliberate deviations from Go (per the measured-simplification plan):
//! - No `FactStore` (see the M5 benchmark: cold gather of a CK3-scale corpus is
//!   ~87 ms single-threaded).
//! - No AST-cache integration in the gather path yet; `pdxl-cache` exists and
//!   can be wired in at the LSP milestone if warm-start measurements ask for it.
//! - The schema is an explicit parameter (Go links the CK3 registry directly).

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use pdxl_analysis::{
    FileFacts, Ref, RefDiag, Schema, SymbolKind, SymbolTable, extract_facts, merge_and_resolve,
};
use pdxl_fileset::FileSet;

/// Identifies a project file by both its FileSet `rel_path` (drives the def
/// rule and stable duplicate ordering) and its on-disk full path.
#[derive(Clone, Debug)]
struct FileKey {
    rel: String,
    full: PathBuf,
}

/// One-shot analysis: gather facts for every winning file and resolve.
///
/// Mirrors Go's `Analyze(fs, nil, nil)` (no caches).
pub fn analyze(fs: &FileSet, schema: &Schema) -> io::Result<(SymbolTable, Vec<RefDiag>)> {
    let (order, facts) = gather_facts(fs, schema)?;
    let rels: Vec<&str> = order.iter().map(|k| k.rel.as_str()).collect();
    Ok(merge_and_resolve(&rels, &facts))
}

/// Walks `fs` once, parsing every winning file and extracting its facts.
fn gather_facts(
    fs: &FileSet,
    schema: &Schema,
) -> io::Result<(Vec<FileKey>, HashMap<String, FileFacts>)> {
    let mut order = Vec::new();
    let mut facts = HashMap::new();
    fs.try_for_each(|entry| -> io::Result<()> {
        let src = std::fs::read(&entry.full_path)?;
        let full = entry.full_path.to_string_lossy().into_owned();
        let parsed = pdxl_parser::parse(full.clone(), src);
        let f = extract_facts(parsed.tree(), &entry.rel_path, &full, schema);
        order.push(FileKey {
            rel: entry.rel_path.clone(),
            full: entry.full_path.clone(),
        });
        facts.insert(entry.rel_path.clone(), f);
        Ok(())
    })?;
    Ok((order, facts))
}

/// A whole-project symbol table held in memory, supporting cheap incremental
/// updates. The foundation for a long-running validator (LSP / watch loop).
/// Not safe for concurrent use (wrap it yourself; the LSP layer owns exactly
/// one behind its own lock, as in Go).
pub struct Project {
    schema: Schema,
    order: Vec<FileKey>,
    facts: HashMap<String, FileFacts>,
    table: SymbolTable,
    diags: Vec<RefDiag>,
}

impl Project {
    /// Gathers facts for every winning file in `fs` and builds the initial
    /// table and diagnostics.
    pub fn new(fs: &FileSet, schema: Schema) -> io::Result<Project> {
        let (order, facts) = gather_facts(fs, &schema)?;
        let mut p = Project {
            schema,
            order,
            facts,
            table: SymbolTable::new(),
            diags: Vec::new(),
        };
        p.rebuild();
        Ok(p)
    }

    /// Recomputes the table and diagnostics from the in-memory facts.
    fn rebuild(&mut self) {
        let rels: Vec<&str> = self.order.iter().map(|k| k.rel.as_str()).collect();
        let (table, diags) = merge_and_resolve(&rels, &self.facts);
        self.table = table;
        self.diags = diags;
    }

    /// Re-extracts the single tracked file at `full_path` from disk, replaces
    /// its facts, then rebuilds in memory. No other file is re-read.
    /// `full_path` must already be part of the project (adding/removing files
    /// needs a fresh FileSet scan).
    pub fn update(&mut self, full_path: &Path) -> io::Result<()> {
        let key = self.key_for(full_path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not part of the project", full_path.display()),
            )
        })?;
        let src = std::fs::read(&key.full)?;
        self.replace_facts(&key, src);
        Ok(())
    }

    /// Re-analyzes a tracked file from an in-memory buffer (e.g. an unsaved
    /// editor document) instead of disk, then rebuilds. Disk state and any
    /// caches are untouched — the buffer may differ from disk.
    pub fn update_source(&mut self, full_path: &Path, src: Vec<u8>) -> io::Result<()> {
        let key = self.key_for(full_path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not part of the project", full_path.display()),
            )
        })?;
        self.replace_facts(&key, src);
        Ok(())
    }

    fn replace_facts(&mut self, key: &FileKey, src: Vec<u8>) {
        let full = key.full.to_string_lossy().into_owned();
        let parsed = pdxl_parser::parse(full.clone(), src);
        let facts = extract_facts(parsed.tree(), &key.rel, &full, &self.schema);
        self.facts.insert(key.rel.clone(), facts);
        self.rebuild();
    }

    /// The current whole-project symbol table.
    pub fn table(&self) -> &SymbolTable {
        &self.table
    }

    /// All unresolved-reference diagnostics across the project.
    pub fn diags(&self) -> &[RefDiag] {
        &self.diags
    }

    /// Only the unresolved references located in `full_path`.
    pub fn file_diags(&self, full_path: &Path) -> Vec<&RefDiag> {
        let Some(key) = self.key_for(full_path) else {
            return Vec::new();
        };
        // Go parity: match on the precomputed loc prefix "<full>:".
        let prefix = format!("{}:", key.full.to_string_lossy());
        self.diags
            .iter()
            .filter(|d| d.loc.starts_with(&prefix))
            .collect()
    }

    /// The facts for a tracked file, or `None` if it is not part of the project.
    pub fn facts_at(&self, full_path: &Path) -> Option<&FileFacts> {
        let key = self.key_for(full_path)?;
        self.facts.get(&key.rel)
    }

    /// Every reference to `(kind, name)` across the project, in walk order.
    pub fn references(&self, kind: SymbolKind, name: &str) -> Vec<&Ref> {
        let mut out = Vec::new();
        for key in &self.order {
            if let Some(f) = self.facts.get(&key.rel) {
                out.extend(f.refs.iter().filter(|r| r.kind == kind && r.name == name));
            }
        }
        out
    }

    /// Resolves a FileSet `rel_path` back to an on-disk full path.
    pub fn rel_to_full(&self, rel_path: &str) -> Option<&Path> {
        self.order
            .iter()
            .find(|k| k.rel == rel_path)
            .map(|k| k.full.as_path())
    }

    /// Finds the tracked key whose on-disk path matches `full_path`, compared
    /// as cleaned absolute paths (Go: `filepath.Abs` + `Clean` — no symlink
    /// resolution).
    fn key_for(&self, full_path: &Path) -> Option<FileKey> {
        let target = abs_clean(full_path)?;
        self.order
            .iter()
            .find(|k| abs_clean(&k.full).as_deref() == Some(target.as_str()))
            .cloned()
    }
}

/// Absolute + lexically cleaned form of a path, as a comparable string.
fn abs_clean(path: &Path) -> Option<String> {
    let abs = std::path::absolute(path).ok()?;
    Some(pdxl_path::clean(&abs.to_string_lossy()))
}
