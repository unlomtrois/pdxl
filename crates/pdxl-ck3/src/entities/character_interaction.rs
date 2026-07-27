//! Character interactions (`common/character_interactions/`) — top-level
//! `NAME = { … }` definitions (from `_character_interactions.info`). Referenced
//! by `interaction = X` (corpus-validated, 0 unresolved), and their bodies are a
//! large documented structure so the many trigger/effect/AI/loc fields complete
//! and hover.
//!
//! The body is reconciled against the live corpus (58 files, game + T4N): every
//! depth-1 key in use is modeled. Corpus-only fields absent from the info are
//! marked `*(corpus)*` — `name`, `recipient_recieve_cooldown` (the engine's own
//! misspelling, alongside the info's `ignore_recipient_recieve_cooldown`),
//! `shows_military_strength`, and the `cost` currencies `influence` / `treasury`
//! / `treasury_or_gold`. The info's `cost` lists only gold/piety/prestige/renown,
//! yet `influence` outnumbers `gold` — and since `COST` denies unknown keys, that
//! gap was a dead spot rather than a soft one.
//!
//! Enum vocabularies come from the info's FAQ and `ai_targets` appendices
//! (`target_filter`, `ai_recipients`, `target_type`), each with the corpus-only
//! additions the info missed (`recipient_lessee_titles`, `diarch`).
//! `interface` / `special_interaction` are engine-side with no enumerable list,
//! so their `values()` are purely corpus-derived.
//!
//! Deliberate omissions: `filter_tags` and `custom_character_sort` hold
//! bare-word lists, not key/value pairs, so they stay opaque with their
//! vocabularies in the field docs. Filter tags imply `<tag>_filter_tag_desc` loc
//! keys, but the tags are not defs of any kind, so no implicit-loc pattern
//! applies. `scheme = X` already resolves via the ungated rule in `scheme.rs`,
//! and `override_background.reference` via the shared event-background rule.

use pdxl_analysis::context::ClauseKind::{self, DynamicDesc, Effect, ScriptValue, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use crate::kinds;

use super::Entity;
use super::common::{DURATION, OPAQUE, anywhere};

/// `cost = { gold = … piety = … prestige = … renown = … }`. Deducted from the
/// actor on send; the interaction is disabled if they cannot pay. Renown can
/// only be spent by the dynast. The info lists only the first four currencies;
/// `influence` / `treasury` / `treasury_or_gold` are corpus-only (and
/// `influence` outnumbers `gold` in the vanilla corpus).
static COST: StructSpec = StructSpec {
    name: "cost",
    fields: &[
        ("gold", scalar_or_block(Setting, ScriptValue)),
        ("piety", scalar_or_block(Setting, ScriptValue)),
        ("prestige", scalar_or_block(Setting, ScriptValue)),
        ("renown", scalar_or_block(Setting, ScriptValue)),
        (
            "influence",
            scalar_or_block(Setting, ScriptValue).doc("Administrative influence *(corpus)*."),
        ),
        (
            "treasury",
            scalar_or_block(Setting, ScriptValue).doc("Domicile treasury *(corpus)*."),
        ),
        (
            "treasury_or_gold",
            scalar_or_block(Setting, ScriptValue)
                .doc("Pay from the treasury, falling back to gold *(corpus)*."),
        ),
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

/// The engine's `ai_targets` candidate lists, in the info's own order. `diarch`
/// is corpus-only (the info's list omits it).
static AI_RECIPIENTS: &[&str] = &[
    "known_secrets",
    "scheme_targets",
    "hooked_characters",
    "neighboring_rulers",
    "neighboring_rulers_including_tributary_borders",
    "neighboring_top_overlords_including_tributary_borders",
    "neighboring_top_overlords_connected_by_land",
    "peer_vassals",
    "guests",
    "dynasty",
    "courtiers",
    "councillors",
    "prisoners",
    "confederation_house_heads",
    "sub_realm_characters",
    "realm_characters",
    "vassals",
    "tributaries",
    "liege",
    "top_liege",
    "suzerain",
    "top_suzerain",
    "self",
    "head_of_faith",
    "spouses",
    "family",
    "children",
    "primary_war_enemies",
    "war_enemies",
    "war_allies",
    "scripted_relations",
    "activity_host",
    "activity_guests",
    "contacts",
    "domicile_location_top_ruler",
    "domicile_location_top_realm_vassals",
    "domicile_location_neighboring_top_rulers",
    "domicile_location_neighboring_top_realm_vassals",
    "top_realm_domicile_owners",
    "sub_realm_domicile_owners",
    "nearby_domicile_owners",
    "situation_participant_group",
    "diarch",
];

/// The `target_filter` vocabulary (info FAQ). `recipient_lessee_titles` is
/// corpus-only.
static TARGET_FILTERS: &[&str] = &[
    "actor_domain_titles",
    "recipient_domain_titles",
    "secondary_actor_domain_titles",
    "secondary_recipient_domain_titles",
    "actor_domain_titles_including_leases",
    "recipient_domain_titles_including_leases",
    "secondary_actor_domain_titles_including_leases",
    "secondary_recipient_domain_titles_including_leases",
    "actor_de_jure_titles",
    "recipient_de_jure_titles",
    "secondary_actor_de_jure_titles",
    "secondary_recipient_de_jure_titles",
    "actor_realm_titles",
    "recipient_realm_titles",
    "secondary_actor_realm_titles",
    "secondary_recipient_realm_titles",
    "actor_top_liege_de_jure_titles",
    "recipient_top_liege_de_jure_titles",
    "secondary_actor_top_liege_de_jure_titles",
    "secondary_recipient_top_liege_de_jure_titles",
    "recipient_lessee_titles",
    "actor_artifacts",
    "recipient_artifacts",
    "actor_artifacts_claimable",
    "recipient_artifacts_claimable",
    "actor_maa",
    "recipient_maa",
    "actor_personal_maa",
    "recipient_personal_maa",
    "actor_title_maa",
    "recipient_title_maa",
    "count",
];

/// `ai_targets = { ai_recipients = … max = … chance = … }`.
static AI_TARGETS: StructSpec = StructSpec {
    name: "ai_targets",
    fields: &[
        (
            "ai_recipients",
            scalar(Setting)
                .doc(
                    "Which target list the AI considers; may be repeated to combine lists. \
                     A list the engine does not know is a hard error.",
                )
                .values(AI_RECIPIENTS),
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

/// `ai_target_quick_trigger = { adult = yes … }` — cheap engine-side prefilters
/// applied to `ai_targets` before any scripted trigger runs. The corpus uses
/// exactly the four keys the info documents.
static AI_TARGET_QUICK_TRIGGER: StructSpec = StructSpec {
    name: "ai_target_quick_trigger",
    fields: &[
        (
            "adult",
            scalar(Setting)
                .doc("The target must be an adult.")
                .values(&["yes", "no"]),
        ),
        (
            "attracted_to_owner",
            scalar(Setting)
                .doc("The target must be attracted to the actor.")
                .values(&["yes", "no"]),
        ),
        (
            "owner_attracted",
            scalar(Setting)
                .doc("The actor must be attracted to the target.")
                .values(&["yes", "no"]),
        ),
        (
            "prison",
            scalar(Setting)
                .doc("The target must be imprisoned.")
                .values(&["yes", "no"]),
        ),
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
        (
            "filter_tags",
            block(Struct(&OPAQUE)).doc(
                "Bare-word tags for the filtered interaction menu \
                 (`ToggleFilteredCharacterInteractionMenu`); each is localized as \
                 `<tag>_filter_tag_desc`.",
            ),
        ),
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
        (
            "interface",
            scalar(Setting)
                .doc("Specialized GUI to use. Engine-side; values below are corpus-derived.")
                .values(&[
                    "blackmail",
                    "call_ally",
                    "concubine_list",
                    "council_task_interaction",
                    "court_task_interaction",
                    "create_claimant_faction_against",
                    "declare_war",
                    "grant_titles",
                    "interfere_in_war",
                    "marriage",
                    "migration",
                    "modify_vassal_contract",
                    "offer_peace",
                    "revoke_title",
                    "transfer_vassal",
                ]),
        ),
        (
            "special_interaction",
            scalar(Setting).doc(
                "Hard-coded engine behaviour keyed off this id (e.g. \
                 `arrange_marriage_interaction` adds the marriage setup, auto-betrothal, \
                 alliances and prestige). Adds its own is_shown/can_send checks.",
            ),
        ),
        (
            "special_ai_interaction",
            scalar(Setting)
                .doc("Identifies the interaction to specialized AI code (e.g. recruit_courtier)."),
        ),
        ("scheme", scalar(Setting).doc("The scheme type this interaction starts.")),
        ("hidden", scalar(Setting)),
        ("diarch_interaction", scalar(Setting)),
        ("popup_on_receive", scalar(Setting)),
        ("pause_on_receive", scalar(Setting)),
        ("force_notification", scalar(Setting)),
        ("needs_recipient_to_open", scalar(Setting)),
        ("show_effects_in_notification", scalar(Setting)),
        (
            "shows_military_strength",
            scalar(Setting)
                .doc("Show both parties' military strength in the window *(corpus)*.")
                .values(&["yes", "no"]),
        ),
        (
            "target_type",
            scalar(Setting)
                .doc("What kind of thing the interaction targets. Defaults to `count`.")
                .values(&[
                    "title",
                    "artifact",
                    "men_at_arms",
                    "court_position_type",
                    "count",
                ]),
        ),
        (
            "target_filter",
            scalar(Setting)
                .doc("Which pool the target list is drawn from (see the info FAQ).")
                .values(TARGET_FILTERS),
        ),
        (
            "custom_character_sort",
            block(Struct(&OPAQUE)).doc(
                "Bare-word sort options for the character picker, last-defined shown first: \
                 `candidate_score` (needs a target title), `governor_efficiency`, \
                 `obedience`, `merit`.",
            ),
        ),
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
        (
            "recipient_recieve_cooldown",
            block(Struct(&DURATION)).doc(
                "Cooldown on the recipient *receiving* this interaction \
                 (engine spelling: `recieve`) *(corpus)*.",
            ),
        ),
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
        (
            "ai_target_quick_trigger",
            block(Struct(&AI_TARGET_QUICK_TRIGGER))
                .doc("Cheap engine prefilters applied to `ai_targets` before scripted triggers."),
        ),
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
        (
            "name",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Overrides the displayed interaction name; defaults to the key *(corpus)*."),
        ),
        (
            "greeting",
            scalar(Setting)
                .doc("Tone of the request text.")
                .values(&["positive", "negative"]),
        ),
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
                alt: &[],
            }],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(
        "common/character_interactions/",
        ClauseKind::Struct(&INTERACTION),
    )];
}
