//! CK3 rules for `pdxl-analysis` — a data-only transcription of the Go
//! `internal/validate/schema_ck3.go` registry (plus the scope keywords from
//! `resolve.go` and the trait-alias keys hardcoded in `facts.go`).
//!
//! Deliberately hand-written and small: deep CK3 validation is ck3-tiger's
//! territory; this schema stays just rich enough to power editor features
//! (go-to-definition, unresolved-reference diagnostics). Grow it incrementally,
//! and bump [`pdxl_analysis::ANALYSIS_VERSION`] when a change alters what
//! previously extracted facts mean.

use pdxl_analysis::{DefRule, Schema, SymbolKind};

/// The file prefix under which list/weighted reference rules apply
/// (Go: `OnActionDir`).
pub const ON_ACTION_DIR: &str = "common/on_action/";

/// Builds the CK3 schema. Cheap to construct; build once and share.
pub fn schema() -> Schema {
    Schema {
        // Directories whose top-level `NAME = { … }` fields are definitions.
        def_rules: vec![
            DefRule {
                prefix: "common/scripted_triggers/",
                kind: SymbolKind::ScriptedTrigger,
            },
            DefRule {
                prefix: "common/scripted_effects/",
                kind: SymbolKind::ScriptedEffect,
            },
            DefRule {
                prefix: "common/traits/",
                kind: SymbolKind::Trait,
            },
            DefRule {
                prefix: "common/decisions/",
                kind: SymbolKind::Decision,
            },
            DefRule {
                prefix: "common/on_action/",
                kind: SymbolKind::OnAction,
            },
            DefRule {
                prefix: "events/",
                kind: SymbolKind::Event,
            },
            DefRule {
                prefix: "history/characters/",
                kind: SymbolKind::Character,
            },
        ],
        // key = value — the scalar value must resolve to the kind.
        ref_rules: [
            ("add_trait", SymbolKind::Trait),
            ("remove_trait", SymbolKind::Trait),
            ("has_trait", SymbolKind::Trait),
            ("trigger_event", SymbolKind::Event), // scalar form: trigger_event = ns.id
        ]
        .into(),
        // key = { id = X … } — X must resolve to the kind.
        block_id_ref_rules: [("trigger_event", SymbolKind::Event)].into(),
        // on_action lists: events = { ns.id … } (ambiguous outside on_action).
        list_ref_rules: [
            ("events", SymbolKind::Event),
            ("first_valid", SymbolKind::Event),
            ("on_actions", SymbolKind::OnAction),
        ]
        .into(),
        // on_action weighted blocks: random_events = { 50 = ns.id … }.
        weighted_ref_rules: [("random_events", SymbolKind::Event)].into(),
        list_gate_prefix: ON_ACTION_DIR,
        // CK3 traits expose group / group_equivalence names as valid refs.
        alias_keys: [(
            SymbolKind::Trait,
            &["group", "group_equivalence"] as &[&str],
        )]
        .into(),
        // Relative-scope references that may hold a trait at runtime;
        // unresolvable without scope tracking.
        scope_keywords: [
            "root",
            "this",
            "prev",
            "prevprev",
            "prevprevprev",
            "prevprevprevprev",
        ]
        .into(),
    }
}
