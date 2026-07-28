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
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use crate::kinds;

use super::Entity;
use super::common::{COST, DURATION, OPAQUE, anywhere};

/// A `send_option = { … }` block: an extra toggle shown when sending.
static SEND_OPTION: StructSpec = StructSpec {
    name: "send_option",
    fields: &[
        ("is_shown", block(Trigger).doc("Is this option shown?")),
        ("is_valid", block(Trigger).doc("Is this option selectable?")),
        (
            "current_description",
            scalar_or_block(LocKey, DynamicDesc).doc("Tooltip for this option."),
        ),
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
        (
            "can_be_changed",
            block(Trigger).doc("May the author move this option off its default?"),
        ),
        (
            "can_invalidate_interaction",
            scalar(Setting)
                .doc(
                    "Re-run the *whole* can-send check when the AI picks this option, instead \
                     of only the cheap refusal and `ai_will_do` checks. Use sparingly and \
                     profile it — options are assumed not to block sending.",
                )
                .values(&["yes", "no"]),
        ),
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
        (
            "parameter",
            scalar(Setting).doc(
                "Detail for a target list that needs one — only \
                 `situation_participant_group`, where it names the situation. Empty by default.",
            ),
        ),
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
        (
            "trigger",
            block_scoped(Trigger, "character")
                .doc("Root is `scope:actor`. The first entry that passes wins."),
        ),
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
        (
            "icon_small",
            scalar(Setting).doc(
                "Small icon. Defaults to \
                 `gfx/interface/icons/character_interactions/<key>_small.dds`.",
            ),
        ),
        (
            "alert_icon",
            scalar(Setting).doc(
                "Alert icon. Defaults to \
                 `gfx/interface/icons/character_interactions/<key>_alert.dds`.",
            ),
        ),
        (
            "extra_icon",
            scalar(Setting)
                .doc("Icon shown when `should_use_extra_icon` passes; its tooltip is `<key>_extra_icon`."),
        ),
        (
            "should_use_extra_icon",
            block(Trigger).doc("When to show `extra_icon`."),
        ),
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
        (
            "hidden",
            scalar(Setting)
                .doc("Hide the interaction entirely.")
                .values(&["yes", "no"]),
        ),
        (
            "diarch_interaction",
            scalar(Setting)
                .doc("Available to a diarch, including a non-ruler one.")
                .values(&["yes", "no"]),
        ),
        (
            "popup_on_receive",
            scalar(Setting)
                .doc("Pop up for the recipient when received.")
                .values(&["yes", "no"]),
        ),
        (
            "pause_on_receive",
            scalar(Setting)
                .doc("Pause the game on receipt — usually paired with `popup_on_receive`.")
                .values(&["yes", "no"]),
        ),
        (
            "force_notification",
            scalar(Setting)
                .doc("Force a diplomacy item even when the interaction auto-accepts.")
                .values(&["yes", "no"]),
        ),
        (
            "needs_recipient_to_open",
            scalar(Setting)
                .doc(
                    "Require a recipient before the window opens. Default `yes`; set `no` only \
                     with code support, for interactions opened from somewhere other than the \
                     right-click menu, where a `redirect` supplies the recipient later.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "show_effects_in_notification",
            scalar(Setting)
                .doc("Show the interaction's effects in the send notification. Default `yes`.")
                .values(&["yes", "no"]),
        ),
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
        (
            "secondary_actor",
            scalar(Setting).doc(
                "Declares a secondary participant on the actor's side and what the pick list \
                 is built from. `marriage` also redirects to the matchmaker and auto-handles \
                 betrothal, alliances and prestige; `marry_off` builds the list from all \
                 marriageable characters instead (see the info's FAQ).",
            ),
        ),
        (
            "secondary_recipient",
            scalar(Setting).doc("The recipient-side counterpart of `secondary_actor`."),
        ),
        (
            "secondary_scopes_optional",
            scalar(Setting)
                .doc("May the interaction send without the secondary participants chosen?")
                .values(&["yes", "no"]),
        ),
        (
            "send_options_exclusive",
            scalar(Setting)
                .doc(
                    "Are the send options mutually exclusive? Non-exclusive options cost AI \
                     performance — keep to four or five visible at once.",
                )
                .values(&["yes", "no"]),
        ),
        ("send_option", block(Struct(&SEND_OPTION))),
        (
            "options_heading",
            scalar(LocKey).doc("Text above the options block, describing them collectively."),
        ),
        // ── triggers ─────────────────────────────────────────────────────
        (
            "is_shown",
            block(Trigger).doc("Is the interaction visible? Scopes: scope:actor, scope:recipient."),
        ),
        (
            "is_valid",
            block(Trigger).doc("Is the interaction selectable (enabled)?"),
        ),
        (
            "is_valid_showing_failures_only",
            block(Trigger).doc(
                "Same gate as `is_valid`, but only its *failures* are printed. Scopes: \
                 `scope:actor`, `scope:recipient`.",
            ),
        ),
        (
            "is_available",
            block_scoped(Trigger, "character").doc(
                "Available for the actor, AI and player alike. Root is the actor — which is \
                 why actor-only checks belong here rather than in `is_shown`, where they cost \
                 the AI a full evaluation per candidate recipient.",
            ),
        ),
        ("is_highlighted", block(Trigger).doc("Highlight the interaction in the menu?")),
        (
            "has_valid_target",
            block(Trigger).doc("Is the selected target valid?"),
        ),
        (
            "has_valid_target_showing_failures_only",
            block(Trigger).doc("Same as `has_valid_target`, printing only its failures."),
        ),
        (
            // The info's FAQ: checked "with the tested character set as root",
            // so the block's root is that candidate rather than the actor.
            "can_be_picked",
            block_scoped(Trigger, "character").doc(
                "Can this character be picked from the list? Root is the tested character. \
                 A filter for the *first* pick only — it is not part of the can-send checks, \
                 so keep it cheap.",
            ),
        ),
        (
            "can_be_picked_title",
            block(Trigger).doc(
                "Can this title be picked? The candidate arrives as `scope:target`, and this \
                 runs only while building the list — never on send.",
            ),
        ),
        (
            "can_be_picked_artifact",
            block(Trigger).doc("Can this artifact be picked? The candidate is `scope:target`."),
        ),
        (
            "can_be_picked_regiment",
            block(Trigger).doc("Can this regiment be picked? The candidate is `scope:target`."),
        ),
        ("can_send", block(Trigger).doc("Can the interaction be sent?")),
        (
            "can_be_blocked",
            block(Trigger)
                .doc("Can the recipient block it — for instance with a hook on the actor?"),
        ),
        (
            "needs_confirmation",
            block(Trigger).doc(
                "Open a confirmation window at all; true when unset. **Deprecated** — it can \
                 run gamestate-changing effects with no warning to the player.",
            ),
        ),
        (
            "ignore_recipient_recieve_cooldown",
            block(Trigger)
                .doc("When this passes, `recipient_recieve_cooldown` is bypassed."),
        ),
        (
            "auto_accept",
            scalar_or_block(Setting, Trigger).doc("`yes`/`no` or a trigger — is it auto-accepted?"),
        ),
        (
            "use_diplomatic_range",
            scalar_or_block(Setting, Trigger)
                .doc("Does the interaction respect diplomatic range? `yes` by default."),
        ),
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
        (
            "on_blocked_effect",
            block(Effect).doc("Runs when the recipient blocks it — only if the intermediary accepted."),
        ),
        (
            "pre_auto_accept",
            block(Effect).doc(
                "Auto-accepted interactions only, and before any hard-coded side effect such \
                 as the marriage itself.",
            ),
        ),
        (
            "on_auto_accept",
            block(Effect).doc("Auto-accepted interactions only, after the built-in effects."),
        ),
        (
            "on_intermediary_accept",
            block(Effect)
                .doc("The intermediary let it through; the recipient's decision comes next."),
        ),
        (
            "on_intermediary_decline",
            block(Effect)
                .doc("The intermediary refused, so nothing reaches the recipient."),
        ),
        (
            "on_decline_summary",
            scalar_or_block(LocKey, DynamicDesc).doc(
                "Flavour under the acceptance widget — for drawing attention to what \
                 declining costs.",
            ),
        ),
        (
            "redirect",
            block(Effect).doc(
                "Reassigns the participants: any of `scope:actor`, `scope:secondary_actor`, \
                 `scope:recipient`, `scope:secondary_recipient` and `scope:intermediary` may \
                 be replaced with another character.",
            ),
        ),
        (
            "populate_actor_list",
            block(Effect).doc(
                "Everyone sorted into the `characters` list becomes selectable. Uses the \
                 actor, recipient and secondary scopes.",
            ),
        ),
        (
            "populate_recipient_list",
            block(Effect).doc("As `populate_actor_list`, for the recipient side."),
        ),
        (
            "localization_values",
            block(Effect).doc(
                "Saves values for the text to interpolate — \
                 `RANSOM_COST = scope:secondary_recipient.ransom_cost_value` then lets loc \
                 write `$RANSOM_COST|0$`.",
            ),
        ),
        // ── AI ───────────────────────────────────────────────────────────
        ("ai_accept", block(ScriptValue).doc("MTTH: will the AI accept this interaction?")),
        (
            "ai_intermediary_accept",
            block(ScriptValue)
                .doc("MTTH: will the intermediary AI forward this to the recipient?"),
        ),
        ("ai_will_do", block(ScriptValue).doc("MTTH: how interested the AI is in sending it (0–100).")),
        (
            "ai_potential",
            block_scoped(Trigger, "character").doc(
                "Will the AI consider this at all? Root is the actor, and no event targets \
                 are available. **Deprecated** — use `is_available`.",
            ),
        ),
        (
            "ai_set_target",
            block(Effect).doc(
                "Set `scope:target` to aim the AI at something specific. Title-targeting \
                 interactions do not need it.",
            ),
        ),
        ("ai_targets", block(Struct(&AI_TARGETS))),
        (
            "ai_target_quick_trigger",
            block(Struct(&AI_TARGET_QUICK_TRIGGER))
                .doc("Cheap engine prefilters applied to `ai_targets` before scripted triggers."),
        ),
        (
            "ai_frequency",
            scalar(Setting).doc("Months between AI considerations of this interaction."),
        ),
        ("ai_frequency_by_tier", block(Struct(&AI_FREQUENCY_BY_TIER))),
        (
            "ai_instant_response",
            scalar(Setting)
                .doc("Reply at once instead of feigning N days of deliberation.")
                .values(&["yes", "no"]),
        ),
        (
            "ai_accept_negotiation",
            scalar(Setting)
                .doc(
                    "A decline opens negotiations, so the interface stops saying \"won't \
                     accept\" — the event chain may still end in acceptance.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "ai_maybe",
            scalar(Setting)
                .doc("Randomize the AI's answer.")
                .values(&["yes", "no"]),
        ),
        (
            "ai_intermediary_maybe",
            scalar(Setting)
                .doc("Randomize the intermediary's answer.")
                .values(&["yes", "no"]),
        ),
        (
            "ai_min_reply_days",
            scalar(Setting).doc("Minimum days before the AI replies."),
        ),
        (
            "ai_max_reply_days",
            scalar(Setting).doc("Maximum days before the AI replies."),
        ),
        (
            "can_send_despite_rejection",
            scalar(Setting)
                .doc("Allow sending even when the AI is known to refuse.")
                .values(&["yes", "no"]),
        ),
        (
            "ignores_pending_interaction_block",
            scalar(Setting)
                .doc(
                    "Send even while the recipient still owes this player an answer. \
                     Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        // ── text (loc keys) ──────────────────────────────────────────────
        (
            "desc",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Short description of the interaction."),
        ),
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
        (
            "highlighted_reason",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Tooltip explaining why the interaction is highlighted."),
        ),
        (
            "send_name",
            scalar(LocKey)
                .doc("Name once sent, as seen in the diplomacy item. Defaults to the key."),
        ),
        (
            "prompt",
            scalar(LocKey).doc("Text under the portrait — \"Pick a Guardian\"."),
        ),
        (
            "notification_text",
            scalar(LocKey).doc("The request as the recipient reads it."),
        ),
        (
            "intermediary_notification_text",
            scalar(LocKey).doc("The request as the intermediary reads it."),
        ),
        (
            "reply_item_key",
            scalar(LocKey).doc(
                "Tooltip on the sent-interaction item; receives the interaction name in \
                 `$INTERACTION$`. Default `INTERACTION_REPLY_ITEM`.",
            ),
        ),
        // Shown to the *sender* while composing: what the target is going to say.
        (
            "pre_answer_yes_key",
            scalar(LocKey).doc("It will be accepted. Default `ANSWER_YES`."),
        ),
        (
            "pre_answer_no_key",
            scalar(LocKey).doc("It will not be accepted. Default `ANSWER_NO`."),
        ),
        (
            "pre_answer_maybe_key",
            scalar(LocKey)
                .doc("It might be accepted; receives the value in `$VALUE$`. Default `ANSWER_MAYBE`."),
        ),
        (
            "pre_answer_yes_breakdown_key",
            scalar(LocKey).doc("Header for the recipient's acceptance breakdown when accepting."),
        ),
        (
            "pre_answer_no_breakdown_key",
            scalar(LocKey).doc("Header for that breakdown when declining."),
        ),
        (
            "pre_answer_maybe_breakdown_key",
            scalar(LocKey).doc("Header for that breakdown when the answer is randomized."),
        ),
        (
            "intermediary_breakdown_yes",
            scalar(LocKey).doc("The same header, for the intermediary, when accepting."),
        ),
        (
            "intermediary_breakdown_no",
            scalar(LocKey).doc("The same, when declining."),
        ),
        (
            "intermediary_breakdown_maybe",
            scalar(LocKey).doc("The same, when randomized."),
        ),
        // Shown to whoever is *answering* — button labels.
        (
            "intermediary_answer_accept_key",
            scalar(LocKey).doc("Intermediary's accept button. Default `ANSWER_YES`."),
        ),
        (
            "intermediary_answer_reject_key",
            scalar(LocKey).doc("Intermediary's decline button. Default `ANSWER_NO`."),
        ),
        (
            "answer_block_key",
            scalar(LocKey).doc("Recipient's block text. Default `ANSWER_BLOCK`."),
        ),
        (
            "answer_accept_key",
            scalar(LocKey).doc("Recipient's accept button. Default `ANSWER_YES`."),
        ),
        (
            "answer_reject_key",
            scalar(LocKey).doc("Recipient's decline button. Default `ANSWER_NO`."),
        ),
        (
            "answer_acknowledge_key",
            scalar(LocKey).doc(
                "Acknowledge button, for notifications about something that already \
                 happened. Default `ANSWER_ACKNOWLEDGE`.",
            ),
        ),
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
