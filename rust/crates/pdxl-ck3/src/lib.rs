//! CK3 rules for `pdxl-analysis` — the game's conventions as *data*.
//!
//! Originally a transcription of the Go `internal/validate/schema_ck3.go`
//! registry (plus the scope keywords from `resolve.go` and the trait-alias
//! keys hardcoded in `facts.go`). Since the landed-titles addition
//! (`ANALYSIS_VERSION` 2) this schema has grown past the Go implementation —
//! the analysis layer's Go oracle is retired; regressions are pinned by golden
//! snapshots in `pdxl-parity` instead.
//!
//! Everything about one game concept lives in a single [`KindSpec`] row below
//! (defs directory, reference shapes, aliases, icon) — the schema-scaling
//! design (`rust/docs/SCHEMA-SCALING.md`); adding a kind is adding a row.
//!
//! Deliberately hand-written and small: deep CK3 validation is ck3-tiger's
//! territory; this schema stays just rich enough to power editor features
//! (go-to-definition, unresolved-reference diagnostics). Grow it incrementally,
//! and bump [`pdxl_analysis::ANALYSIS_VERSION`] when a change alters what
//! previously extracted facts mean.

use pdxl_analysis::{
    DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule, Schema, SymbolKind,
};

pub mod contexts;
pub mod tables;

/// The file prefix that gates the on_action list/weighted reference rules —
/// those shapes are ambiguous elsewhere (Go: `OnActionDir`).
pub const ON_ACTION_DIR: &str = "common/on_action/";

/// Where landed titles are defined — and the only place a bare
/// `capital = c_x` attribute unambiguously names a title.
pub const LANDED_TITLES_DIR: &str = "common/landed_titles/";

/// Landed-title tier prefixes, as observed in vanilla + real mods: empire,
/// kingdom, duchy, county, barony, and the hegemony tier (`h_china`, …).
/// A key in `common/landed_titles/` is a title definition iff it starts with
/// one of these AND has a block body (which excludes loc-key decoys like
/// `cultural_names = { x = k_something }`).
pub const TITLE_TIER_PREFIXES: &[&str] = &["e_", "k_", "d_", "c_", "b_", "h_"];

/// An ungated reference rule (applies in every file).
const fn anywhere(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: None,
    }
}

/// A reference rule gated to on_action files.
const fn in_on_action(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(ON_ACTION_DIR),
    }
}

/// One row per CK3 concept the analyzer knows about.
const KIND_SPECS: &[KindSpec] = &[
    KindSpec {
        kind: SymbolKind::ScriptedTrigger,
        icon: IconHint::Function,
        defs: Some(DefSource {
            dir_prefix: "common/scripted_triggers/",
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    },
    KindSpec {
        kind: SymbolKind::ScriptedEffect,
        icon: IconHint::Function,
        defs: Some(DefSource {
            dir_prefix: "common/scripted_effects/",
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    },
    KindSpec {
        kind: SymbolKind::Trait,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: "common/traits/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("add_trait")),
            anywhere(RefPattern::KeyValue("remove_trait")),
            anywhere(RefPattern::KeyValue("has_trait")),
        ],
        // CK3 traits expose group / group_equivalence names as valid refs.
        aliases: &["group", "group_equivalence"],
    },
    KindSpec {
        kind: SymbolKind::Decision,
        icon: IconHint::Action,
        defs: Some(DefSource {
            dir_prefix: "common/decisions/",
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    },
    KindSpec {
        kind: SymbolKind::OnAction,
        icon: IconHint::Event,
        defs: Some(DefSource {
            dir_prefix: "common/on_action/",
            shape: DefShape::TopLevel,
        }),
        refs: &[in_on_action(RefPattern::KeyList("on_actions"))],
        aliases: &[],
    },
    KindSpec {
        kind: SymbolKind::Event,
        icon: IconHint::Event,
        defs: Some(DefSource {
            dir_prefix: "events/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            // Scalar form: trigger_event = ns.id.
            anywhere(RefPattern::KeyValue("trigger_event")),
            // Block form: trigger_event = { id = ns.id … }.
            anywhere(RefPattern::KeyBlockId("trigger_event")),
            // on_action lists: events = { ns.id … } (ambiguous elsewhere).
            in_on_action(RefPattern::KeyList("events")),
            in_on_action(RefPattern::KeyList("first_valid")),
            // on_action weighted blocks: random_events = { 50 = ns.id … }.
            in_on_action(RefPattern::KeyWeighted("random_events")),
        ],
        aliases: &[],
    },
    KindSpec {
        kind: SymbolKind::Character,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "history/characters/",
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    },
    KindSpec {
        kind: SymbolKind::Title,
        icon: IconHint::Hierarchy,
        defs: Some(DefSource {
            dir_prefix: LANDED_TITLES_DIR,
            shape: DefShape::Tree {
                key_prefixes: TITLE_TIER_PREFIXES,
            },
        }),
        refs: &[
            // Self-identifying scope literals: `title:e_hre`,
            // `title:k_x = { … }`, `title:e_byzantium.holder` — anywhere,
            // any position.
            anywhere(RefPattern::ScopePrefix("title")),
            // Inside title definitions, `capital = c_x` names the county a
            // title is administered from. Gated: elsewhere `capital` sets a
            // holding/realm capital by other means and is ambiguous.
            RefRule {
                pattern: RefPattern::KeyValue("capital"),
                gate: Some(LANDED_TITLES_DIR),
            },
        ],
        aliases: &[],
    },
];

/// Relative-scope references that may hold a trait at runtime;
/// unresolvable without scope tracking.
const SCOPE_KEYWORDS: &[&str] = &[
    "root",
    "this",
    "prev",
    "prevprev",
    "prevprevprev",
    "prevprevprevprev",
];

/// Builds the CK3 schema. Cheap to construct; build once and share.
pub fn schema() -> Schema {
    Schema::new(KIND_SPECS, SCOPE_KEYWORDS)
}
