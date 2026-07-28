//! Activities (`common/activities/`) — the travel-and-attend events rulers
//! host (feasts, hunts, tournaments…). Six databases in one directory, from
//! six info files: `_activity_type.info` (the big one), `_intents.info`,
//! `_pulse_actions.info`, `_activity_locales.info`, `_invite_rules.info`,
//! `_activity_group_types.info`.
//!
//! Seven kinds. The heart is `activity_type` (44 defs in game+T4N); the
//! satellites it names are `activity_intent` (59), `activity_pulse_action`
//! (413 across game+mod files), `activity_locale` (6), `guest_invite_rule`
//! (56) and `activity_group_type` (6). Phases are `ScopedChildrenOf` under
//! `phases` — the contribution precedent: keys recur across activities
//! (every tournament re-lists `tournament_phase_joust`), so a repeat
//! gap-fills instead of reporting a duplicate.
//!
//! Cross-references, each corpus-validated over game + T4N:
//! - `activity_type:X` resolves through the table-derived scope-link rule
//!   (`derived.rs`) once this kind exists.
//! - engine triggers/effects taking a literal key: `has_activity_type` (~460),
//!   `is_activity_type_on_cooldown`, `ai_attempt_to_host_activity`,
//!   `can_host_activity` (one `scope:…` value, skipped by the `:` rule);
//!   `has_activity_intent` (~600), `set_activity_intent` (scalar and its
//!   doc-only `{ intent = X }` block form),
//!   `has_completed_activity_intent = { type = X }`; `has_current_phase`
//!   (~300), `has_phase[_past/_future]` in both scalar and `{ type = X }`
//!   form (`_past`/`_future` have no corpus use yet — listed in the docs,
//!   kept for symmetry); `has_active_locale`; the guest-subset family's
//!   `phase = X` field.
//! - list/weighted forms gated to `activity_types/`: `intents` /
//!   `player_defaults` / `blocked_intents` → intent, `entries` → pulse
//!   action, the inner `locales` lists → locale, and `guest_invite_rules`'s
//!   numeric-priority `rules` / `defaults` → invite rule.
//!
//! Where the info and the corpus disagree the corpus wins:
//! - `notify_player_can_join_activity` (4 uses) is the real key; the info's
//!   `notify_player_can_join_open_activity` never occurs.
//! - `pulse_actions = { entries = { … } chance_of_no_event = N }` (43/42
//!   uses) is entirely undocumented — the info has no `pulse_actions` field.
//! - intents' `auto_complete` (65 uses) is undocumented.
//! - `guest_description` (2 uses, loc `<key>_guest_desc` 20/21) is
//!   undocumented.
//!
//! Implicit localization (measured over the 21 vanilla types): `<key>` and
//! `<key>_desc` 21/21, `<key>_host_desc` 21/21, `<key>_guest_desc` 20/21,
//! `<key>_conclusion_desc` 17/21, `<key>_province_desc` 16/21. Phases,
//! intents and locales use `<key>`/`<key>_desc`; group types
//! `activity_group_type_<key>` (6/6); invite rules `<key>` (55/56) plus the
//! documented `<key>_desc` no rule uses yet. `<key>_name` (6/21) is *not* a
//! convention and is omitted.
//!
//! Deliberate omissions: option categories, options, special guests, guest
//! subsets and locale slots are named blocks, but script reaches them only
//! through runtime constructs (`scope:<key>`, `flag:<option>`,
//! `name = <subset>`) our rules do not resolve — no kinds for them.
//! `special_option_category` names a category of the same file's `options`;
//! `province_filter_target` is a landed-title *or* region key — a ref would
//! misdiagnose the region half, so both stay documented settings. Pulse
//! actions get no implicit loc: their text hangs off the
//! `add_activity_log_entry` key, which only conventionally matches the
//! action's name.

use pdxl_analysis::context::ClauseKind::{
    self, Config, DynamicDesc, Effect, ScriptValue, ScriptedModifier, Struct, Trigger,
};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{
    Fallback, FieldSpec, StructSpec, block, block_scoped, scalar, scalar_or_block,
};
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern, RefRule,
};

use crate::kinds;

use super::Entity;
use super::common::{CHECK_INTERVAL_BY_TIER, COST, DURATION, OPAQUE, TRIGGERED_ASSET, anywhere};

const TYPES_DIR: &str = "common/activities/activity_types/";
const INTENTS_DIR: &str = "common/activities/intents/";
const PULSE_DIR: &str = "common/activities/pulse_actions/";
const LOCALES_DIR: &str = "common/activities/activity_locales/";
const INVITE_RULES_DIR: &str = "common/activities/guest_invite_rules/";
const GROUP_TYPES_DIR: &str = "common/activities/activity_group_types/";

/// A reference rule gated to the activity-type files.
const fn in_types(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(TYPES_DIR),
        alt: &[],
    }
}

/// A `yes`/`no` toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// The `province_filter` / `ai_province_filter` vocabulary.
const PROVINCE_FILTERS: &[&str] = &[
    "capital",
    "domain",
    "realm",
    "top_realm",
    "holy_sites",
    "holy_sites_domain",
    "holy_sites_realm",
    "domicile",
    "domicile_domain",
    "domicile_realm",
    "top_liege_border_inner",
    "top_liege_border_outer",
    "landed_title",
    "geographical_region",
    "all",
];

/// A triggered `background` / `locale_background` candidate: the first whose
/// trigger passes is shown.
static ACTIVITY_BACKGROUND: StructSpec = StructSpec {
    name: "activity_background",
    fields: &[
        (
            "trigger",
            block_scoped(Trigger, "activity")
                .doc("Root is the activity; the first candidate that passes wins."),
        ),
        ("texture", scalar(Setting).doc("Background texture path.")),
        (
            "environment",
            scalar(Setting)
                .doc("Reference key to a database object in gfx/portraits/environments/."),
        ),
        ("ambience", scalar(Setting).doc("Ambience event path.")),
        ("music", scalar(Setting).doc("Music cue track name.")),
    ],
    fallback: Fallback::Deny,
};

/// The inline scripted-animation block (`window_characters`, intents): trigger-
/// gated animations plus a default.
static SCRIPTED_ANIMATION: StructSpec = StructSpec {
    name: "scripted_animation",
    fields: &[
        (
            "triggered_animation",
            block(Struct(&super::event::TRIGGERED_ANIMATION))
                .doc("Trigger-gated animation; the first whose trigger passes wins."),
        ),
        (
            "animation",
            scalar_or_block(Setting, Config)
                .doc("Default animation if all triggers fail — one name, or a list to pick from."),
        ),
        (
            "scripted_animation",
            scalar(Setting).doc("A scripted-animation key (alternative to `animation`)."),
        ),
        (
            "camera",
            scalar(Setting)
                .doc("Camera name; overrides the default camera when this animation is chosen."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `travel_entourage_selection = { … }` — which court characters join the
/// travel entourage. Appears on the activity, and per option.
static TRAVEL_ENTOURAGE: StructSpec = StructSpec {
    name: "travel_entourage_selection",
    fields: &[
        (
            "weight",
            block_scoped(ScriptValue, "character").doc(
                "Root is a character in the travel-plan owner's court; evaluated until all \
                 values are negative or `max` is reached. `scope:host`, `scope:owner`, \
                 `scope:special_option`.",
            ),
        ),
        (
            "max",
            scalar(Setting).doc("Up to how many characters to select for a player."),
        ),
        (
            "ai_max",
            scalar(Setting).doc("Up to how many characters to select for an AI."),
        ),
        (
            "invite_rule_order",
            scalar(Setting)
                .doc("Where, relative to the invite-rule orders, entourage members are invited."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `host_intents` / `guest_intents` — the intents pickable in this activity.
static INTENT_LIST: StructSpec = StructSpec {
    name: "intent_list",
    fields: &[
        (
            "intents",
            block(Config).doc("The pickable intents (`common/activities/intents/`)."),
        ),
        (
            "default",
            scalar(Setting)
                .refs(kinds::ACTIVITY_INTENT)
                .doc("The default intent; must always be valid, and be in `intents` too."),
        ),
        (
            "player_defaults",
            block(Config).doc("Optional intents to try, in order, as the player's default."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `guest_invite_rules = { rules = { <prio> = <rule> … } defaults = { … } }`.
static GUEST_INVITE_RULES: StructSpec = StructSpec {
    name: "guest_invite_rules",
    fields: &[
        (
            "rules",
            block(Config)
                .doc("`<priority> = <rule>` pairs; lower priority invites earlier (must be ≥ 1)."),
        ),
        (
            "defaults",
            block(Config).doc(
                "As `rules`, but enabled by default for the player; do not repeat rules \
                 already listed there.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `pulse_actions = { entries = { … } … }` *(corpus)* — the info does not
/// document this field at all.
static PULSE_ENTRIES: StructSpec = StructSpec {
    name: "pulse_actions",
    fields: &[
        (
            "entries",
            block(Config)
                .doc("The pulse actions (`common/activities/pulse_actions/`) this activity rolls."),
        ),
        (
            "chance_of_no_event",
            scalar_or_block(Setting, ScriptValue)
                .doc("Weight for rolling no pulse action at all *(corpus)*."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// One slot in an activity's `locales = { <slot> = { … } }`.
static LOCALE_SLOT: StructSpec = StructSpec {
    name: "activity_locale_slot",
    fields: &[
        (
            "is_available",
            block_scoped(Trigger, "activity")
                .doc("Is this slot available? Defaults to true; use for optional slots."),
        ),
        (
            "locales",
            block(Config).doc("The locale types that can fill this slot between active phases."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `locales = { <slot key> = { … } }`.
static LOCALE_SLOTS: StructSpec = StructSpec {
    name: "locales",
    fields: &[],
    fallback: Fallback::Struct(&LOCALE_SLOT),
};

/// One `special_guests = { <key> = { … } }` entry. The key is localized as
/// `<key>` and, for the host, `<key>_for_host`.
static SPECIAL_GUEST: StructSpec = StructSpec {
    name: "special_guest",
    fields: &[
        (
            "is_shown",
            block_scoped(Trigger, "character").doc("Is this slot shown? Root is the host."),
        ),
        (
            "is_required",
            toggle(
                "Must be set at the start (wedding bride); declining invalidates the \
                 activity. Default `no`.",
            ),
        ),
        (
            "select_character",
            block_scoped(Effect, "character").doc(
                "Interface effects picking the guest (`save_scope_as = character`); \
                 unset means manual selection. Root is the host.",
            ),
        ),
        (
            "can_pick",
            block_scoped(Trigger, "character").doc(
                "Is the scoped character valid for this role? `scope:host`, \
                 `scope:special_guests`, and one scope per defined special-guest key.",
            ),
        ),
        (
            "ai_will_do",
            block_scoped(ScriptValue, "character")
                .doc("Weight for the AI's weighted-random pick among valid characters."),
        ),
        (
            "on_invite",
            block_scoped(Effect, "character").doc(
                "Fires when the invitation is sent (before acceptance); shown in the interface.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `special_guests = { <key> = { … } }`.
static SPECIAL_GUESTS: StructSpec = StructSpec {
    name: "special_guests",
    fields: &[],
    fallback: Fallback::Struct(&SPECIAL_GUEST),
};

/// One option inside an option category.
static OPTION: StructSpec = StructSpec {
    name: "activity_option",
    fields: &[
        (
            "is_shown",
            block_scoped(Trigger, "character")
                .doc("Is this option shown in its category? Root is the planning character."),
        ),
        (
            "is_valid",
            block_scoped(Trigger, "character")
                .doc("Is this option pickable? Root is the planning character."),
        ),
        (
            "on_start",
            block_scoped(Effect, "activity").doc("Fires when the activity is created."),
        ),
        (
            "default",
            scalar_or_block(Setting, Trigger)
                .doc(
                    "The first option whose trigger passes is the default; `default = yes` \
                     for a constant.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "blocked_intents",
            block(Config).doc("Intents blocked for the host if this option is picked."),
        ),
        (
            "blocked_phases",
            block(Config).doc(
                "Phases removed from the activity if this option is picked (activities \
                 with no pickable phases only).",
            ),
        ),
        (
            "ai_will_do",
            block_scoped(ScriptValue, "character")
                .doc("Weight for the AI's weighted-random pick among valid options."),
        ),
        (
            "cost",
            block(Struct(&COST)).doc("Cost for enabling this option."),
        ),
        (
            "travel_entourage_selection",
            block(Struct(&TRAVEL_ENTOURAGE)).doc(
                "Court characters added to the entourage when this option is picked \
                 (run for host and guests alike).",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// One `options = { <category> = { <option> = { … } … } }` category.
static OPTION_CATEGORY: StructSpec = StructSpec {
    name: "activity_option_category",
    fields: &[],
    fallback: Fallback::Struct(&OPTION),
};

/// `options = { <category> = { … } }`.
static OPTIONS: StructSpec = StructSpec {
    name: "options",
    fields: &[],
    fallback: Fallback::Struct(&OPTION_CATEGORY),
};

/// One `phases = { <key> = { … } }` entry.
static PHASE: StructSpec = StructSpec {
    name: "activity_phase",
    fields: &[
        (
            "is_predefined",
            toggle("Always present and not removable (vs. pickable). Default `no`."),
        ),
        (
            "number_of_picks",
            scalar_or_block(Setting, ScriptValue).doc(
                "How many times this phase can be picked within one province \
                 (pickable phases only). Default 1.",
            ),
        ),
        (
            "order",
            scalar(Setting)
                .doc("Position of this phase in the activity; ties resolve in the order added."),
        ),
        (
            "location_source",
            scalar(Setting)
                .doc("How the phase determines its location. Default `pickable`.")
                .values(&[
                    "pickable",
                    "first_picked_phase",
                    "last_picked_phase",
                    "scripted",
                ]),
        ),
        (
            "select_scripted_location",
            block_scoped(Effect, "character").doc(
                "Picks the location (`save_scope_as = province`); requires \
                 `location_source = scripted`. Root is the host.",
            ),
        ),
        (
            "ai_will_do",
            block_scoped(ScriptValue, "character").doc(
                "Weight for the AI's phase pick; `scope:province` is the location \
                 under evaluation.",
            ),
        ),
        (
            "is_shown",
            block_scoped(Trigger, "character")
                .doc("Is this phase offered at all? Root is the planning character."),
        ),
        (
            "can_pick",
            block_scoped(Trigger, "character")
                .doc("Is this phase pickable? `scope:province` is the proposed location."),
        ),
        (
            "is_valid",
            block_scoped(Trigger, "activity")
                .doc("Is this phase still valid for the ongoing activity?"),
        ),
        (
            "on_enter_phase",
            block_scoped(Effect, "character")
                .doc("Fires when this becomes the current (not yet active) phase."),
        ),
        (
            "on_phase_active",
            block_scoped(Effect, "character").doc("Fires when this phase becomes active."),
        ),
        (
            "on_end",
            block_scoped(Effect, "character").doc("Fires when this phase ends."),
        ),
        (
            "on_monthly_pulse",
            block_scoped(Effect, "character").doc("The active phase's monthly pulse."),
        ),
        (
            "on_weekly_pulse",
            block_scoped(Effect, "character")
                .doc("The active phase's weekly pulse, for extra time dilation."),
        ),
        (
            "on_invalidated",
            block_scoped(Effect, "character").doc("Fires if `is_valid` fails for this phase."),
        ),
        (
            "cost",
            block(Struct(&COST)).doc(
                "Cost for planning this phase; `scope:province` and \
                 `scope:previous_province` are available.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `phases = { <key> = { … } }`.
static PHASES: StructSpec = StructSpec {
    name: "phases",
    fields: &[],
    fallback: Fallback::Struct(&PHASE),
};

/// One `window_characters = { <key> = { … } }` entry (localized as
/// `activity_window_character_<key>`); displayed left to right.
static WINDOW_CHARACTER: StructSpec = StructSpec {
    name: "window_character",
    fields: &[
        (
            "camera",
            scalar(Setting).doc("Camera used for this portrait."),
        ),
        (
            "effect",
            block_scoped(Effect, "activity").doc(
                "Picks the shown character: scope to them and `add_to_list = characters`; \
                 a random valid, not-yet-displayed one is used. `scope:player` views.",
            ),
        ),
        (
            "scripted_animation",
            scalar_or_block(Setting, Struct(&SCRIPTED_ANIMATION))
                .doc("Animation selection: a scripted-animation key, or an inline block."),
        ),
        (
            "animation",
            scalar(Setting).doc("Plain animation, without trigger selection."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `window_characters = { <key> = { … } }`.
static WINDOW_CHARACTERS: StructSpec = StructSpec {
    name: "window_characters",
    fields: &[],
    fallback: Fallback::Struct(&WINDOW_CHARACTER),
};

/// The body of one activity type.
static ACTIVITY_TYPE: StructSpec = StructSpec {
    name: "activity_type",
    fields: &[
        (
            "is_shown",
            block_scoped(Trigger, "character")
                .doc("Should the player see this activity type? Root is the would-be host."),
        ),
        (
            "notify_player_can_join_activity",
            toggle(
                "Alert the player when they can join this activity if open. Default `no`. \
                 The info calls it `notify_player_can_join_open_activity`; every use has \
                 this name *(corpus)*.",
            ),
        ),
        (
            "activity_group_type",
            scalar(Setting)
                .refs(kinds::ACTIVITY_GROUP_TYPE)
                .doc("Foldable group this activity sits in. Default `activities`."),
        ),
        (
            "sort_order",
            scalar(Setting).doc("Order within the activity group; higher sorts first. Default 0."),
        ),
        (
            "can_start",
            block_scoped(Trigger, "character").doc(
                "Can this activity be picked if visible? Shows met and failed triggers; \
                 `scope:on_create_variables` carries creation-data scopes.",
            ),
        ),
        (
            "can_start_showing_failures_only",
            block_scoped(Trigger, "character")
                .doc("As `can_start`, showing only failing triggers."),
        ),
        (
            "can_plan",
            block_scoped(Trigger, "character").doc(
                "Can this activity be planned? Falls back to \
                 `can_start_showing_failures_only` when absent.",
            ),
        ),
        (
            "can_always_plan",
            toggle(
                "Let planning start even when requirements (not costs or cooldown) \
                 are unmet. Default `yes`.",
            ),
        ),
        (
            "is_valid",
            block_scoped(Trigger, "activity").doc("Is this activity valid to continue?"),
        ),
        (
            "on_invalidated",
            block_scoped(Effect, "activity").doc("Fires when `is_valid` fails."),
        ),
        (
            "on_host_death",
            block_scoped(Effect, "activity").doc("Fires when the host dies."),
        ),
        (
            "province_filter",
            scalar(Setting)
                .doc(
                    "Which provinces are candidate locations. Default `capital`; \
                     `all` is noticeably slower.",
                )
                .values(PROVINCE_FILTERS),
        ),
        (
            "ai_province_filter",
            scalar(Setting)
                .doc("As `province_filter`, for the AI; defaults to the base filter.")
                .values(PROVINCE_FILTERS),
        ),
        (
            "province_filter_target",
            scalar(Setting).doc(
                "Target for a filter that needs one: a landed-title key for \
                 `landed_title`, a region key for `geographical_region`.",
            ),
        ),
        (
            "is_location_valid",
            block_scoped(Trigger, "province")
                .doc("Is the scoped province valid for this activity at all?"),
        ),
        (
            "province_score",
            block_scoped(ScriptValue, "province")
                .doc("How good the scoped province is to host in."),
        ),
        (
            "max_province_icons",
            scalar(Setting)
                .doc("Show only the top-scored provinces as map icons. Default unlimited."),
        ),
        (
            "options",
            block(Struct(&OPTIONS)).doc("Option categories, each holding uniquely-named options."),
        ),
        (
            "special_option_category",
            scalar(Setting).doc(
                "The category the player must pick from before selecting a location; \
                 its options need an illustration and a flat icon.",
            ),
        ),
        (
            "phases",
            block(Struct(&PHASES)).doc("The activity's phases; at least one, uniquely named."),
        ),
        (
            "num_pickable_phases",
            scalar_or_block(Setting, ScriptValue).doc(
                "How many non-predefined phases the player picks (DLC-gated above one). \
                 Default 0.",
            ),
        ),
        (
            "max_pickable_phases_per_province",
            scalar_or_block(Setting, ScriptValue)
                .doc("Phase picks allowed per province. Defaults to `num_pickable_phases`."),
        ),
        (
            "wait_time_before_start",
            block(Struct(&DURATION))
                .doc("Delay after the host's estimated arrival before the first phase."),
        ),
        (
            "max_guest_arrival_delay_time",
            block(Struct(&DURATION)).doc(
                "Extra delay granted for invited guests' travel (required special \
                 guests are always waited for).",
            ),
        ),
        (
            "max_route_deviation_mult",
            scalar(Setting).doc(
                "How far a player's waypoints may stretch the default path, as a \
                 duration multiplier.",
            ),
        ),
        (
            "cooldown",
            block(Struct(&DURATION)).doc("Cooldown before hosting this activity again."),
        ),
        (
            "is_single_location",
            toggle("Do all phases share one province? Default `yes`."),
        ),
        (
            "planner_type",
            scalar(Setting)
                .doc("How the activity planner presents locations. Default `province`.")
                .values(&["province", "holder"]),
        ),
        (
            "ai_will_do",
            block_scoped(ScriptValue, "character").doc(
                "Hosting weight; must beat ACTIVITY_SCORE_THRESHOLD, then acts as a \
                 percent chance for the best-scoring activity.",
            ),
        ),
        (
            "ai_check_interval",
            scalar(Setting).doc("Months between AI hosting checks (all tiers)."),
        ),
        (
            "ai_check_interval_by_tier",
            block(Struct(&CHECK_INTERVAL_BY_TIER))
                .doc("Months per tier, used instead of `ai_check_interval`; `0` never."),
        ),
        (
            "ai_will_select_province",
            block_scoped(ScriptValue, "province").doc(
                "The AI's weighted-random province score; `scope:score` carries \
                 `province_score`.",
            ),
        ),
        (
            "ai_select_num_provinces",
            block_scoped(ScriptValue, "character")
                .doc("How many provinces the AI tries to select (multi-location only)."),
        ),
        (
            "cost",
            block(Struct(&COST)).doc(
                "Cost of planning the activity; `scope:province` / the `provinces` \
                 list carry the selected location(s) when available.",
            ),
        ),
        (
            "ui_predicted_cost",
            block(Struct(&COST)).doc("Rough expected cost shown before planning."),
        ),
        (
            "max_guests",
            block(ScriptValue).doc(
                "Guest cap; one `scope:<option_category>` flag per category (use `?=`). \
                 High values hurt performance.",
            ),
        ),
        (
            "reserved_guest_slots",
            scalar(Setting)
                .doc("Guest slots kept free for characters added via effects or events."),
        ),
        (
            "allow_zero_guest_invites",
            toggle("May the activity start with no guest invitations? Default `no`."),
        ),
        (
            "guest_invite_rules",
            block(Struct(&GUEST_INVITE_RULES)).doc("Which invite rules this activity offers."),
        ),
        (
            "can_be_activity_guest",
            block_scoped(Trigger, "character").doc(
                "Extra per-type guest check, on top of the `can_be_activity_guest` \
                 scripted rule.",
            ),
        ),
        (
            "guest_subsets",
            block(Config).doc("Guest-subset names phases can reference (`any_guest_subset`)."),
        ),
        (
            "special_guests",
            block(Struct(&SPECIAL_GUESTS)).doc(
                "Appointable guests script can reference; the activity waits for them, \
                 and a required one declining invalidates it.",
            ),
        ),
        (
            "locales",
            block(Struct(&LOCALE_SLOTS)).doc("Locale slots available between active phases."),
        ),
        (
            "locale_cooldown",
            block(Struct(&DURATION)).doc("Days all locales stay closed after entering one."),
        ),
        (
            "auto_select_locale_cooldown",
            block(Struct(&DURATION))
                .doc("Cadence of the player's auto-visits; unset disables them."),
        ),
        (
            "early_locale_opening_duration",
            block(Struct(&DURATION)).doc(
                "How long before the first phase locales open; unset opens them \
                 from creation.",
            ),
        ),
        (
            "open_invite",
            toggle(
                "Anyone meeting the requirements may join; only the host's court is \
                 explicitly invited. Default `no`.",
            ),
        ),
        (
            "host_intents",
            block(Struct(&INTENT_LIST)).doc("The host's pickable intents."),
        ),
        (
            "guest_intents",
            block(Struct(&INTENT_LIST)).doc("The guests' pickable intents."),
        ),
        (
            "guest_join_chance",
            block_scoped(ScriptedModifier, "character").doc(
                "Chance an invitee accepts; `scope:minimal_travel_time` and \
                 `scope:activity_start_diff_days` describe the trip.",
            ),
        ),
        (
            "on_enter_travel_state",
            block_scoped(Effect, "character").doc("A character starts travelling."),
        ),
        (
            "on_enter_passive_state",
            block_scoped(Effect, "character").doc("A character arrives and waits."),
        ),
        (
            "on_enter_active_state",
            block_scoped(Effect, "character").doc("A character enters an active phase."),
        ),
        (
            "on_leave_travel_state",
            block_scoped(Effect, "character").doc("A character leaves the travel state."),
        ),
        (
            "on_leave_passive_state",
            block_scoped(Effect, "character").doc("A character leaves the passive state."),
        ),
        (
            "on_leave_active_state",
            block_scoped(Effect, "character").doc("A character leaves the active state."),
        ),
        (
            "on_travel_state_pulse",
            block_scoped(Effect, "character").doc("Event pulse for travelling characters."),
        ),
        (
            "on_passive_state_pulse",
            block_scoped(Effect, "character").doc("Event pulse for waiting characters."),
        ),
        (
            "on_active_state_pulse",
            block_scoped(Effect, "character").doc("Event pulse for active characters."),
        ),
        (
            "on_start",
            block_scoped(Effect, "activity").doc("Fires when the activity is created."),
        ),
        (
            "on_complete",
            block_scoped(Effect, "character").doc(
                "Fires after the last phase; AI-only activities persist at most a \
                 day beyond it.",
            ),
        ),
        (
            "activity_window_widgets",
            block(Struct(&OPAQUE))
                .doc("`<widget name> = <container>` plugins for the activity window."),
        ),
        (
            "activity_planner_widgets",
            block(Struct(&OPAQUE))
                .doc("`<widget name> = <container>` plugins for the planner window."),
        ),
        (
            "map_entity",
            scalar_or_block(Setting, Struct(&TRIGGERED_ASSET)).doc(
                "Map entity for the activity — a name, or triggered `reference` \
                 blocks where the first passing entry wins.",
            ),
        ),
        (
            "background",
            block(Struct(&ACTIVITY_BACKGROUND)).doc(
                "Triggered background candidates; the first passing entry wins. \
                 (The event background of the same key covers the event windows.)",
            ),
        ),
        (
            "locale_background",
            block(Struct(&ACTIVITY_BACKGROUND)).doc("As `background`, for the locale window."),
        ),
        (
            "window_characters",
            block(Struct(&WINDOW_CHARACTERS))
                .doc("Characters displayed in the activity window, left to right."),
        ),
        (
            "travel_entourage_selection",
            block(Struct(&TRAVEL_ENTOURAGE))
                .doc("Court characters added to a travelling participant's entourage."),
        ),
        (
            "province_description",
            block(DynamicDesc).doc(
                "Location description while planning; defaults to `<key>_province_desc`. \
                 Root is the province.",
            ),
        ),
        (
            "host_description",
            block(DynamicDesc)
                .doc("Host description while planning; defaults to `<key>_host_desc`."),
        ),
        (
            "guest_description",
            block(DynamicDesc).doc("Guest description; defaults to `<key>_guest_desc` *(corpus)*."),
        ),
        (
            "conclusion_description",
            block(DynamicDesc)
                .doc("Conclusion-screen description; defaults to `<key>_conclusion_desc`."),
        ),
        (
            "pulse_actions",
            block(Struct(&PULSE_ENTRIES))
                .doc("The pulse actions this activity draws from on its event pulse *(corpus)*."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one intent (`_intents.info`; `ai_targets` /
/// `ai_target_quick_trigger` defer to `character_interaction.info`).
static INTENT: StructSpec = StructSpec {
    name: "activity_intent",
    fields: &[
        (
            "is_shown",
            block_scoped(Trigger, "character").doc(
                "Is this intent shown? Root is the picking character; \
                 `scope:magnificence`, `scope:special_option`.",
            ),
        ),
        (
            "is_valid",
            block_scoped(Trigger, "character").doc("Is this intent pickable?"),
        ),
        (
            "is_target_valid",
            block_scoped(Trigger, "character").doc(
                "Is `scope:target` a valid target? Defining a target trigger makes \
                 the intent require one.",
            ),
        ),
        (
            "auto_complete",
            toggle("Complete the intent automatically *(corpus — undocumented)*."),
        ),
        (
            "on_invalidated",
            block_scoped(Effect, "character").doc("Fires when the intent invalidates."),
        ),
        (
            "on_target_invalidated",
            block_scoped(Effect, "character").doc("Fires when `scope:target` invalidates."),
        ),
        (
            "ai_will_do",
            block_scoped(ScriptValue, "character")
                .doc("Weight for the AI's weighted-random intent pick."),
        ),
        (
            "ai_targets",
            block(Struct(&super::character_interaction::AI_TARGETS)).doc(
                "Which characters the AI considers; participants only, and repeatable \
                 to combine lists.",
            ),
        ),
        (
            "ai_target_quick_trigger",
            block(Struct(
                &super::character_interaction::AI_TARGET_QUICK_TRIGGER,
            ))
            .doc("Cheap engine prefilters on candidate targets."),
        ),
        (
            "ai_target_score",
            block_scoped(ScriptValue, "character")
                .doc("Weight for the AI's weighted-random target pick (`scope:target`)."),
        ),
        (
            "icon",
            scalar(Setting).doc("Icon file name; defaults to the intent key."),
        ),
        (
            "scripted_animation",
            scalar_or_block(Setting, Struct(&SCRIPTED_ANIMATION))
                .doc("Animation for characters holding this intent — a key or an inline block."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one pulse action (`_pulse_actions.info`). All script runs with
/// the activity as root; `scope:province` is the current phase location.
static PULSE_ACTION: StructSpec = StructSpec {
    name: "activity_pulse_action",
    fields: &[
        (
            "icon",
            scalar(Setting).doc("Icon name; defaults to the action key."),
        ),
        (
            "is_valid",
            block_scoped(Trigger, "activity").doc("Can this action be picked on the pulse?"),
        ),
        (
            "weight",
            block_scoped(ScriptValue, "activity").doc("Relative weight against all others."),
        ),
        (
            "effect",
            block_scoped(Effect, "activity").doc(
                "Runs when picked; `scope:first` / `scope:second` show in the \
                 activity-window notification.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one locale type (`_activity_locales.info`). Root is the
/// visiting/picking character; `scope:host`, `scope:activity`.
static LOCALE_TYPE: StructSpec = StructSpec {
    name: "activity_locale",
    fields: &[
        (
            "is_available",
            block_scoped(Trigger, "character")
                .doc("Is this locale type valid for the activity? Defaults to true."),
        ),
        (
            "chance",
            block_scoped(ScriptValue, "character")
                .doc("Weight for filling a locale slot (weighted random)."),
        ),
        (
            "on_enter_locale",
            block_scoped(Effect, "character").doc("Fires the locale's event on entry."),
        ),
        (
            "ai_will_do",
            block_scoped(ScriptValue, "character")
                .doc("Weight for the AI choosing to visit (weighted random)."),
        ),
        (
            "cooldown",
            block(Struct(&DURATION)).doc("Days before this locale can be entered again."),
        ),
        (
            "visuals",
            scalar_or_block(Setting, Struct(&TRIGGERED_ASSET)).doc(
                "Widget in gui/activity_locale_widgets — a name, or triggered \
                 `reference` blocks where the first passing entry wins.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one guest invite rule (`_invite_rules.info`).
static INVITE_RULE: StructSpec = StructSpec {
    name: "guest_invite_rule",
    fields: &[(
        "effect",
        block_scoped(Effect, "character").doc(
            "Builds the list: `add_to_list = characters` for every valid guest. Root \
             is the host; `scope:special_option` and one flag scope per option \
             category.",
        ),
    )],
    fallback: Fallback::Deny,
};

/// The body of one activity group type (`_activity_group_types.info`).
static GROUP_TYPE: StructSpec = StructSpec {
    name: "activity_group_type",
    fields: &[
        (
            "sort_order",
            scalar(Setting).doc("Order in the activity view; higher sorts first. Default 0."),
        ),
        (
            "gui_tags",
            block(Config).doc("Gui tags, used to set size etc. in gui views."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Activity;

impl Entity for Activity {
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_TYPE,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_TYPE,
            suffix: "_desc",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_TYPE,
            suffix: "_host_desc",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_TYPE,
            suffix: "_guest_desc",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_TYPE,
            suffix: "_province_desc",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_TYPE,
            suffix: "_conclusion_desc",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_PHASE,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_PHASE,
            suffix: "_desc",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_INTENT,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_INTENT,
            suffix: "_desc",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_LOCALE,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_LOCALE,
            suffix: "_desc",
        },
        ImplicitLocPattern {
            kind: kinds::GUEST_INVITE_RULE,
            suffix: "",
        },
        // Documented, though no vanilla rule carries one yet.
        ImplicitLocPattern {
            kind: kinds::GUEST_INVITE_RULE,
            suffix: "_desc",
        },
        ImplicitLocPattern {
            kind: kinds::ACTIVITY_GROUP_TYPE,
            suffix: "activity_group_type_{}",
        },
    ];

    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::ACTIVITY_TYPE,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: TYPES_DIR,
                shape: DefShape::TopLevel,
            }),
            // `activity_type:X` comes from the derived scope-link rule.
            refs: &[
                anywhere(RefPattern::KeyValue("has_activity_type")),
                anywhere(RefPattern::KeyValue("is_activity_type_on_cooldown")),
                anywhere(RefPattern::KeyValue("ai_attempt_to_host_activity")),
                anywhere(RefPattern::KeyValue("can_host_activity")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ACTIVITY_PHASE,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: TYPES_DIR,
                // Scoped: the same phase key recurs under different activity
                // types, so a repeat gap-fills rather than duplicating.
                shape: DefShape::ScopedChildrenOf {
                    containers: &["phases"],
                },
            }),
            refs: &[
                anywhere(RefPattern::KeyValue("has_current_phase")),
                anywhere(RefPattern::KeyValue("has_phase")),
                anywhere(RefPattern::KeyBlockField("has_phase", "type")),
                anywhere(RefPattern::KeyValue("has_phase_past")),
                anywhere(RefPattern::KeyBlockField("has_phase_past", "type")),
                anywhere(RefPattern::KeyValue("has_phase_future")),
                anywhere(RefPattern::KeyBlockField("has_phase_future", "type")),
                // The guest-subset family's optional `phase = X` field.
                anywhere(RefPattern::KeyBlockField("add_to_guest_subset", "phase")),
                anywhere(RefPattern::KeyBlockField(
                    "remove_from_guest_subset",
                    "phase",
                )),
                anywhere(RefPattern::KeyBlockField("is_in_guest_subset", "phase")),
                anywhere(RefPattern::KeyBlockField("any_guest_subset", "phase")),
                anywhere(RefPattern::KeyBlockField("every_guest_subset", "phase")),
                anywhere(RefPattern::KeyBlockField("random_guest_subset", "phase")),
                anywhere(RefPattern::KeyBlockField("ordered_guest_subset", "phase")),
                in_types(RefPattern::KeyList("blocked_phases")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ACTIVITY_INTENT,
            icon: IconHint::Action,
            defs: Some(DefSource {
                dir_prefix: INTENTS_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                anywhere(RefPattern::KeyValue("has_activity_intent")),
                anywhere(RefPattern::KeyValue("set_activity_intent")),
                // The effect's documented block form (`{ intent = X target = … }`);
                // zero corpus uses today, but a modder following the doc gets it.
                anywhere(RefPattern::KeyBlockField("set_activity_intent", "intent")),
                anywhere(RefPattern::KeyBlockField(
                    "has_completed_activity_intent",
                    "type",
                )),
                in_types(RefPattern::KeyList("intents")),
                in_types(RefPattern::KeyList("player_defaults")),
                in_types(RefPattern::KeyList("blocked_intents")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ACTIVITY_PULSE_ACTION,
            icon: IconHint::Action,
            defs: Some(DefSource {
                dir_prefix: PULSE_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[in_types(RefPattern::KeyList("entries"))],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ACTIVITY_LOCALE,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: LOCALES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                anywhere(RefPattern::KeyValue("has_active_locale")),
                // The inner per-slot lists; the outer `locales` block has only
                // field children, which a KeyList never touches.
                in_types(RefPattern::KeyList("locales")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::GUEST_INVITE_RULE,
            icon: IconHint::Function,
            defs: Some(DefSource {
                dir_prefix: INVITE_RULES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                in_types(RefPattern::KeyWeighted("rules")),
                in_types(RefPattern::KeyWeighted("defaults")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ACTIVITY_GROUP_TYPE,
            icon: IconHint::Hierarchy,
            defs: Some(DefSource {
                dir_prefix: GROUP_TYPES_DIR,
                shape: DefShape::TopLevel,
            }),
            // The one reference is `activity_group_type = X`, carried by the
            // FieldSpec on the activity-type body.
            refs: &[],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (TYPES_DIR, Struct(&ACTIVITY_TYPE)),
        (INTENTS_DIR, Struct(&INTENT)),
        (PULSE_DIR, Struct(&PULSE_ACTION)),
        (LOCALES_DIR, Struct(&LOCALE_TYPE)),
        (INVITE_RULES_DIR, Struct(&INVITE_RULE)),
        (GROUP_TYPES_DIR, Struct(&GROUP_TYPE)),
    ];
}
