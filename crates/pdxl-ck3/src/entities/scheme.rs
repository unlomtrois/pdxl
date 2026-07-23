//! Schemes (`common/schemes/scheme_types/`): schema row (scheme-type
//! references) plus the `_schemes.info` structural context.
//!
//! A scheme's key is its type id (`elope`, `murder`, …); it is referenced as
//! `scheme_type = X`, `start_scheme = { type = X }`, and the bare `scheme = X`
//! trigger idiom.
//!
//! The text fields `desc` / `success_desc` / `discovery_desc` are loc-key
//! references (rules live in `loc.rs`, gated to this dir, the target-kind
//! precedent). The rule also catches the `desc = X` tooltip keys inside the
//! scripted-modifier blocks (`base_success_chance`, `agent_join_chance`).
//! Corpus: 97.8% of 743 resolve; the misses exist in no language, genuine
//! dead-loc bugs the tool now surfaces.
//!
//! The `_schemes.info` warns it "does not currently include all possible
//! parameters"; the body here is reconciled against the live corpus (82
//! scheme types across game + T4N). Fields present in the corpus but absent
//! from the info: `on_start`, `phases_per_agent_charge`, `discovery_desc`,
//! `base_maximum_success` (the info's `base_maximum_success_chance` is not
//! used), `starting_agent_slots`. Enum values are corpus-derived too:
//! `category` gained `political` beyond the info's personal/contract/hostile.
//!
//! Sibling dirs `common/schemes/{pulse_actions,agent_types,scheme_countermeasures}/`
//! are their own kinds (not modeled yet); modeling `pulse_actions` would turn
//! this body's `pulse_actions.entries` list into references.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, Effect, ScriptValue, ScriptedModifier, Trigger,
};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{DURATION, OPAQUE, anywhere};
use pdxl_analysis::context::FieldSpec;

/// A `yes`/`no` toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

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
            scalar(Setting)
                .doc("What skill to base scheme speed on.")
                .values(&[
                    "diplomacy",
                    "martial",
                    "stewardship",
                    "intrigue",
                    "learning",
                    "prowess",
                ]),
        ),
        (
            "category",
            scalar(Setting)
                .doc(
                    "Scheme category. Hostile schemes use hostile_scheme_resistance/speed; \
                     personal & contract use personal_scheme_*.",
                )
                .values(&["personal", "contract", "hostile", "political"]),
        ),
        (
            "target_type",
            scalar(Setting)
                .doc("The type of scope the scheme targets.")
                .values(&["character", "title", "culture", "faith", "nothing"]),
        ),
        (
            "hostile",
            toggle("(Deprecated) marks the scheme hostile. Prefer category."),
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
        ("discovery_desc", scalar_or_block(LocKey, DynamicDesc)),
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
            "phases_per_agent_charge",
            scalar(Setting)
                .doc("How many scheme phases each agent charge lasts. (Corpus field, not in the info docs.)"),
        ),
        (
            "starting_agent_slots",
            scalar(Setting)
                .doc("Number of agent slots the scheme starts with. (Corpus field, not in the info docs.)"),
        ),
        (
            "uses_resistance",
            toggle(
                "If no, the target's modifiers/skill/spymaster/tier are ignored when computing \
                 phase speed.",
            ),
        ),
        (
            "is_basic",
            toggle(
                "A basic scheme has no success-chance growth per phase, no agents, and no \
                 opportunities.",
            ),
        ),
        (
            "valid_agent",
            block_scoped(Trigger, "character").doc(
                "Trigger checking whether an agent is valid for the scheme. Checked frequently; \
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
        // The live corpus uses the shorter `base_maximum_success` (82×); keep
        // both so either spelling completes.
        ("base_maximum_success", scalar(Setting)),
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
            toggle("If yes, secrecy mechanics apply; otherwise secrecy is always 100%."),
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
            "on_start",
            block(Effect)
                .doc("Runs when the scheme starts. (Corpus hook, not in the info docs.)"),
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
        ("freeze_scheme_when_traveling", toggle("Freeze the scheme when the schemer starts traveling.")),
        ("freeze_scheme_when_traveling_target", toggle("Freeze the scheme when the scheme target starts traveling.")),
        ("cancel_scheme_when_traveling", toggle("Cancel the scheme when the schemer starts traveling.")),
        ("cancel_scheme_when_traveling_target", toggle("Cancel the scheme when the scheme target starts traveling.")),
        (
            "hide_target_name",
            toggle("Whether to hide the target when showing the scheme name."),
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
        kind: kinds::SCHEME,
        icon: IconHint::Action,
        defs: Some(DefSource {
            dir_prefix: "common/schemes/scheme_types/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            // `scheme_type = elope`: the common trigger idiom.
            anywhere(RefPattern::KeyValue("scheme_type")),
            // Bare `scheme = elope`, checks/switches on the scheme type. Scope
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
