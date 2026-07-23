//! The whole-project symbol table: merged definitions with duplicate tracking.
//!
//! Ported from `internal/validate` (`SymbolTable`, `Duplicate`). Merge policy
//! is **first-writer-wins**: the first definition of a (kind, name) in walk
//! order is the one the table serves (go-to-definition targets it), and every
//! later redefinition is recorded in [`SymbolTable::duplicates`]. Aliases fill
//! gaps only — a name that legitimately repeats (CK3 trait groups) never
//! shadows a real definition and is never duplicate-tracked.
//!
//! Walk order matters: FileSet winner order (locked by the M3 differential)
//! feeds the merge, so "first" is stable across runs.

use std::collections::HashMap;
use std::sync::Arc;

use crate::kind::KindId;
use crate::model::Symbol;

/// A redefinition of an already-defined symbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Duplicate {
    pub kind: KindId,
    pub name: String,
    /// The previously registered (winning) definition.
    pub first: Symbol,
    /// The file that redefined it.
    pub file: Arc<str>,
}

/// All collected definitions, indexed by kind and name.
#[derive(Default)]
pub struct SymbolTable {
    by_kind: HashMap<KindId, HashMap<String, Symbol>>,
    pub duplicates: Vec<Duplicate>,
}

impl SymbolTable {
    pub fn new() -> SymbolTable {
        SymbolTable::default()
    }

    /// Registers a definition; a repeat of an existing (kind, name) is recorded
    /// as a [`Duplicate`] and does not replace the first definition.
    pub fn add(&mut self, symbol: Symbol) {
        let bucket = self.by_kind.entry(symbol.kind).or_default();
        if let Some(first) = bucket.get(&symbol.name) {
            self.duplicates.push(Duplicate {
                kind: symbol.kind,
                name: symbol.name,
                first: first.clone(),
                file: symbol.file,
            });
            return;
        }
        bucket.insert(symbol.name.clone(), symbol);
    }

    /// Registers an additional resolvable name for a kind without duplicate
    /// tracking — gap-fill only (an existing entry always wins).
    pub fn add_alias(&mut self, kind: KindId, name: &str, symbol: Symbol) {
        let bucket = self.by_kind.entry(kind).or_default();
        if !bucket.contains_key(name) {
            bucket.insert(name.to_string(), symbol);
        }
    }

    /// Number of symbols of the given kind.
    pub fn count(&self, kind: KindId) -> usize {
        self.by_kind.get(&kind).map_or(0, HashMap::len)
    }

    /// Total number of symbols across all kinds.
    pub fn total(&self) -> usize {
        self.by_kind.values().map(HashMap::len).sum()
    }

    /// The symbol of the given kind and name, if present.
    pub fn lookup(&self, kind: KindId, name: &str) -> Option<&Symbol> {
        self.by_kind.get(&kind)?.get(name)
    }

    /// All defined names of a kind, in arbitrary order (completion sources).
    pub fn names(&self, kind: KindId) -> impl Iterator<Item = &str> {
        self.by_kind
            .get(&kind)
            .into_iter()
            .flat_map(|m| m.keys().map(String::as_str))
    }

    /// Every symbol across all kinds, in arbitrary order (workspace-symbol
    /// search). Includes aliases (they share a bucket with defs).
    pub fn iter(&self) -> impl Iterator<Item = &Symbol> {
        self.by_kind.values().flat_map(HashMap::values)
    }
}
