//! Static modifiers (`common/modifiers/`) — top-level `NAME = { icon = …
//! <stat> = <value> … }` definitions, applied to a target via an
//! `add_<scope>_modifier` effect.
//!
//! Two reference shapes, both `add_*_modifier`-gated so the very common bare
//! `modifier =` / `type =` keys elsewhere are never touched:
//! - block form `add_character_modifier = { modifier = X … }` — the inner field
//!   is `modifier` for every scope except `add_scheme_modifier`, which (with a
//!   handful of `add_character_modifier` uses) names it `type`;
//! - scalar shorthand `add_character_modifier = X`.
//!
//! Corpus-validated at ~0.7% unresolved (macro-interpolated names), so no gate
//! beyond the add-key is needed.

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

/// Block form `add_<scope>_modifier = { modifier = X … }`.
const fn block_modifier(add_key: &'static str) -> RefRule {
    anywhere(RefPattern::KeyBlockField(add_key, "modifier"))
}

/// Scalar shorthand `add_<scope>_modifier = X`.
const fn scalar_modifier(add_key: &'static str) -> RefRule {
    anywhere(RefPattern::KeyValue(add_key))
}

pub(crate) struct Modifier;

impl Entity for Modifier {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::MODIFIER,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/modifiers/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            // Block form, `{ modifier = X }` — every scope but scheme.
            block_modifier("add_character_modifier"),
            block_modifier("add_county_modifier"),
            block_modifier("add_province_modifier"),
            block_modifier("add_house_modifier"),
            block_modifier("add_dynasty_modifier"),
            block_modifier("add_travel_plan_modifier"),
            block_modifier("add_legend_owner_modifier"),
            block_modifier("add_legend_county_modifier"),
            block_modifier("add_legend_province_modifier"),
            // Block form, `{ type = X }` — schemes (and some character uses).
            anywhere(RefPattern::KeyBlockField("add_scheme_modifier", "type")),
            anywhere(RefPattern::KeyBlockField("add_character_modifier", "type")),
            // Scalar shorthand `add_*_modifier = X`.
            scalar_modifier("add_character_modifier"),
            scalar_modifier("add_artifact_modifier"),
            scalar_modifier("add_county_modifier"),
            scalar_modifier("add_province_modifier"),
            scalar_modifier("add_house_modifier"),
            scalar_modifier("add_dynasty_modifier"),
            scalar_modifier("add_travel_plan_modifier"),
            // Game-rule settings apply modifiers as `apply_modifier =
            // <category>:<modifier>` (categories: player/ai/all) — the
            // colon literal form, gated to the game-rules dir (19 corpus
            // refs, 0 unresolved).
            RefRule {
                pattern: RefPattern::ScopePrefix("player"),
                gate: Some("common/game_rules/"),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::ScopePrefix("ai"),
                gate: Some("common/game_rules/"),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::ScopePrefix("all"),
                gate: Some("common/game_rules/"),
                alt: &[],
            },
        ],
        aliases: &[],
    }];

    // A definition body is a static-modifier clause: keys are built-in modifier
    // tags, so the editor can complete and document them.
    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[("common/modifiers/", ClauseKind::StaticModifier)];
}
