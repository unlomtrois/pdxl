//! The facts data model: what one file *claims*.
//!
//! Ported from `internal/validate` (`Symbol`, `Ref`, `FileFacts`). A fact is a
//! small, tree-free claim extracted from a parsed file — a definition, an
//! alias, or a reference — deterministic from the file's content **and path**
//! (directory location decides what a definition means), which is what makes
//! facts independently extractable and cacheable per file.

use std::sync::Arc;

use crate::kind::KindId;

/// A single definition found in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: KindId,
    /// FileSet `RelPath` where it was defined (overlay key, not disk path).
    /// Interned per file — all symbols from one file share one allocation
    /// (localization files hold tens of thousands of keys with the same path).
    pub file: Arc<str>,
    /// Byte offset of the definition field node (go-to-definition target).
    pub offset: u32,
    /// Byte offset just past the definition name (the key scalar's end).
    pub end_offset: u32,
    /// Sorted, deduped `$PARAM$` names found in the body.
    pub params: Vec<String>,
}

/// A reference to check against the symbol table. `name` has quotes stripped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ref {
    pub kind: KindId,
    /// Alternate kinds this reference may resolve to (see `RefRule::alt`);
    /// empty for ordinary single-kind references.
    pub alt: &'static [KindId],
    pub name: String,
    /// On-disk path (editor diagnostics point at a clickable file). Interned
    /// per file — all refs from one file share one allocation.
    pub file: Arc<str>,
    /// Byte range of the referenced value.
    pub start: u32,
    pub end: u32,
}

/// Everything one file contributes to analysis. Deterministic from the file's
/// content and path, so it can be cached — or simply re-extracted — per file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFacts {
    /// Definitions this file declares (duplicate-tracked on merge).
    pub defs: Vec<Symbol>,
    /// Alias names (e.g. trait groups) — gap-fill on merge, no dup tracking.
    pub aliases: Vec<Symbol>,
    /// Filtered, resolvable references.
    pub refs: Vec<Ref>,
    /// Call-by-name references: `my_effect = yes` / `my_trigger = { … }`, whose
    /// *key* is the referenced scripted effect/trigger name (so they can't be
    /// matched by a fixed-keyword rule). Emitted only for keys that name a
    /// defined scripted effect/trigger (see [`CallTargets`]), and only nested
    /// (never at file top level, where the same name would be the definition).
    /// Never diagnosed — a call to an unknown name is indistinguishable from a
    /// builtin we don't model — so these live outside `refs` and the `check`
    /// path; they power editor find-references / CodeLens only.
    pub calls: Vec<Ref>,
}

/// The names of every defined scripted effect / trigger in the project, used to
/// recognize call-by-name references during extraction. A whole-corpus fact, so
/// it is gathered in a cheap pre-pass over the two `scripted_*` directories and
/// passed into [`crate::extract_facts`].
#[derive(Clone, Copy)]
pub struct CallTargets<'a> {
    /// The kinds these name sets resolve to (game-supplied, so the extractor
    /// stays kind-agnostic).
    pub kinds: crate::kind::CallKinds,
    /// Names of defined scripted effects — matched in *key* position
    /// (`my_effect = yes`).
    pub effects: &'a std::collections::HashSet<String>,
    /// Names of defined scripted triggers — matched in *key* position.
    pub triggers: &'a std::collections::HashSet<String>,
    /// Names of defined script values — matched in *value* position
    /// (`add_stress = minor_stress_gain`, `value = X`, list items).
    pub script_values: &'a std::collections::HashSet<String>,
}
