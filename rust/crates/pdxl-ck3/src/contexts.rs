//! CK3 structural-context specs: which directory produces which root
//! context, and what event / option / decision / on_action blocks look like.
//!
//! Hand-distilled from the game's own structure docs (each directory ships a
//! `_*.info` file — `_events.info`, `_decisions.info`, `_on_actions.info`),
//! cross-checked against tiger's bespoke validators. See
//! `rust/docs/STRUCTURAL-CONTEXTS.md` for the model and the findings these
//! specs encode (the option block's inline-effect fallback, value-form
//! forking, per-context key meaning).

use pdxl_analysis::context::{
    ClauseKind, ContextSchema, Fallback, ScalarKind, StructSpec, block, block_scoped, scalar,
    scalar_or_block,
};

use ClauseKind::{DynamicDesc, Effect, ScriptValue, ScriptedModifier, Trigger};
use ScalarKind::{LocKey, Setting, Target};

/// A block whose contents we don't model (controller payloads, role maps).
static OPAQUE: StructSpec = StructSpec {
    name: "opaque",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// `trigger` + `reference` blocks (`picture`, every event `override_*`).
static TRIGGERED_ASSET: StructSpec = StructSpec {
    name: "triggered_asset",
    fields: &[
        ("trigger", block(Trigger)),
        ("reference", scalar(Setting)),
        ("soundeffect", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// `days/weeks/months/years = <script value>` (cooldowns, delays).
static DURATION: StructSpec = StructSpec {
    name: "duration",
    fields: &[
        ("days", scalar_or_block(Setting, ScriptValue)),
        ("weeks", scalar_or_block(Setting, ScriptValue)),
        ("months", scalar_or_block(Setting, ScriptValue)),
        ("years", scalar_or_block(Setting, ScriptValue)),
    ],
    fallback: Fallback::Deny,
};

// ── events (`_events.info`) ─────────────────────────────────────────────────

static TRIGGERED_ANIMATION: StructSpec = StructSpec {
    name: "triggered_animation",
    fields: &[
        ("trigger", block(Trigger)),
        ("animation", scalar(Setting)),
        ("scripted_animation", scalar(Setting)),
        ("camera", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

static TRIGGERED_OUTFIT: StructSpec = StructSpec {
    name: "triggered_outfit",
    fields: &[
        ("trigger", block(Trigger)),
        (
            "outfit_tags",
            scalar_or_block(Setting, ClauseKind::Struct(&OPAQUE)),
        ),
        ("remove_default_outfit", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// The block form of `left_portrait = { … }` et al. Its `trigger` runs in
/// the *portrait character's* scope (layer-3 note in the design doc).
static PORTRAIT: StructSpec = StructSpec {
    name: "portrait",
    fields: &[
        ("character", scalar(Target)),
        ("trigger", block(Trigger)),
        ("animation", scalar(Setting)),
        ("scripted_animation", scalar(Setting)),
        (
            "triggered_animation",
            block(ClauseKind::Struct(&TRIGGERED_ANIMATION)),
        ),
        (
            "triggered_outfit",
            block(ClauseKind::Struct(&TRIGGERED_OUTFIT)),
        ),
        ("camera", scalar(Setting)),
        ("override_imprisonment_visuals", scalar(Setting)),
        ("animate_if_dead", scalar(Setting)),
        (
            "outfit_tags",
            scalar_or_block(Setting, ClauseKind::Struct(&OPAQUE)),
        ),
        ("remove_default_outfit", scalar(Setting)),
        ("hide_info", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

static ARTIFACT: StructSpec = StructSpec {
    name: "artifact",
    fields: &[
        ("target", scalar(Target)),
        ("position", scalar(Setting)),
        ("trigger", block(Trigger)),
    ],
    fallback: Fallback::Deny,
};

static COURT_SCENE: StructSpec = StructSpec {
    name: "court_scene",
    fields: &[
        ("button_position_character", scalar(Target)),
        ("court_owner", scalar(Target)),
        ("court_event_force_open", scalar(Setting)),
        ("show_timeout_info", scalar(Setting)),
        ("should_pause_time", scalar(Setting)),
        // Role keys are dynamic (`scope:x = { role = … }`).
        ("roles", block(ClauseKind::Struct(&OPAQUE))),
    ],
    fallback: Fallback::Deny,
};

static WIDGET: StructSpec = StructSpec {
    name: "widget",
    fields: &[
        ("is_shown", block(Trigger)),
        ("gui", scalar(Setting)),
        ("container", scalar(Setting)),
        (
            "controller",
            scalar_or_block(Setting, ClauseKind::Struct(&OPAQUE)),
        ),
        ("setup_scope", block(Effect)),
    ],
    fallback: Fallback::Deny,
};

static WIDGETS: StructSpec = StructSpec {
    name: "widgets",
    fields: &[("widget", block(ClauseKind::Struct(&WIDGET)))],
    fallback: Fallback::Deny,
};

/// `name = { text = … trigger = { … } }` — the gated option-name candidate.
static OPTION_NAME: StructSpec = StructSpec {
    name: "option_name",
    fields: &[
        ("text", scalar_or_block(LocKey, DynamicDesc)),
        ("trigger", block(Trigger)),
    ],
    fallback: Fallback::Deny,
};

/// The event option: known structural fields, and — the key finding from
/// `_events.info` — **every unknown key is an inline effect**.
static OPTION: StructSpec = StructSpec {
    name: "option",
    fields: &[
        (
            "name",
            scalar_or_block(LocKey, ClauseKind::Struct(&OPTION_NAME)),
        ),
        ("trigger", block(Trigger)),
        ("show_as_unavailable", block(Trigger)),
        ("highlight_portrait", scalar(Target)),
        ("reason", scalar(Setting)),
        ("skill", scalar(Setting)),
        ("trait", scalar(Setting)),
        ("show_unlock_reason", scalar(Setting)),
        ("is_cancel_option", scalar(Setting)),
        ("clicksound", scalar(Setting)),
        ("fallback", scalar(Setting)),
        ("exclusive", scalar(Setting)),
        ("ai_chance", block(ScriptedModifier)),
        ("ai_will_select", block(ScriptValue)),
        ("custom_tooltip", scalar(LocKey)),
    ],
    fallback: Fallback::Effect,
};

static EVENT: StructSpec = StructSpec {
    name: "event",
    fields: &[
        ("type", scalar(Setting)),
        ("scope", scalar(Setting)),
        ("window", scalar(Setting)),
        ("hidden", scalar(Setting)),
        ("major", scalar(Setting)),
        ("orphan", scalar(Setting)),
        ("content_source", scalar(Setting)),
        ("theme", scalar(Setting)),
        ("title", scalar_or_block(LocKey, DynamicDesc)),
        ("desc", scalar_or_block(LocKey, DynamicDesc)),
        ("opening", scalar_or_block(LocKey, DynamicDesc)),
        ("trigger", block(Trigger)),
        ("major_trigger", block(Trigger)),
        ("immediate", block(Effect)),
        ("after", block(Effect)),
        ("on_trigger_fail", block(Effect)),
        ("cooldown", block(ClauseKind::Struct(&DURATION))),
        (
            "left_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "right_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "center_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "lower_left_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "lower_center_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "lower_right_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "sender",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        ("artifact", block(ClauseKind::Struct(&ARTIFACT))),
        ("court_scene", block(ClauseKind::Struct(&COURT_SCENE))),
        ("widgets", block(ClauseKind::Struct(&WIDGETS))),
        ("widget", block(ClauseKind::Struct(&WIDGET))),
        ("option", block(ClauseKind::Struct(&OPTION))),
        (
            "override_background",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        (
            "override_transition",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        (
            "override_effect_2d",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        ("override_icon", block(ClauseKind::Struct(&TRIGGERED_ASSET))),
        (
            "override_header_background",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        (
            "override_sound",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
    ],
    fallback: Fallback::Deny,
};

// ── decisions (`_decisions.info`) ───────────────────────────────────────────

static DECISION_ITEM: StructSpec = StructSpec {
    name: "decision_widget_item",
    fields: &[
        ("value", scalar(Setting)),
        ("is_shown", block(Trigger)),
        ("is_valid", block(Trigger)),
        ("current_description", scalar_or_block(LocKey, DynamicDesc)),
        ("localization", scalar(LocKey)),
        ("is_default", scalar(Setting)),
        ("icon", scalar(Setting)),
        ("flat", scalar(Setting)),
        ("ai_chance", scalar_or_block(Setting, ScriptValue)),
    ],
    fallback: Fallback::Deny,
};

static DECISION_WIDGET: StructSpec = StructSpec {
    name: "decision_widget",
    fields: &[
        ("gui", scalar(Setting)),
        ("controller", scalar(Setting)),
        ("show_from_start", scalar(Setting)),
        ("item", block(ClauseKind::Struct(&DECISION_ITEM))),
    ],
    fallback: Fallback::Deny,
};

static COST: StructSpec = StructSpec {
    name: "cost",
    fields: &[
        ("gold", scalar_or_block(Setting, ScriptValue)),
        ("piety", scalar_or_block(Setting, ScriptValue)),
        ("prestige", scalar_or_block(Setting, ScriptValue)),
    ],
    fallback: Fallback::Deny,
};

static DECISION: StructSpec = StructSpec {
    name: "decision",
    fields: &[
        ("title", scalar_or_block(LocKey, DynamicDesc)),
        ("desc", scalar_or_block(LocKey, DynamicDesc)),
        ("selection_tooltip", scalar_or_block(LocKey, DynamicDesc)),
        ("confirm_text", scalar_or_block(LocKey, DynamicDesc)),
        (
            "picture",
            scalar_or_block(Setting, ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        ("extra_picture", scalar(Setting)),
        ("decision_group_type", scalar(Setting)),
        ("major", scalar(Setting)),
        ("sort_order", scalar(Setting)),
        ("progress", scalar_or_block(Setting, ScriptValue)),
        ("advice", block(ClauseKind::Struct(&OPAQUE))),
        ("ai_goal", scalar(Setting)),
        ("ai_check_interval", scalar(Setting)),
        (
            "ai_check_interval_by_tier",
            block(ClauseKind::Struct(&OPAQUE)),
        ),
        ("is_shown", block(Trigger)),
        ("is_valid", block(Trigger)),
        ("is_valid_showing_failures_only", block(Trigger)),
        ("should_create_alert", block(Trigger)),
        ("cost", block(ClauseKind::Struct(&COST))),
        ("minimum_cost", block(ClauseKind::Struct(&COST))),
        ("effect", block(Effect)),
        ("ai_potential", block(Trigger)),
        ("ai_will_do", block(ScriptedModifier)),
        (
            "widget",
            scalar_or_block(Setting, ClauseKind::Struct(&DECISION_WIDGET)),
        ),
    ],
    fallback: Fallback::Deny,
};

// ── on_actions (`_on_actions.info`) ─────────────────────────────────────────

/// `events = { id delay = { … } id }` — loose items are event/on_action ids.
static FIRE_LIST: StructSpec = StructSpec {
    name: "fire_list",
    fields: &[("delay", block(ClauseKind::Struct(&DURATION)))],
    fallback: Fallback::Ignore,
};

/// `random_events = { chance_to_happen = 25  100 = id }` — weight keys are
/// dynamic numbers.
static WEIGHTED_FIRE_LIST: StructSpec = StructSpec {
    name: "weighted_fire_list",
    fields: &[
        ("chance_to_happen", scalar(Setting)),
        ("chance_of_no_event", scalar_or_block(Setting, ScriptValue)),
        ("delay", block(ClauseKind::Struct(&DURATION))),
    ],
    fallback: Fallback::Ignore,
};

static ON_ACTION: StructSpec = StructSpec {
    name: "on_action",
    fields: &[
        ("trigger", block(Trigger)),
        ("weight_multiplier", block(ScriptedModifier)),
        ("events", block(ClauseKind::Struct(&FIRE_LIST))),
        ("first_valid", block(ClauseKind::Struct(&FIRE_LIST))),
        ("on_actions", block(ClauseKind::Struct(&FIRE_LIST))),
        (
            "first_valid_on_action",
            block(ClauseKind::Struct(&FIRE_LIST)),
        ),
        (
            "random_events",
            block(ClauseKind::Struct(&WEIGHTED_FIRE_LIST)),
        ),
        (
            "random_on_actions",
            block(ClauseKind::Struct(&WEIGHTED_FIRE_LIST)),
        ),
        ("effect", block(Effect)),
        ("fallback", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

// ── laws (`_laws.info`) ─────────────────────────────────────────────────────

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

/// A single law inside a law group. Root scopes are per `_laws.info`: the
/// ruler (`character`) for most fields, the `title` for the title checks.
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

// ── the directory table ─────────────────────────────────────────────────────

static CONTEXTS: ContextSchema = ContextSchema {
    roots: &[
        ("events/", ClauseKind::Struct(&EVENT)),
        ("common/decisions/", ClauseKind::Struct(&DECISION)),
        ("common/on_action/", ClauseKind::Struct(&ON_ACTION)),
        ("common/scripted_effects/", Effect),
        ("common/scripted_triggers/", Trigger),
        ("common/script_values/", ScriptValue),
        ("common/scripted_modifiers/", ScriptedModifier),
        ("common/laws/", ClauseKind::Struct(&LAW_GROUP)),
    ],
};

/// The CK3 structural-context schema (static data; free to share).
pub fn context_schema() -> &'static ContextSchema {
    &CONTEXTS
}
