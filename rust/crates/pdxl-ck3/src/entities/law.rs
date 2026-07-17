//! Realm/title laws (`common/laws/`) — grouped-block schema (law groups →
//! laws) plus the `_laws.info` structural context, with per-field root
//! scopes and hover documentation.

use pdxl_analysis::context::ClauseKind::{self, Effect, ScriptValue, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule, SymbolKind};

use super::Entity;
use super::common::{COST, OPAQUE, anywhere};

/// Where realm/title laws are defined; also gates the in-group
/// `default = law_name` reference.
const LAWS_DIR: &str = "common/laws/";

/// `triggered_flag = { trigger = { … } flag = … }` on a law.
static TRIGGERED_FLAG: StructSpec = StructSpec {
    name: "triggered_flag",
    fields: &[
        ("trigger", block_scoped(Trigger, "character")),
        ("flag", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// A law's `succession = { … }` rules (all enum/key/bool settings).
static SUCCESSION: StructSpec = StructSpec {
    name: "succession",
    fields: &[
        ("order_of_succession", scalar(Setting)),
        ("title_division", scalar(Setting)),
        ("traversal_order", scalar(Setting)),
        ("rank", scalar(Setting)),
        ("pool_character_config", scalar(Setting)),
        ("election_type", scalar(Setting)),
        ("appointment_type", scalar(Setting)),
        ("gender_law", scalar(Setting)),
        ("faith", scalar(Setting)),
        ("create_primary_tier_titles", scalar(Setting)),
        ("primary_heir_minimum_share", scalar(Setting)),
        ("exclude_rulers", scalar(Setting)),
        ("limit_to_courtiers", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// A single law inside a law group. Root scopes and docs are per
/// `_laws.info`: the ruler (`character`) for most fields, the `landed_title`
/// for the title checks.
static LAW: StructSpec = StructSpec {
    name: "law",
    fields: &[
        (
            "can_keep",
            block_scoped(Trigger, "character").doc(
                "Requirements for keeping the law. If this invalidates, the law will be \
                 replaced with the default law within a month. Also checked after changing \
                 faith since doctrinal changes are likely to invalidate laws. Always true if \
                 not specified. Root scope = ruler with the law.",
            ),
        ),
        (
            "can_have",
            block_scoped(Trigger, "character").doc(
                "Requirements for adopting the law in the ruler's scope. If true, the character \
                 is allowed to adopt the law and it shows as available (but may be disabled if \
                 can_pass is false). Always true if not specified. Root scope = ruler.",
            ),
        ),
        (
            "can_pass",
            block_scoped(Trigger, "character").doc(
                "Requirements for adopting the law, for more temporary conditions (e.g. being \
                 at war — 'I can have the law, but can't pass it right now'). Always true if \
                 not specified. Root scope = ruler.",
            ),
        ),
        (
            "should_start_with",
            block_scoped(Trigger, "character").doc(
                "If these conditions are true, this is a valid law for a ruler to start with. \
                 Always includes the can_keep check. Root scope = ruler.",
            ),
        ),
        (
            "can_title_have",
            block_scoped(Trigger, "landed_title").doc(
                "Requirements for titles being able to have this law. Always false if not \
                 specified. Root scope = title.",
            ),
        ),
        (
            "can_realm_have",
            block_scoped(Trigger, "character").doc(
                "Requirements for characters being able to apply this law at realm level. \
                 Always false if not specified. Some succession orders (inheritance, theocracy, \
                 company, generate, appointment) imply realm application by default. \
                 Root scope = character.",
            ),
        ),
        (
            "should_show_for_title",
            block_scoped(Trigger, "landed_title")
                .doc("Should this law be shown in the UI for titles? Root scope = title."),
        ),
        (
            "pass_cost",
            block_scoped(ClauseKind::Struct(&COST), "character")
                .doc("The cost of enacting this law. Root scope = ruler wanting to pass it."),
        ),
        (
            "revoke_cost",
            block_scoped(ClauseKind::Struct(&COST), "character").doc(
                "The cost of revoking or clearing this law. Root scope = ruler wanting to \
                 revoke it.",
            ),
        ),
        // A character-modifier block (`tag = value` pairs); tags are the
        // modifiers.log domain, not modeled as a context here.
        (
            "modifier",
            block(ClauseKind::Struct(&OPAQUE))
                .doc("Modifier applied to the ruler when this law is active."),
        ),
        (
            "flag",
            scalar(Setting).doc(
                "A flag; some have special meaning in code. Checkable in script with \
                 has_realm_law_flag = <flag>.",
            ),
        ),
        (
            "triggered_flag",
            block(ClauseKind::Struct(&TRIGGERED_FLAG)).doc(
                "Checks and adds a flag only if the trigger's condition is met. Both trigger \
                 and flag must be specified within the block.",
            ),
        ),
        (
            "shown_in_encyclopedia",
            scalar(Setting).doc("Whether this law shows up in the Encyclopedia. default = yes."),
        ),
        (
            "on_pass",
            block_scoped(Effect, "character").doc(
                "Effect run just before law change, on the ruler when the law is added. Does \
                 NOT run when default laws are initialized, nor when inheriting a law. \
                 Root = ruler; on a title, the title is accessible as scope:title.",
            ),
        ),
        (
            "on_after_pass",
            block_scoped(Effect, "character").doc(
                "Effect run just after law change, on the ruler when the law is added. Does \
                 NOT run when default laws are initialized, nor when inheriting a law. \
                 Root = ruler; on a title, the title is accessible as scope:title.",
            ),
        ),
        (
            "on_revoke",
            block_scoped(Effect, "character").doc(
                "Effect run on the ruler when the law is removed. Does NOT run when the law is \
                 removed due to inheriting a law. Root = ruler; on a title, scope:title.",
            ),
        ),
        (
            "succession",
            block(ClauseKind::Struct(&SUCCESSION)).doc(
                "Succession rules. Any new law with a rule set overrides the previous law's \
                 rule set, in law definition order.",
            ),
        ),
        (
            "ai_will_do",
            block_scoped(ScriptValue, "character").doc(
                "Script value in the ruler scope. If above 0, the AI will enact this law if \
                 able (checked in RARE_TASK_TICK). If multiple laws are possible, the AI enacts \
                 the highest-scoring one. Root scope = ruler.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// A top-level law group; its arbitrarily-named block children are laws.
static LAW_GROUP: StructSpec = StructSpec {
    name: "law_group",
    fields: &[
        (
            "default",
            scalar(Setting).doc(
                "New rulers use this law by default, provided its should_start_with trigger \
                 returns true or is undefined.",
            ),
        ),
        (
            "cumulative",
            scalar(Setting).doc(
                "If set, each subsequent law in the group provides all effects of the previous \
                 law. default = no.",
            ),
        ),
        (
            "flag",
            scalar(Setting).doc(
                "A law-group flag; some have special code treatment. Checkable via \
                 LawGroup.HasFlag('flag').",
            ),
        ),
        (
            "is_treasury_budget_group",
            scalar(Setting).doc(
                "If set, this group is part of the Treasury Budget set and shown in the budget \
                 interface. default = no.",
            ),
        ),
        (
            "can_change_law_group",
            block(Trigger).doc(
                "Optional trigger: rulers who fail it still see the law group but can't change \
                 it. Empty/undefined = always true.",
            ),
        ),
    ],
    fallback: Fallback::Struct(&LAW),
};

pub(crate) struct Law;

impl Entity for Law {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::Law,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: LAWS_DIR,
            // Top-level law groups; their block children are laws, minus the
            // one block-valued group attribute.
            shape: DefShape::GroupedBlocks {
                exclude: &["can_change_law_group"],
            },
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("has_realm_law")),
            anywhere(RefPattern::KeyValue("add_realm_law")),
            anywhere(RefPattern::KeyValue("add_realm_law_skip_effects")),
            anywhere(RefPattern::KeyValue("remove_realm_law")),
            // A group's `default = law_name` names a law in that group.
            // Gated: `default` means other things outside laws files.
            RefRule {
                pattern: RefPattern::KeyValue("default"),
                gate: Some(LAWS_DIR),
            },
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[("common/laws/", ClauseKind::Struct(&LAW_GROUP))];
}
