//! Phase 2: merge all files' facts into a table and resolve every reference.
//!
//! Ported from `internal/validate`'s `mergeAndResolve` + `resolveRefs`. Pure
//! and in-memory — no disk I/O, no parsing. This is what makes the incremental
//! model cheap: after one file's facts are replaced, rerunning this whole phase
//! over ~3,500 tiny `FileFacts` costs almost nothing.

use std::collections::HashMap;

use crate::model::{FileFacts, Ref};
use crate::table::SymbolTable;

/// An unresolved-reference diagnostic. `file`/`start`/`end` give the on-disk
/// path and byte range of the offending value (editor ranges); `loc` is the
/// precomputed `file:line:col` used by the CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefDiag {
    pub file: String,
    pub start: u32,
    pub end: u32,
    pub loc: String,
    pub msg: String,
}

/// Builds the symbol table from the gathered facts (in walk order, so a
/// duplicate's "first" is stable) and resolves all references.
///
/// Go parity: definitions and references are gathered in one pass over the
/// order (duplicates tracked), then aliases gap-fill in a second pass, then
/// every reference is looked up.
pub fn merge_and_resolve(
    order: &[&str],
    facts: &HashMap<String, FileFacts>,
) -> (SymbolTable, Vec<RefDiag>) {
    let mut table = SymbolTable::new();
    let mut refs: Vec<&Ref> = Vec::new();

    for rel in order {
        if let Some(f) = facts.get(*rel) {
            for def in &f.defs {
                table.add(def.clone());
            }
            refs.extend(f.refs.iter());
        }
    }
    for rel in order {
        if let Some(f) = facts.get(*rel) {
            for alias in &f.aliases {
                table.add_alias(alias.kind, &alias.name, alias.clone());
            }
        }
    }

    let diags = resolve_refs(&table, refs.iter().copied());
    (table, diags)
}

/// Checks each reference against the completed table, returning a diagnostic
/// for every one that does not resolve.
pub fn resolve_refs<'a>(
    table: &SymbolTable,
    refs: impl IntoIterator<Item = &'a Ref>,
) -> Vec<RefDiag> {
    let mut diags = Vec::new();
    for r in refs {
        if table.lookup(r.kind, &r.name).is_none() {
            diags.push(RefDiag {
                file: r.file.clone(),
                start: r.start,
                end: r.end,
                loc: r.loc.clone(),
                // Matches Go's `unknown %s %q` for the identifier-shaped names
                // that reach this point (skip rules filter the rest).
                msg: format!("unknown {} {:?}", r.kind.as_str(), r.name),
            });
        }
    }
    diags
}
