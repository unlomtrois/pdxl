//! Schemes (`common/schemes/scheme_types/`) — schema row (scheme-type
//! references) plus the `_schemes.info` structural context.
//!
//! A scheme's key is its type id (`elope`, `murder`, …); it is referenced as
//! `scheme_type = X`, `start_scheme = { type = X }`, and the bare `scheme = X`
//! trigger idiom.

use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, Effect, ScriptValue, ScriptedModifier, Trigger,
};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, SymbolKind};

use super::Entity;
use super::common::{DURATION, OPAQUE, anywhere};

/// `pulse_actions = { entries = { a b } chance_of_no_event = 0 }`.
static PULSE_ACTIONS: StructSpec = StructSpec {
    name: "pulse_actions",
    fields: &[
        // Space-separated list of scheme-pulse-action keys (not modeled).
        ("entries", block(ClauseKind::Struct(&OPAQUE))),
        ("chance_of_no_event", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// A scheme type body. Root scope is the scheme's owner (a `character`) for the
/// gating triggers; the `on_*` hooks run in the scheme scope (left unscoped, as
/// they reach the owner via `scheme_owner`/`scope:owner`).
static SCHEME: StructSpec = StructSpec {
    name: "scheme",
    fields: &[
        (
            "skill",
            scalar(Setting).doc("What skill to base scheme speed on."),
        ),
        (
            "category",
            scalar(Setting).doc(
                "Scheme category: personal / contract / hostile. Hostile schemes use \
                 hostile_scheme_resistance/speed; personal & contract use personal_scheme_*.",
            ),
        ),
        (
            "target_type",
            scalar(Setting).doc(
                "The type of scope the scheme targets: character / title / culture / faith / \
                 nothing.",
            ),
        ),
        (
            "hostile",
            scalar(Setting).doc("(Deprecated) marks the scheme hostile. Prefer category."),
        ),
        (
            "icon",
            scalar(Setting).doc("Icon key. Defaults to the scheme key."),
        ),
        (
            "illustration",
            scalar(Setting).doc("Texture used in scheme windows."),
        ),
        ("desc", scalar_or_block(LocKey, DynamicDesc)),
        ("success_desc", scalar_or_block(LocKey, DynamicDesc)),
        (
            "allow",
            block_scoped(Trigger, "character")
                .doc("When this scheme can be started. Root = owner."),
        ),
        (
            "valid",
            block_scoped(Trigger, "character").doc(
                "Conditions that must hold for the scheme to keep going; if invalid it ends. \
                 Checked daily.",
            ),
        ),
        (
            "agent_join_threshold",
            scalar(Setting).doc("How much the AI must want to join in order to actually join."),
        ),
        (
            "agent_leave_threshold",
            scalar(Setting).doc("If AI desire falls below this, the agent auto-leaves."),
        ),
        (
            "uses_resistance",
            scalar(Setting).doc(
                "If no, the target's modifiers/skill/spymaster/tier are ignored when computing \
                 phase speed.",
            ),
        ),
        (
            "is_basic",
            scalar(Setting).doc(
                "A basic scheme has no success-chance growth per phase, no agents, and no \
                 opportunities.",
            ),
        ),
        (
            "valid_agent",
            block_scoped(Trigger, "character").doc(
                "Trigger checking whether an agent is valid for the scheme. Checked frequently — \
                 use sparingly.",
            ),
        ),
        (
            "agent_groups_owner_perspective",
            block(ClauseKind::Struct(&OPAQUE)).doc(
                "Groups of characters considered for agent slots (from the owner's perspective).",
            ),
        ),
        (
            "agent_groups_target_character_perspective",
            block(ClauseKind::Struct(&OPAQUE)).doc(
                "Same groups as agent_groups_owner_perspective, fetched around the target \
                 character.",
            ),
        ),
        (
            "odds_prediction",
            block_scoped(ScriptValue, "character")
                .doc("Script value (0-100) approximating how likely the scheme is to go well."),
        ),
        (
            "agent_join_chance",
            block_scoped(ScriptedModifier, "character")
                .doc("How much the AI wants to join (agent as root)."),
        ),
        (
            "agent_success_chance",
            block_scoped(ScriptedModifier, "character").doc(
                "Each agent adds this to the scheme's success chance. Same scopes as \
                 agent_join_chance (agent as root).",
            ),
        ),
        (
            "base_success_chance",
            block_scoped(ScriptedModifier, "scheme").doc(
                "Base success chance. Root = scheme; scope:target is the target; \
                 scope:target_title exists when targeting a title.",
            ),
        ),
        ("base_maximum_success_chance", scalar(Setting)),
        ("minimum_success", scalar(Setting)),
        ("maximum_secrecy", scalar(Setting)),
        ("minimum_secrecy", scalar(Setting)),
        ("base_progress_goal", scalar(Setting)),
        (
            "maximum_breaches",
            scalar(Setting).doc("Number of secrecy breaches before the scheme forcibly ends."),
        ),
        ("pulse_actions", block(ClauseKind::Struct(&PULSE_ACTIONS))),
        (
            "cooldown",
            block(ClauseKind::Struct(&DURATION)).doc(
                "After the scheme ends, the minimum days before the same scheme type can be used \
                 by the owner on the target.",
            ),
        ),
        (
            "is_secret",
            scalar(Setting)
                .doc("If yes, secrecy mechanics apply; otherwise secrecy is always 100%."),
        ),
        (
            "use_secrecy",
            block_scoped(Trigger, "character").doc(
                "Trigger for schemes that are sometimes secret; if false, secrecy is set to 100%.",
            ),
        ),
        (
            "base_secrecy",
            scalar(Setting).doc(
                "Base for the monthly expose check: base_secrecy + success_chance + modifiers, \
                 clamped 0-100.",
            ),
        ),
        (
            "on_phase_completed",
            block(Effect).doc("Runs when the scheme phase completes (progress reached its max)."),
        ),
        (
            "on_hud_click",
            block(Effect).doc("Runs when the scheme is clicked in the bottom HUD."),
        ),
        (
            "on_monthly",
            block(Effect).doc("Runs once a month while the scheme exists."),
        ),
        (
            "on_semiyearly",
            block(Effect).doc("Runs twice a year while the scheme exists."),
        ),
        (
            "on_invalidated",
            block(Effect).doc("Runs if the scheme invalidates (see valid)."),
        ),
        ("freeze_scheme_when_traveling", scalar(Setting)),
        ("freeze_scheme_when_traveling_target", scalar(Setting)),
        ("cancel_scheme_when_traveling", scalar(Setting)),
        ("cancel_scheme_when_traveling_target", scalar(Setting)),
        (
            "hide_target_name",
            scalar(Setting).doc("Whether to hide the target when showing the scheme name."),
        ),
        ("speed_per_skill_point", scalar(Setting)),
        ("speed_per_target_skill_point", scalar(Setting)),
        ("success_chance_growth_per_skill_point", scalar(Setting)),
        ("spymaster_speed_per_skill_point", scalar(Setting)),
        ("target_spymaster_speed_per_skill_point", scalar(Setting)),
        ("tier_speed", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Scheme;

impl Entity for Scheme {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::Scheme,
        icon: IconHint::Action,
        defs: Some(DefSource {
            dir_prefix: "common/schemes/scheme_types/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            // `scheme_type = elope` — the common trigger idiom.
            anywhere(RefPattern::KeyValue("scheme_type")),
            // Bare `scheme = elope` — checks/switches on the scheme type. Scope
            // values (`scheme = scope:x`) are skipped by skip_ref_value.
            anywhere(RefPattern::KeyValue("scheme")),
            // `start_scheme = { type = elope … }` / `create_scheme = { type = … }`.
            anywhere(RefPattern::KeyBlockField("start_scheme", "type")),
            anywhere(RefPattern::KeyBlockField("create_scheme", "type")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[("common/schemes/scheme_types/", ClauseKind::Struct(&SCHEME))];
}
