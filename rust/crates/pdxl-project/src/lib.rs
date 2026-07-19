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
//! - No `FactStore` (see the M5 benchmark; facts re-extract in one cheap walk).
//! - The AST cache is opt-in via [`analyze_with`]; real-corpus measurement
//!   (`docs/BASELINE.md`) showed it does NOT pay for one-shot `check` runs
//!   (warm ≈ cold: both read+hash every file, and decoding stored trees costs
//!   as much as parsing) — Go's fast warm path was its tiny-entry FactStore.
//!   The CLI therefore does not use it; it remains available for consumers
//!   with different access patterns.
//! - The schema is an explicit parameter (Go links the CK3 registry directly).

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use std::collections::HashSet;

use pdxl_analysis::{
    CallTargets, FileFacts, Ref, RefDiag, Schema, SymbolKind, SymbolTable, extract_calls,
    extract_facts, merge_and_resolve,
};
use pdxl_fileset::FileSet;

/// The localization language consumers opt the FileSet into
/// (`set_localization_language`) for loc-key symbols.
pub const DEFAULT_LOC_LANGUAGE: &str = "english";

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
    analyze_with(fs, schema, None)
}

/// Like [`analyze`], but parse results flow through the syntax cache when one
/// is supplied: an unchanged file's tree is reconstructed from the cache entry
/// instead of re-parsed, and misses populate the cache for the next run.
///
/// Real-corpus measurement (see `docs/BASELINE.md`): this does NOT speed up
/// one-shot `check`-style runs (warm ≈ cold at CK3 scale). It exists for
/// consumers whose access pattern differs (e.g. re-analyzing a subset).
pub fn analyze_with(
    fs: &FileSet,
    schema: &Schema,
    cache: Option<&pdxl_cache::Store>,
) -> io::Result<(SymbolTable, Vec<RefDiag>)> {
    let (order, facts, _names) = gather_facts(fs, schema, cache)?;
    let rels: Vec<&str> = order.iter().map(|k| k.rel.as_str()).collect();
    Ok(merge_and_resolve(&rels, &facts))
}

/// Walks `fs` once, obtaining every winning file's tree (via the cache when
/// supplied, else by parsing) and extracting its facts.
fn gather_facts(
    fs: &FileSet,
    schema: &Schema,
    cache: Option<&pdxl_cache::Store>,
) -> io::Result<(Vec<FileKey>, HashMap<String, FileFacts>, CallNames)> {
    // Pass 1: definitions + references (no calls yet).
    //
    // Fast path: with no cache, every winning file is independent — read,
    // parse, and extract are pure per file — so fan them out across cores.
    // Order is preserved (rayon's indexed collect keeps input order), which
    // duplicate detection in `merge_and_resolve` depends on. The cached path
    // stays sequential (the Store is shared mutable state).
    let (order, mut facts) = if cache.is_none() {
        gather_facts_parallel(fs, schema)?
    } else {
        let mut order = Vec::new();
        let mut facts = HashMap::new();
        fs.try_for_each(|entry| -> io::Result<()> {
            let full = entry.full_path.to_string_lossy().into_owned();
            let f = if entry.rel_path.ends_with(".yml") {
                let src = std::fs::read(&entry.full_path)?;
                loc_facts(&src, &entry.rel_path)
            } else {
                let tree = obtain_tree(&entry.full_path, &full, cache)?;
                extract_facts(&tree, &entry.rel_path, &full, schema, None)
            };
            order.push(FileKey {
                rel: entry.rel_path.clone(),
                full: entry.full_path.clone(),
            });
            facts.insert(entry.rel_path.clone(), f);
            Ok(())
        })?;
        (order, facts)
    };

    // The callable-name set — every defined scripted effect/trigger, including
    // inline typed defs harvested from event files — is a whole-project fact,
    // known only now that every file's definitions are in. Pass 2 re-derives
    // each file's call-by-name references against it.
    let names = CallNames::from_facts(&facts);
    fill_calls(&order, &mut facts, cache, &names.targets())?;
    Ok((order, facts, names))
}

/// Owns the scripted effect/trigger name sets (a whole-corpus fact). Borrowed as
/// [`CallTargets`] for extraction; kept on the [`Project`] so incremental
/// re-extraction of one file recognizes the same calls.
#[derive(Default)]
struct CallNames {
    effects: HashSet<String>,
    triggers: HashSet<String>,
}

impl CallNames {
    fn targets(&self) -> CallTargets<'_> {
        CallTargets {
            effects: &self.effects,
            triggers: &self.triggers,
        }
    }

    /// Collects every scripted effect/trigger definition name from gathered
    /// facts (directory-defined and inline typed defs alike).
    fn from_facts(facts: &HashMap<String, FileFacts>) -> CallNames {
        let mut names = CallNames::default();
        for f in facts.values() {
            for def in &f.defs {
                match def.kind {
                    SymbolKind::ScriptedEffect => {
                        names.effects.insert(def.name.clone());
                    }
                    SymbolKind::ScriptedTrigger => {
                        names.triggers.insert(def.name.clone());
                    }
                    _ => {}
                }
            }
        }
        names
    }
}

/// Pass 2: re-parse each script file and record its call-by-name references
/// against the now-complete callable-name set. Cheap relative to pass 1 (`.yml`
/// files are skipped; the cached path reuses stored trees).
fn fill_calls(
    order: &[FileKey],
    facts: &mut HashMap<String, FileFacts>,
    cache: Option<&pdxl_cache::Store>,
    targets: &CallTargets,
) -> io::Result<()> {
    // Nothing can be called if no scripted symbols are defined.
    if targets.effects.is_empty() && targets.triggers.is_empty() {
        return Ok(());
    }
    if cache.is_none() {
        use rayon::prelude::*;
        let calls: Vec<(usize, Vec<pdxl_analysis::Ref>)> = order
            .par_iter()
            .enumerate()
            .filter(|(_, key)| !key.rel.ends_with(".yml"))
            .map(|(i, key)| -> io::Result<(usize, Vec<pdxl_analysis::Ref>)> {
                let full = key.full.to_string_lossy().into_owned();
                let src = std::fs::read(&key.full)?;
                let (tree, _) = pdxl_parser::parse(full.clone(), src).into_parts();
                Ok((i, extract_calls(&tree, &full, targets)))
            })
            .collect::<io::Result<Vec<_>>>()?;
        for (i, file_calls) in calls {
            if let Some(f) = facts.get_mut(&order[i].rel) {
                f.calls = file_calls;
            }
        }
        return Ok(());
    }
    for key in order {
        if key.rel.ends_with(".yml") {
            continue;
        }
        let full = key.full.to_string_lossy().into_owned();
        let tree = obtain_tree(&key.full, &full, cache)?;
        if let Some(f) = facts.get_mut(&key.rel) {
            f.calls = extract_calls(&tree, &full, targets);
        }
    }
    Ok(())
}

/// Data-parallel pass-1 gather for the no-cache path: one rayon task per winning
/// file, each doing the pure read → parse → extract pipeline (no calls — those
/// need the whole-project name set), then a single sequential fold that rebuilds
/// `order` (FileSet insertion order) and the facts map.
fn gather_facts_parallel(
    fs: &FileSet,
    schema: &Schema,
) -> io::Result<(Vec<FileKey>, HashMap<String, FileFacts>)> {
    use rayon::prelude::*;

    let entries: Vec<&pdxl_fileset::FileEntry> = fs.iter().collect();
    let results: Vec<(FileKey, FileFacts)> = entries
        .par_iter()
        .map(|entry| -> io::Result<(FileKey, FileFacts)> {
            let full = entry.full_path.to_string_lossy().into_owned();
            let f = if entry.rel_path.ends_with(".yml") {
                let src = std::fs::read(&entry.full_path)?;
                loc_facts(&src, &entry.rel_path)
            } else {
                let src = std::fs::read(&entry.full_path)?;
                let (tree, _) = pdxl_parser::parse(full.clone(), src).into_parts();
                extract_facts(&tree, &entry.rel_path, &full, schema, None)
            };
            Ok((
                FileKey {
                    rel: entry.rel_path.clone(),
                    full: entry.full_path.clone(),
                },
                f,
            ))
        })
        .collect::<io::Result<Vec<_>>>()?;

    let mut order = Vec::with_capacity(results.len());
    let mut facts = HashMap::with_capacity(results.len());
    for (key, f) in results {
        facts.insert(key.rel.clone(), f);
        order.push(key);
    }
    Ok((order, facts))
}

/// Extracts loc-key definitions from a localization `.yml` file (routed by
/// extension: those entries only exist when the FileSet was opted in via
/// `set_localization_language`). Files without an `l_<lang>:` header yield
/// empty facts.
fn loc_facts(src: &[u8], rel_path: &str) -> FileFacts {
    let mut facts = FileFacts::default();
    let Some(loc) = pdxl_loc::parse(src) else {
        return facts;
    };
    // One shared allocation for all of this file's keys — a localization file
    // holds tens of thousands, and this is 73% of the corpus's symbols.
    let file: std::sync::Arc<str> = std::sync::Arc::from(rel_path);
    for e in loc.entries {
        facts.defs.push(pdxl_analysis::Symbol {
            name: e.key,
            kind: SymbolKind::LocKey,
            file: file.clone(),
            offset: e.key_start,
            end_offset: e.key_end,
            params: Vec::new(),
        });
    }
    facts
}

/// Returns the syntax tree for a file: a cache hit when possible, otherwise a
/// fresh parse (which populates the cache). Mirrors Go's `parseEntry`.
fn obtain_tree(
    path: &Path,
    full: &str,
    cache: Option<&pdxl_cache::Store>,
) -> io::Result<std::sync::Arc<pdxl_parser::SyntaxTree>> {
    if let Some(store) = cache {
        let mtime = pdxl_cache::Store::mtime_nanos(&std::fs::metadata(path)?);
        if let Some(hit) = store.get(path, mtime) {
            return Ok(hit.tree);
        }
        let src = std::fs::read(path)?;
        let (tree, diags) = pdxl_parser::parse(full.to_string(), src.clone()).into_parts();
        let parse = pdxl_cache::CachedParse {
            tree: std::sync::Arc::new(tree),
            diagnostics: diags.into(),
        };
        // Best-effort: a cache write failure must not fail the analysis.
        let _ = store.put(path, mtime, &src, parse.clone());
        return Ok(parse.tree);
    }
    let src = std::fs::read(path)?;
    let (tree, _) = pdxl_parser::parse(full.to_string(), src).into_parts();
    Ok(std::sync::Arc::new(tree))
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
    /// Scripted effect/trigger names, so incremental re-extraction of one file
    /// recognizes the same call-by-name references as the initial build.
    call_names: CallNames,
}

impl Project {
    /// Gathers facts for every winning file in `fs` and builds the initial
    /// table and diagnostics.
    pub fn new(fs: &FileSet, schema: Schema) -> io::Result<Project> {
        let (order, facts, call_names) = gather_facts(fs, &schema, None)?;
        let mut p = Project {
            schema,
            order,
            facts,
            table: SymbolTable::new(),
            diags: Vec::new(),
            call_names,
        };
        p.rebuild();
        Ok(p)
    }

    /// The schema this project was built with (e.g. for presentation hints).
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The FileSet `RelPath` key of a project file (drives directory-keyed
    /// features: structural contexts, def rules).
    pub fn rel_at(&self, full_path: &Path) -> Option<&str> {
        self.order
            .iter()
            .find(|k| k.full == full_path)
            .map(|k| k.rel.as_str())
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
        let facts = if key.rel.ends_with(".yml") {
            loc_facts(&src, &key.rel)
        } else {
            let full = key.full.to_string_lossy().into_owned();
            let parsed = pdxl_parser::parse(full.clone(), src);
            extract_facts(
                parsed.tree(),
                &key.rel,
                &full,
                &self.schema,
                Some(&self.call_names.targets()),
            )
        };
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
        // The diagnostic's file is the on-disk path — match it directly
        // (equivalent to the old "<full>:" loc-prefix match).
        let full = key.full.to_string_lossy();
        self.diags
            .iter()
            .filter(|d| d.file.as_ref() == full)
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
                // Call-by-name references (scripted effect/trigger invocations)
                // live outside `refs`; surface them too. They only exist for the
                // scripted kinds, so the kind filter naturally scopes this.
                out.extend(f.calls.iter().filter(|r| r.kind == kind && r.name == name));
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
