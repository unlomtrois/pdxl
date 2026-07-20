//! The facts data model: what one file *claims*.
//!
//! Ported from `internal/validate` (`Symbol`, `Ref`, `FileFacts`). A fact is a
//! small, tree-free claim extracted from a parsed file — a definition, an
//! alias, or a reference — deterministic from the file's content **and path**
//! (directory location decides what a definition means), which is what makes
//! facts independently extractable and cacheable per file.

use std::sync::Arc;

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
    Culture = 8,
    Faith = 9,
    /// Localization keys (`localization/<lang>/**/*.yml`). Defined outside
    /// PDXScript entirely; extracted by `pdxl-loc`, not the schema engine.
    LocKey = 10,
    /// Realm/title laws (`common/laws/`): block children of law groups.
    Law = 11,
    /// Schemes (`common/schemes/scheme_types/`): top-level scheme definitions.
    Scheme = 12,
    /// Event backgrounds (`common/event_backgrounds/`): top-level background
    /// definitions, referenced by `background = { reference = X }`.
    EventBackground = 13,
    /// Event themes (`common/event_themes/`): top-level theme definitions,
    /// referenced by the event `theme = X` keyword.
    EventTheme = 14,
    /// Static modifiers (`common/modifiers/`): top-level definitions, referenced
    /// by `add_*_modifier = { modifier|type = X }` blocks and scalar shorthand.
    Modifier = 15,
    /// Script values (`common/script_values/`): top-level `NAME = <number>` or
    /// `NAME = { <formula> }` definitions, referenced by name in any
    /// number-accepting value position (`add_stress = minor_stress_gain`).
    ScriptValue = 16,
    /// Portrait animations (`gfx/portraits/portrait_animations/`): top-level
    /// definitions, referenced by an event portrait's `animation = X`.
    PortraitAnimation = 17,
    /// Scripted character templates (`common/scripted_character_templates/`):
    /// top-level definitions, referenced by `create_character = { template = X }`.
    ScriptedCharacterTemplate = 18,
    /// An event namespace declaration (`namespace = X` at file top level). The
    /// symbol is the declaration itself (its value), so hovering it shows the
    /// file's doc; the events that use the namespace are unaffected.
    Namespace = 19,
}

impl SymbolKind {
    /// Every kind, in discriminant order (stable iteration for reports).
    pub const ALL: [SymbolKind; 20] = [
        SymbolKind::ScriptedTrigger,
        SymbolKind::ScriptedEffect,
        SymbolKind::Trait,
        SymbolKind::Event,
        SymbolKind::Decision,
        SymbolKind::OnAction,
        SymbolKind::Character,
        SymbolKind::Title,
        SymbolKind::Culture,
        SymbolKind::Faith,
        SymbolKind::LocKey,
        SymbolKind::Law,
        SymbolKind::Scheme,
        SymbolKind::EventBackground,
        SymbolKind::EventTheme,
        SymbolKind::Modifier,
        SymbolKind::ScriptValue,
        SymbolKind::PortraitAnimation,
        SymbolKind::ScriptedCharacterTemplate,
        SymbolKind::Namespace,
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
            SymbolKind::Culture => "culture",
            SymbolKind::Faith => "faith",
            SymbolKind::LocKey => "loc_key",
            SymbolKind::Law => "law",
            SymbolKind::Scheme => "scheme",
            SymbolKind::EventBackground => "event_background",
            SymbolKind::EventTheme => "event_theme",
            SymbolKind::Modifier => "modifier",
            SymbolKind::ScriptValue => "script_value",
            SymbolKind::PortraitAnimation => "portrait_animation",
            SymbolKind::ScriptedCharacterTemplate => "scripted_character_template",
            SymbolKind::Namespace => "namespace",
        }
    }
}

/// A single definition found in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
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
    pub kind: SymbolKind,
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
    /// Names of defined `scripted_effect`s — matched in *key* position
    /// (`my_effect = yes`).
    pub effects: &'a std::collections::HashSet<String>,
    /// Names of defined `scripted_trigger`s — matched in *key* position.
    pub triggers: &'a std::collections::HashSet<String>,
    /// Names of defined script values — matched in *value* position
    /// (`add_stress = minor_stress_gain`, `value = X`, list items).
    pub script_values: &'a std::collections::HashSet<String>,
}
