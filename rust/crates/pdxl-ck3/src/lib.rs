//! CK3 rules for `pdxl-analysis` — the game's conventions as *data*.
//!
//! Originally a transcription of the Go `internal/validate/schema_ck3.go`
//! registry (plus the scope keywords from `resolve.go` and the trait-alias
//! keys hardcoded in `facts.go`). Since the landed-titles addition
//! (`ANALYSIS_VERSION` 2) this schema has grown past the Go implementation —
//! the analysis layer's Go oracle is retired; regressions are pinned by golden
//! snapshots in `pdxl-parity` instead.
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

/// Landed-title tier prefixes, as observed in vanilla + real mods: empire,
/// kingdom, duchy, county, barony, and the hegemony tier (`h_china`, …).
/// A key in `common/landed_titles/` is a title definition iff it starts with
/// one of these AND has a block body (which excludes loc-key decoys like
/// `cultural_names = { x = k_something }`).
pub const TITLE_TIER_PREFIXES: &[&str] = &["e_", "k_", "d_", "c_", "b_", "h_"];

/// Builds the CK3 schema. Cheap to construct; build once and share.
pub fn schema() -> Schema {
    Schema {
        // Directories whose fields define symbols. All rules harvest top-level
        // `NAME = { … }` fields except landed titles, which form a tree.
        def_rules: vec![
            DefRule {
                prefix: "common/scripted_triggers/",
                kind: SymbolKind::ScriptedTrigger,
                nested_key_prefixes: None,
            },
            DefRule {
                prefix: "common/scripted_effects/",
                kind: SymbolKind::ScriptedEffect,
                nested_key_prefixes: None,
            },
            DefRule {
                prefix: "common/traits/",
                kind: SymbolKind::Trait,
                nested_key_prefixes: None,
            },
            DefRule {
                prefix: "common/decisions/",
                kind: SymbolKind::Decision,
                nested_key_prefixes: None,
            },
            DefRule {
                prefix: "common/on_action/",
                kind: SymbolKind::OnAction,
                nested_key_prefixes: None,
            },
            DefRule {
                prefix: "events/",
                kind: SymbolKind::Event,
                nested_key_prefixes: None,
            },
            DefRule {
                prefix: "history/characters/",
                kind: SymbolKind::Character,
                nested_key_prefixes: None,
            },
            DefRule {
                prefix: "common/landed_titles/",
                kind: SymbolKind::Title,
                nested_key_prefixes: Some(TITLE_TIER_PREFIXES),
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
        // Self-identifying scope literals: `title:e_hre`, `title:k_x = { … }`,
        // `title:e_byzantium.holder` — anywhere, any position.
        scope_ref_prefixes: vec![("title", SymbolKind::Title)],
    }
}
