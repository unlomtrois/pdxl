//! Phase 2: merge all files' facts into a table and resolve every reference.
//!
//! Ported from `internal/validate`'s `mergeAndResolve` + `resolveRefs`. Pure
//! and in-memory — no disk I/O, no parsing. This is what makes the incremental
//! model cheap: after one file's facts are replaced, rerunning this whole phase
//! over ~3,500 tiny `FileFacts` costs almost nothing.

use std::collections::HashMap;
use std::sync::Arc;

use crate::kind::KindId;
use crate::model::{FileFacts, Ref};
use crate::table::SymbolTable;

/// How loudly a diagnostic speaks.
///
/// `Error` is the default because every reference the schema declares is one
/// the game itself will read. The exception is a smart-doc anchor: `#!` is
/// pdxl's own convention, invisible to the game, so a stale `![@key]` is worth
/// a squiggle but must never fail a build.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Severity {
    #[default]
    Error,
    Warning,
}

/// The severity an unresolved reference of `kind` is reported at. An engine
/// rule, not game data — only the engine-owned anchor kind is soft.
fn ref_severity(kind: KindId) -> Severity {
    if kind == crate::kind::DOC_ANCHOR {
        Severity::Warning
    } else {
        Severity::Error
    }
}

/// An unresolved-reference diagnostic. `file`/`start`/`end` give the on-disk
/// path and byte range of the offending value (editor ranges). The CLI's
/// `file:line:col` string is derived on demand from these (few diagnostics),
/// rather than precomputed and stored on every reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefDiag {
    pub file: Arc<str>,
    pub start: u32,
    pub end: u32,
    pub msg: String,
    pub severity: Severity,
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

    let mut diags = resolve_refs(&table, refs.iter().copied());

    // Script constants resolve file-locally: `@name` references check only
    // their own file's `@name = …` definitions (the global table never sees
    // them — the same name recurs across files with different values).
    for rel in order {
        let Some(f) = facts.get(*rel) else { continue };
        if f.constant_refs.is_empty() {
            continue;
        }
        let defined: std::collections::HashSet<&str> =
            f.constants.iter().map(|c| c.name.as_str()).collect();
        for r in &f.constant_refs {
            if !defined.contains(r.name.as_str()) {
                diags.push(RefDiag {
                    file: r.file.clone(),
                    start: r.start,
                    end: r.end,
                    msg: format!("unknown {} {:?}", r.kind.name(), r.name),
                    severity: Severity::Error,
                });
            }
        }
    }
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
        let resolved = table.lookup(r.kind, &r.name).is_some()
            || r.alt.iter().any(|&k| table.lookup(k, &r.name).is_some());
        if !resolved {
            diags.push(RefDiag {
                file: r.file.clone(),
                start: r.start,
                end: r.end,
                // Matches Go's `unknown %s %q` for the identifier-shaped names
                // that reach this point (skip rules filter the rest).
                msg: format!("unknown {} {:?}", r.kind.name(), r.name),
                severity: ref_severity(r.kind),
            });
        }
    }
    diags
}
