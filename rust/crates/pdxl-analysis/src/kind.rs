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
/// universal to every Paradox game and is extracted by `pdxl-loc`, not the
/// per-game schema. Games reference this rather than redeclaring it.
pub const LOC_KEY: KindId = KindId::new("loc_key");

/// The kinds a game's call-by-name references resolve to (scripted effects and
/// triggers matched in *key* position, script values in *value* position).
/// Supplied by the game schema so the engine's extractor stays kind-agnostic.
#[derive(Clone, Copy, Debug)]
pub struct CallKinds {
    pub effect: KindId,
    pub trigger: KindId,
    pub value: KindId,
}
