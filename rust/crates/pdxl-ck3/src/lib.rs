//! CK3 rules for `pdxl-analysis` — the game's conventions as *data*.
//!
//! Originally a transcription of the Go `internal/validate/schema_ck3.go`
//! registry. Since the landed-titles addition (`ANALYSIS_VERSION` 2) this
//! schema has grown past the Go implementation — the analysis layer's Go
//! oracle is retired; regressions are pinned by golden snapshots in
//! `pdxl-parity` instead.
//!
//! Everything about one game concept lives in one file under [`entities`]:
//! its schema row(s) ([`KindSpec`] — def directory, reference shapes,
//! aliases, icon) and its structural context (see [`contexts`]). Adding a
//! concept is adding a file and one registry line — the schema-scaling
//! design (`rust/docs/SCHEMA-SCALING.md`).
//!
//! Deliberately hand-written and small: deep CK3 validation is ck3-tiger's
//! territory; this schema stays just rich enough to power editor features.
//! Bump [`pdxl_analysis::ANALYSIS_VERSION`] when a change alters what
//! previously extracted facts mean.
//!
//! [`KindSpec`]: pdxl_analysis::KindSpec

use pdxl_analysis::{Schema, SymbolKind};

mod entities;

pub mod contexts;
pub mod tables;

/// Relative-scope keywords a reference value may hold at runtime
/// (`has_trait = prev`); unresolvable without scope tracking, so skipped.
/// Game-wide — they belong to no single concept.
const SCOPE_KEYWORDS: &[&str] = &[
    "root",
    "this",
    "prev",
    "prevprev",
    "prevprevprev",
    "prevprevprevprev",
];

/// Typed-definition keywords: `KEYWORD NAME = { … }` defines `NAME` of the
/// given kind regardless of directory. CK3 uses these inline in event files
/// (`scripted_effect elope_outcome_effect = { … }`), and the same names are
/// invoked as call-by-name references (`NAME = yes`).
const TYPED_DEFS: &[(&str, SymbolKind)] = &[
    ("scripted_effect", SymbolKind::ScriptedEffect),
    ("scripted_trigger", SymbolKind::ScriptedTrigger),
];

/// Keyed-value definitions: a top-level `KEY = value` whose *value* is the
/// symbol. `namespace = X` declares event namespace `X`.
const KEYED_VALUE_DEFS: &[(&str, SymbolKind)] = &[("namespace", SymbolKind::Namespace)];

/// Builds the CK3 schema from the entity registry. Cheap to construct; build
/// once and share.
pub fn schema() -> Schema {
    Schema::new(
        &entities::kinds(),
        SCOPE_KEYWORDS,
        TYPED_DEFS,
        KEYED_VALUE_DEFS,
    )
}
