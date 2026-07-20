//! Character interactions (`common/character_interactions/`) — top-level
//! `NAME = { … }` definitions (from `_character_interactions.info`). Referenced
//! by `interaction = X` (corpus-validated, 0 unresolved), and their bodies are a
//! large documented structure so the many trigger/effect/AI/loc fields complete
//! and hover.

use pdxl_analysis::context::ClauseKind::{self, DynamicDesc, Effect, ScriptValue, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use crate::kinds;

use super::Entity;
use super::common::{DURATION, OPAQUE, anywhere};

/// `cost = { gold = … piety = … prestige = … renown = … }`.
static COST: StructSpec = StructSpec {
    name: "cost",
    fields: &[
        ("gold", scalar_or_block(Setting, ScriptValue)),
        ("piety", scalar_or_block(Setting, ScriptValue)),
        ("prestige", scalar_or_block(Setting, ScriptValue)),
        ("renown", scalar_or_block(Setting, ScriptValue)),
    ],
    fallback: Fallback::Deny,
};

/// A `send_option = { … }` block: an extra toggle shown when sending.
static SEND_OPTION: StructSpec = StructSpec {
    name: "send_option",
    fields: &[
        ("is_shown", block(Trigger).doc("Is this option shown?")),
        ("is_valid", block(Trigger).doc("Is this option selectable?")),
        ("current_description", scalar_or_block(LocKey, DynamicDesc)),
        (
            "flag",
            scalar(Setting).doc("If selected, sets `scope:<flag>` to yes."),
        ),
        (
            "localization",
            scalar(LocKey).doc("Loc key for the option label."),
        ),
        (
            "starts_enabled",
            block(Trigger).doc("On by default when the window opens?"),
        ),
        ("can_be_changed", block(Trigger)),
        ("can_invalidate_interaction", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// `ai_targets = { ai_recipients = … max = … chance = … }`.
static AI_TARGETS: StructSpec = StructSpec {
    name: "ai_targets",
    fields: &[
        (
            "ai_recipients",
            scalar(Setting).doc("Which target list the AI considers (see the `ai_targets` list)."),
        ),
        (
            "max",
            scalar(Setting).doc("Max targets to consider (unset = all)."),
        ),
        (
            "chance",
            scalar(Setting).doc("0–1; randomly skips that fraction of targets (perf)."),
        ),
        ("parameter", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// `ai_frequency_by_tier = { barony = 0 county = … }` (months per tier).
static AI_FREQUENCY_BY_TIER: StructSpec = StructSpec {
    name: "ai_frequency_by_tier",
    fields: &[
        ("barony", scalar(Setting)),
        ("county", scalar(Setting)),
        ("duchy", scalar(Setting)),
        ("kingdom", scalar(Setting)),
        ("empire", scalar(Setting)),
        ("hegemony", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// `override_background = { reference = X trigger = { … } }` — the interaction
/// window's background. `reference` resolves to an event background (via the
/// shared `override_background` rule).
static OVERRIDE_BACKGROUND: StructSpec = StructSpec {
    name: "override_background",
    fields: &[
        ("reference", scalar(Setting).doc("An event-background key.")),
        ("trigger", block(Trigger)),
    ],
    fallback: Fallback::Deny,
};

/// The body of one `NAME = { … }` character-interaction definition.
static INTERACTION: StructSpec = StructSpec {
    name: "character_interaction",
    fields: &[
        // ── menu / presentation ──────────────────────────────────────────
        (
            "category",
            scalar(Setting).doc("Required. The interaction menu category (`interaction_category_*`)."),
        ),
        (
            "interface_priority",
            scalar(Setting).doc("Sort order within the menu (higher first)."),
        ),
        (
            "common_interaction",
            scalar(Setting).doc("`yes`/`no` — keep out of the More… submenu."),
        ),
        ("filter_tags", block(Struct(&OPAQUE)).doc("Tags to filter the menu by.")),
        ("icon", scalar(Setting).doc("Icon key under gfx/interface/icons/character_interactions/.")),
        ("icon_small", scalar(Setting)),
        ("alert_icon", scalar(Setting)),
        ("extra_icon", scalar(Setting)),
        ("should_use_extra_icon", block(Trigger)),
        (
            "override_background",
            block(Struct(&OVERRIDE_BACKGROUND))
                .doc("Interaction-window background (root is `scope:actor`)."),
        ),
        ("interface", scalar(Setting).doc("Specialized GUI to use (marriage, grant_titles, …).")),
        ("special_interaction", scalar(Setting)),
        ("special_ai_interaction", scalar(Setting)),
        ("scheme", scalar(Setting).doc("The scheme type this interaction starts.")),
        ("hidden", scalar(Setting)),
        ("diarch_interaction", scalar(Setting)),
        ("popup_on_receive", scalar(Setting)),
        ("pause_on_receive", scalar(Setting)),
        ("force_notification", scalar(Setting)),
        ("needs_recipient_to_open", scalar(Setting)),
        ("show_effects_in_notification", scalar(Setting)),
        ("target_type", scalar(Setting).doc("title / artifact / men_at_arms / court_position_type / count.")),
        ("target_filter", scalar(Setting)),
        ("custom_character_sort", block(Struct(&OPAQUE))),
        ("secondary_actor", scalar(Setting)),
        ("secondary_recipient", scalar(Setting)),
        ("secondary_scopes_optional", scalar(Setting)),
        ("send_options_exclusive", scalar(Setting)),
        ("send_option", block(Struct(&SEND_OPTION))),
        ("options_heading", scalar(LocKey)),
        // ── triggers ─────────────────────────────────────────────────────
        (
            "is_shown",
            block(Trigger).doc("Is the interaction visible? Scopes: scope:actor, scope:recipient."),
        ),
        (
            "is_valid",
            block(Trigger).doc("Is the interaction selectable (enabled)?"),
        ),
        ("is_valid_showing_failures_only", block(Trigger)),
        (
            "is_available",
            block(Trigger)
                .doc("Available for the actor (AI + player). Root is the actor; prefer this over is_shown for actor-only checks."),
        ),
        ("is_highlighted", block(Trigger).doc("Highlight the interaction in the menu?")),
        ("has_valid_target", block(Trigger)),
        ("has_valid_target_showing_failures_only", block(Trigger)),
        ("can_be_picked", block(Trigger).doc("Can this character be picked as a target?")),
        ("can_be_picked_title", block(Trigger)),
        ("can_be_picked_artifact", block(Trigger)),
        ("can_be_picked_regiment", block(Trigger)),
        ("can_send", block(Trigger).doc("Can the interaction be sent?")),
        ("can_be_blocked", block(Trigger)),
        ("needs_confirmation", block(Trigger)),
        ("ignore_recipient_recieve_cooldown", block(Trigger)),
        (
            "auto_accept",
            scalar_or_block(Setting, Trigger).doc("`yes`/`no` or a trigger — is it auto-accepted?"),
        ),
        ("use_diplomatic_range", scalar_or_block(Setting, Trigger)),
        // ── cooldowns / cost ─────────────────────────────────────────────
        ("cooldown", block(Struct(&DURATION)).doc("Reuse cooldown (`{ years = x }`).")),
        ("cooldown_against_recipient", block(Struct(&DURATION))),
        ("category_cooldown", block(Struct(&DURATION))),
        ("category_cooldown_against_recipient", block(Struct(&DURATION))),
        (
            "cost",
            block(Struct(&COST)).doc("Scripted cost paid by the actor when sent."),
        ),
        // ── effects ──────────────────────────────────────────────────────
        ("on_send", block(Effect).doc("Runs immediately when the interaction is sent.")),
        ("on_accept", block(Effect).doc("Runs when the recipient accepts.")),
        ("on_decline", block(Effect).doc("Runs when the recipient declines.")),
        ("on_blocked_effect", block(Effect)),
        ("pre_auto_accept", block(Effect)),
        ("on_auto_accept", block(Effect)),
        ("on_intermediary_accept", block(Effect)),
        ("on_intermediary_decline", block(Effect)),
        ("on_decline_summary", scalar_or_block(LocKey, DynamicDesc)),
        ("redirect", block(Effect).doc("Reassign actor/recipient/intermediary scopes.")),
        ("populate_actor_list", block(Effect).doc("Fill the `characters` list of pickable actors.")),
        ("populate_recipient_list", block(Effect)),
        ("localization_values", block(Effect)),
        // ── AI ───────────────────────────────────────────────────────────
        ("ai_accept", block(ScriptValue).doc("MTTH: will the AI accept this interaction?")),
        ("ai_intermediary_accept", block(ScriptValue)),
        ("ai_will_do", block(ScriptValue).doc("MTTH: how interested the AI is in sending it (0–100).")),
        ("ai_potential", block(Trigger).doc("Deprecated — use is_available.")),
        ("ai_set_target", block(Effect)),
        ("ai_targets", block(Struct(&AI_TARGETS))),
        ("ai_target_quick_trigger", block(Struct(&OPAQUE))),
        ("ai_frequency", scalar(Setting)),
        ("ai_frequency_by_tier", block(Struct(&AI_FREQUENCY_BY_TIER))),
        ("ai_instant_response", scalar(Setting)),
        ("ai_accept_negotiation", scalar(Setting)),
        ("ai_maybe", scalar(Setting)),
        ("ai_intermediary_maybe", scalar(Setting)),
        ("ai_min_reply_days", scalar(Setting)),
        ("ai_max_reply_days", scalar(Setting)),
        ("can_send_despite_rejection", scalar(Setting)),
        ("ignores_pending_interaction_block", scalar(Setting)),
        // ── text (loc keys) ──────────────────────────────────────────────
        ("desc", scalar_or_block(LocKey, DynamicDesc)),
        ("greeting", scalar(Setting).doc("`positive` / `negative` — tone of the request text.")),
        ("highlighted_reason", scalar_or_block(LocKey, DynamicDesc)),
        ("send_name", scalar(LocKey)),
        ("prompt", scalar(LocKey)),
        ("notification_text", scalar(LocKey)),
        ("intermediary_notification_text", scalar(LocKey)),
        ("reply_item_key", scalar(LocKey)),
        ("pre_answer_yes_key", scalar(LocKey)),
        ("pre_answer_no_key", scalar(LocKey)),
        ("pre_answer_maybe_key", scalar(LocKey)),
        ("pre_answer_yes_breakdown_key", scalar(LocKey)),
        ("pre_answer_no_breakdown_key", scalar(LocKey)),
        ("pre_answer_maybe_breakdown_key", scalar(LocKey)),
        ("intermediary_breakdown_yes", scalar(LocKey)),
        ("intermediary_breakdown_no", scalar(LocKey)),
        ("intermediary_breakdown_maybe", scalar(LocKey)),
        ("intermediary_answer_accept_key", scalar(LocKey)),
        ("intermediary_answer_reject_key", scalar(LocKey)),
        ("answer_block_key", scalar(LocKey)),
        ("answer_accept_key", scalar(LocKey)),
        ("answer_reject_key", scalar(LocKey)),
        ("answer_acknowledge_key", scalar(LocKey)),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct CharacterInteraction;

impl Entity for CharacterInteraction {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::CHARACTER_INTERACTION,
            icon: IconHint::Action,
            defs: Some(DefSource {
                dir_prefix: "common/character_interactions/",
                shape: DefShape::TopLevel,
            }),
            // `interaction = X` resolves everywhere (important_actions,
            // decisions, scripted effects, events, …); 0 unresolved.
            refs: &[anywhere(RefPattern::KeyValue("interaction"))],
            aliases: &[],
        },
        // Interaction categories (`common/character_interaction_categories/`),
        // named by an interaction's `category = X`. Gated there because `category`
        // is overloaded (traits, activities, portraits, …).
        KindSpec {
            kind: kinds::INTERACTION_CATEGORY,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: "common/character_interaction_categories/",
                shape: DefShape::TopLevel,
            }),
            refs: &[RefRule {
                pattern: RefPattern::KeyValue("category"),
                gate: Some("common/character_interactions/"),
            }],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(
        "common/character_interactions/",
        ClauseKind::Struct(&INTERACTION),
    )];
}
