//! Game rules (`common/game_rules/`, from `_game_rules.info`) — top-level
//! rule blocks whose block-valued children (minus the `categories`
//! attribute) are the rule's **settings**, the referenced symbols
//! ([`DefShape::GroupedBlocks`], the laws precedent; 299 in vanilla).
//!
//! References (corpus-validated, 0 unresolved):
//! - `has_game_rule = X` — the trigger, script-wide (752 refs);
//! - `default = X` — depth-1 inside a rule body, gated to the dir.
//!
//! A setting's `apply_modifier = <category>:<modifier>` names a static
//! modifier — that rule lives in [`super::modifier`]. Localization is
//! implicit (`rule_<key>`, `setting_<key>`, `setting_<key>_desc`,
//! `game_rule_category_<category>`), so no loc refs fire here.
//!
//! `blocks_achievements` is corpus-real but missing from the `.info`'s
//! flag list.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::{Setting, Target};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{OPAQUE, anywhere};

const GAME_RULES_DIR: &str = "common/game_rules/";

/// The body of one setting: modifiers and engine flags.
static GAME_RULE_SETTING: StructSpec = StructSpec {
    name: "game rule setting",
    fields: &[
        (
            "apply_modifier",
            scalar(Target).doc(
                "Apply a modifier to characters matching a category — \
                 `player:X`, `ai:X`, or `all:X` (repeatable).",
            ),
        ),
        (
            "flag",
            scalar(Setting)
                .doc(
                    "An engine flag with a hardcoded effect (repeatable). \
                     `blocks_achievements` is corpus-real but undocumented in the \
                     `.info` flag list.",
                )
                .values(&[
                    "blocks_achievements",
                    "no_end_date",
                    "no_diplomatic_range",
                    "restricted_diplomatic_range",
                    "advantage_damage_effect_1",
                    "advantage_damage_effect_2",
                    "advantage_damage_effect_5",
                    "advantage_damage_effect_7",
                    "advantage_damage_effect_10",
                ]),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one game rule: attributes plus arbitrarily-named settings.
static GAME_RULE: StructSpec = StructSpec {
    name: "game rule",
    fields: &[
        (
            "categories",
            block(Struct(&OPAQUE)).doc(
                "The categories this rule is listed under (localized as \
                 `game_rule_category_<category>`).",
            ),
        ),
        (
            "default",
            scalar(Setting).doc("The setting this rule defaults to."),
        ),
    ],
    // Unknown block-valued keys are the rule's settings.
    fallback: Fallback::Struct(&GAME_RULE_SETTING),
};

pub(crate) struct GameRule;

impl Entity for GameRule {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::GAME_RULE_SETTING,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: GAME_RULES_DIR,
            shape: DefShape::GroupedBlocks {
                exclude: &["categories"],
            },
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("has_game_rule")),
            RefRule {
                pattern: RefPattern::KeyValueTop("default"),
                gate: Some(GAME_RULES_DIR),
                alt: &[],
            },
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(GAME_RULES_DIR, ClauseKind::Struct(&GAME_RULE))];
}
