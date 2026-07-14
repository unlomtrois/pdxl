//! The facts data model: what one file *claims*.
//!
//! Ported from `internal/validate` (`Symbol`, `Ref`, `FileFacts`). A fact is a
//! small, tree-free claim extracted from a parsed file — a definition, an
//! alias, or a reference — deterministic from the file's content **and path**
//! (directory location decides what a definition means), which is what makes
//! facts independently extractable and cacheable per file.

/// The type of a defined symbol. Variants, discriminants, and [`as_str`]
/// (`SymbolKind::as_str`) names match Go's `SymbolKind` / `String()` exactly.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    ScriptedTrigger = 0,
    ScriptedEffect = 1,
    Trait = 2,
    Event = 3,
    Decision = 4,
    OnAction = 5,
    Character = 6,
    /// Landed titles (`common/landed_titles/` tree). First post-parity kind:
    /// not present in the Go implementation.
    Title = 7,
}

impl SymbolKind {
    /// Every kind, in discriminant order (stable iteration for reports).
    pub const ALL: [SymbolKind; 8] = [
        SymbolKind::ScriptedTrigger,
        SymbolKind::ScriptedEffect,
        SymbolKind::Trait,
        SymbolKind::Event,
        SymbolKind::Decision,
        SymbolKind::OnAction,
        SymbolKind::Character,
        SymbolKind::Title,
    ];

    /// The report name, identical to Go's `SymbolKind.String()`.
    pub const fn as_str(self) -> &'static str {
        match self {
            SymbolKind::ScriptedTrigger => "scripted_trigger",
            SymbolKind::ScriptedEffect => "scripted_effect",
            SymbolKind::Trait => "trait",
            SymbolKind::Event => "event",
            SymbolKind::Decision => "decision",
            SymbolKind::OnAction => "on_action",
            SymbolKind::Character => "character",
            SymbolKind::Title => "title",
        }
    }
}

/// A single definition found in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// FileSet `RelPath` where it was defined (overlay key, not disk path).
    pub file: String,
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
    pub kind: SymbolKind,
    pub name: String,
    /// On-disk path (editor diagnostics point at a clickable file).
    pub file: String,
    /// Byte range of the referenced value.
    pub start: u32,
    pub end: u32,
    /// Precomputed `file:line:col` for the CLI.
    pub loc: String,
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
}
