//! Symbol-kind identity — the open-registry replacement for the old closed
//! `SymbolKind` enum (see `rust/docs/SCHEMA-SCALING.md`, Phase 2).
//!
//! The engine treats a kind as an **opaque** identity: it compares, hashes,
//! counts, and names kinds, but never enumerates them. Each *game* crate owns
//! its vocabulary (`pdxl_ck3::kinds`), so adding a game touches no engine code.
//!
//! The representation is a private `&'static str` (the kind's report name), so a
//! `KindId` is self-describing — `name()` needs no registry. That costs a fat
//! pointer per symbol; because the type is opaque, the encoding can later become
//! a `u16` + interned-name table without changing a single call site.

/// The identity of a symbol kind. Constructed by game crates as `const`s.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KindId(&'static str);

impl KindId {
    /// Declares a kind by its stable report name (`"secret_type"`). `const`, so
    /// game crates expose one `pub const` per concept.
    pub const fn new(name: &'static str) -> KindId {
        KindId(name)
    }

    /// The stable report name — used in dumps, diagnostics, and hover.
    pub const fn name(self) -> &'static str {
        self.0
    }
}

/// Localization keys — the one kind the engine owns, since localization is
/// universal to every Paradox game and is extracted by `pdxl-yml`, not the
/// per-game schema. Games reference this rather than redeclaring it.
pub const LOC_KEY: KindId = KindId::new("loc_key");

/// Script constants (`@name = value` at file top level) — the second engine-
/// owned kind: the `@` syntax is universal PDXScript, and the symbols are
/// **file-local** (corpus-verified: zero cross-file uses in CK3 vanilla), so
/// they resolve per file and never enter the global symbol table.
pub const SCRIPT_CONSTANT: KindId = KindId::new("script_constant");

/// The kinds a game's call-by-name references resolve to (scripted effects and
/// triggers matched in *key* position, script values in *value* position).
/// Supplied by the game schema so the engine's extractor stays kind-agnostic.
#[derive(Clone, Copy, Debug)]
pub struct CallKinds {
    pub effect: KindId,
    pub trigger: KindId,
    pub value: KindId,
}

/// The kinds a game assigns to the two interface-script (`.gui`) symbol
/// roles — `template NAME { … }` and `type name = base { … }` definitions.
/// Like [`CallKinds`], the engine defines the *roles*; the game names the
/// kinds. `None` on the schema disables `.gui` analysis entirely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiKinds {
    pub template: KindId,
    pub ty: KindId,
    /// Datafunctions whose first quoted argument names a symbol of the given
    /// kind (`GetScriptedGui('x')`, `ScriptValue('v')`, `Custom('k')`).
    /// Extracted as navigation-only references from `.gui` files.
    pub arg_refs: &'static [(&'static str, KindId)],
    /// Widget properties whose quoted value is a localization key
    /// (`text = "MY_KEY"`, `tooltip = "…"`). Identifier-like values become
    /// navigation-only [`LOC_KEY`] references; prose and datafn-embedded
    /// strings are skipped.
    pub loc_fields: &'static [&'static str],
}
