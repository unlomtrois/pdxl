//! Player-facing messages (`common/messages/`, from `_messages.info`) — the
//! feed entries and toasts raised by `send_interface_message` — plus the two
//! kinds that organize them in the message-settings window:
//! `common/message_filter_types/` and `common/message_group_types/`.
//!
//! The three form a chain, and modeling only the first would leave most of it
//! dangling: a message names a filter type (665 uses), and a filter type names
//! a group (174). Corpus: every one resolves.
//!
//! Cross-references:
//! - `send_interface_message = { type = X }` (and the `_as_toast` variant)
//!   names a message. `type` is far too overloaded to catch ungated, so the
//!   rule keys on the enclosing effect via [`RefPattern::KeyValueUnder`].
//! - `message_filter_type = X` is ungated: besides a message body it appears in
//!   `send_interface_message`, which the info says overrides the message's own.
//! - `group = X` names a group type, gated to `common/message_filter_types/` —
//!   `group` means something different in eight other directories.
//!
//! Where the info and the corpus disagree, the corpus wins: the Structure
//! section documents `text` for the message title, but no file uses it and 536
//! use `title` — which is also what the info's own EXAMPLES section writes. The
//! same section omits `display = popup`, used twice.
//!
//! **A quarter of messages have no call site in script, and that is correct.**
//! Of 682 definitions, 390 are named by a `send_interface_*` effect and 165 are
//! strings in the game binary — the engine raises those itself, from the siege
//! code (`msg_siege_won`), from trait acquisition (`msg_gain_trait`) and so on.
//! They are listed in `ENGINE_RAISED` and reported through
//! `Schema::is_intrinsic`, so a zero reference count reads as "the engine calls
//! this" rather than "this is dead". The remaining 127 are named by neither,
//! which may mean dead content or a call site built from a computed name — the
//! call-site scan is a windowed heuristic, so treat that number as an upper
//! bound rather than a verdict.
//!
//! Implicit localization, all three documented by the info and corpus-verified:
//! a filter type is `message_filter_<key>` (180/180), a group type is
//! `message_group_type_<key>` (29/31), and a message falls back to its own key
//! when it declares no `title` — 146 of 146 such messages have exactly that.

use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, scalar};
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindId, KindSpec, RefPattern, RefRule,
};

use crate::kinds;

use super::Entity;
use super::common::{anywhere, toggle};

const MESSAGES_DIR: &str = "common/messages/";
const FILTER_DIR: &str = "common/message_filter_types/";
const GROUP_DIR: &str = "common/message_group_types/";

/// The body of one message definition.
static MESSAGE: StructSpec = StructSpec {
    name: "message",
    fields: &[
        (
            "title",
            scalar(LocKey).doc(
                "The message's headline. Defaults to the message's own key \
                 *(the info calls this field `text`; no file does)*.",
            ),
        ),
        (
            "desc",
            scalar(LocKey).doc("Longer text explaining what happened."),
        ),
        (
            "tooltip",
            scalar(LocKey).doc("Hover text for the message item. Default: none."),
        ),
        (
            "style",
            scalar(Setting)
                .doc("How the item reads. Default `neutral`.")
                .values(&["good", "bad", "neutral"]),
        ),
        (
            "display",
            scalar(Setting)
                .doc("Where the message appears. Default `feed`.")
                .values(&["feed", "toast", "popup"]),
        ),
        (
            "message_filter_type",
            scalar(Setting).doc(
                "The filter group this message sits under in message settings. \
                 Overridden by the same key on `send_interface_message`. \
                 Default: empty, which hides it from settings entirely.",
            ),
        ),
        (
            "icon",
            scalar(Setting).doc("Texture under `gfx/interface/message_icons`."),
        ),
        (
            "soundeffect",
            scalar(Setting).doc("Sound played when shown. Default: chosen from display + style."),
        ),
        (
            "flag",
            scalar(Setting).doc(
                "Repeatable customization key. A flagged message needs matching \
                 handling in `hud_notification_templates.gui` — the default toast \
                 only renders for messages carrying no flags.",
            ),
        ),
        (
            "combine_into_one",
            toggle(
                "Merge into an existing message of this type rather than \
                 animating a new one — for high-frequency messages.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one message filter type.
static FILTER_TYPE: StructSpec = StructSpec {
    name: "message_filter_type",
    fields: &[
        (
            "display",
            scalar(Setting)
                .doc("Where messages of this filter appear. Default `feed`.")
                .values(&["feed", "toast", "hidden"]),
        ),
        (
            "group",
            scalar(Setting).doc("The foldable group in message settings. Default `misc`."),
        ),
        (
            "always_show",
            toggle("Stop the player hiding messages of this filter. Default `no`."),
        ),
        (
            "auto_pause",
            toggle("Pause the game when one of these appears. Default `no`."),
        ),
        (
            "sort_order",
            scalar(Setting).doc(
                "Position in message settings, higher first; ties break on \
                 definition order. Default `0`.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one message group type.
static GROUP_TYPE: StructSpec = StructSpec {
    name: "message_group_type",
    fields: &[(
        "sort_order",
        scalar(Setting).doc(
            "Position of the group in message settings, higher first; ties \
             break on definition order. Default `0`.",
        ),
    )],
    fallback: Fallback::Deny,
};

/// A `type = X` message reference, keyed on the effect that encloses it.
const fn message_type(effect: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValueUnder(effect, "type"),
        gate: None,
        alt: &[],
    }
}

/// Messages the engine raises itself, so no `send_interface_*` names them.
///
/// Their call sites are compiled into the game: `msg_siege_won` comes from the
/// siege code, `msg_gain_trait` from trait acquisition. They are live content
/// with zero script references, which is exactly what makes marking them worth
/// the lines — otherwise each reads as dead.
///
/// The last two are the *default* message types the engine substitutes when a
/// `send_interface_toast` block names none, which is why they share an effect's
/// spelling.
///
/// Rebuild after a game patch by intersecting the message definitions with the
/// strings in the binary, minus everything script calls:
///
/// ```sh
/// strings -n 6 "<game>/../binaries/ck3" | grep -xF -f <definitions> \
///   | sort -u | comm -23 - <called-by-script>
/// ```
#[rustfmt::skip]
const ENGINE_RAISED: &[&str] = &[
    "msg_2_agents_removed_my_scheme", "msg_agent_joined_my_scheme", "msg_agent_removed_my_scheme", "msg_alliance",
    "msg_alliance_became_landed", "msg_alliance_became_unlanded", "msg_appointed_to_title", "msg_barter_aborted",
    "msg_became_dynast", "msg_became_head_of_house", "msg_become_culture_head", "msg_becomes_malnourished",
    "msg_becomes_obese", "msg_broken_alliance", "msg_building_done", "msg_cadet_branch_created",
    "msg_catalyst_triggered", "msg_catalyst_triggered_no_character", "msg_change_diarchy_type", "msg_change_primary",
    "msg_consort_invalidated", "msg_contract_completion_reward", "msg_council_swap_position", "msg_council_task_finished",
    "msg_council_task_finished_location", "msg_county_faction_against_liege_created", "msg_county_faction_against_me_created", "msg_court_amenity_setting_invalidated",
    "msg_court_position_gained", "msg_court_position_invalidated_and_replaced_employer", "msg_court_position_invalidated_employee", "msg_court_position_invalidated_employer",
    "msg_court_position_removed", "msg_custom_player_message", "msg_diarchy_less_power", "msg_diarchy_more_power",
    "msg_disbanded_faction_target_died", "msg_dynasty_perk_added", "msg_dynasty_perk_removed", "msg_dynasty_prestige_level_dencrease",
    "msg_dynasty_prestige_level_increase", "msg_end_diarchy", "msg_era_discovered", "msg_event_timeout",
    "msg_expired_alliance", "msg_expired_alliance_ally_death", "msg_faction_against_liege_created", "msg_faction_against_me_created",
    "msg_faction_against_me_disbanded", "msg_faction_forced_to_join", "msg_faction_local_becomes_leader", "msg_faction_local_not_leader_anymore",
    "msg_fascination_discovered", "msg_fired_from_council", "msg_first_era_discovered", "msg_focus_invalidated",
    "msg_gain_nickname", "msg_gain_trait", "msg_governor_candidate_demoted_leading", "msg_governor_candidate_promoted_leading",
    "msg_great_project_aborted", "msg_great_project_completed", "msg_great_project_planned", "msg_great_project_started",
    "msg_holy_order_dismissed", "msg_holy_order_patronage_gained", "msg_holy_order_patronage_lost", "msg_hook_on_me_added",
    "msg_hook_on_me_expired", "msg_hook_on_me_replaced", "msg_house_relation_ended", "msg_house_relation_level_changed_bad",
    "msg_house_relation_level_changed_good", "msg_house_unity_change_stage", "msg_i_became_dynast", "msg_i_became_head_of_faith",
    "msg_i_became_head_of_house", "msg_i_became_head_of_multiple_faiths", "msg_inherit_diarchy", "msg_inherited_single_title",
    "msg_inherited_titles", "msg_innovation_discovered", "msg_i_not_dynast_anymore", "msg_i_not_head_of_faith_anymore",
    "msg_i_not_head_of_house_anymore", "msg_i_not_head_of_multiple_faiths_anymore", "msg_invalidation_of_council_task", "msg_law_invalidated",
    "msg_law_invalidated_no_new_law", "msg_legend_library_added", "msg_legend_promoter_join", "msg_legend_promoter_leave",
    "msg_letter_event_timeout", "msg_liege_changed_budget_law", "msg_liege_passed_law", "msg_liege_passed_title_law",
    "msg_liege_removed_title_law", "msg_lose_trait", "msg_lost_nickname", "msg_lost_single_title",
    "msg_lost_titles", "msg_marriage", "msg_mercenary_company_dismissed", "msg_multiple_agents_removed_my_scheme",
    "msg_my_faction_disbanded", "msg_my_hook_added", "msg_my_hook_expired", "msg_my_hook_replaced",
    "msg_new_fascination_selected", "msg_new_heir", "msg_new_heir_newborn", "msg_new_heir_old_heir_dead",
    "msg_new_theocracy_lesee_approve", "msg_new_theocracy_lesee_disapprove", "msg_no_longer_culture_head", "msg_no_new_heir",
    "msg_no_new_heir_old_heir_dead", "msg_peace_armies_disbanded", "msg_perk_point_added", "msg_player_character_changed",
    "msg_player_new_character", "msg_player_new_observer", "msg_promoted_legend_completed", "msg_promoted_legend_unowned",
    "msg_provincial_army_reassigned", "msg_realm_law_changed", "msg_removed_from_faction_not_liege", "msg_removed_from_faction_not_valid",
    "msg_removed_from_faction_not_valid_leader", "msg_scheme_abandoned", "msg_scheme_froze", "msg_scheme_froze_until",
    "msg_siege_loot", "msg_siege_started", "msg_siege_won", "msg_stops_being_malnourished",
    "msg_stops_being_obese", "msg_struggle_phase_end", "msg_struggle_phase_transitioned", "msg_title_rank_down",
    "msg_title_rank_up", "msg_tributary_invalidated", "msg_troops_disbanding_civil_war", "msg_vassal_contract_2_levels_invalidated",
    "msg_vassal_contract_level_invalidated", "msg_vassal_contract_multiple_levels_invalidated", "msg_war_ally_joined", "msg_war_ally_joined_multiple",
    "msg_war_ally_removed", "msg_war_ally_removed_multiple", "msg_war_ally_replaced", "msg_war_ally_transferred",
    "msg_war_casus_belli_changed", "msg_war_enemy_joined", "msg_war_enemy_joined_multiple", "msg_war_enemy_removed",
    "msg_war_enemy_removed_multiple", "msg_war_enemy_replaced", "msg_war_enemy_transferred", "msg_war_player_joined",
    "msg_war_player_removed", "msg_war_player_replaced", "msg_war_player_transferred", "send_interface_message_as_popup",
    "send_interface_message_as_toast",
];

pub(crate) struct Message;

impl Entity for Message {
    const INTRINSICS: &'static [(KindId, &'static [&'static str])] =
        &[(kinds::MESSAGE, ENGINE_RAISED)];

    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::MESSAGE,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::MESSAGE_FILTER_TYPE,
            suffix: "message_filter_{}",
        },
        ImplicitLocPattern {
            kind: kinds::MESSAGE_GROUP_TYPE,
            suffix: "message_group_type_{}",
        },
    ];

    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::MESSAGE,
            icon: IconHint::Text,
            defs: Some(DefSource {
                dir_prefix: MESSAGES_DIR,
                shape: DefShape::TopLevel,
            }),
            // The effects that raise a message. `send_interface_toast` is by
            // far the common one (11156 uses to `send_interface_message`'s
            // 2001) despite the info naming only the other two.
            // `send_interface_message_as_toast` never appears as an effect in
            // the corpus — it survives here because the info documents it, and
            // exists otherwise only as the *default message* of that name.
            // `send_interface_message_good`/`_bad` are scripted effects
            // wrapping these, so their bodies carry the real reference.
            refs: &[
                message_type("send_interface_toast"),
                message_type("send_interface_message"),
                message_type("send_interface_message_as_toast"),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::MESSAGE_FILTER_TYPE,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: FILTER_DIR,
                shape: DefShape::TopLevel,
            }),
            // Ungated: a message body names one, and so does
            // `send_interface_message`, which overrides it.
            refs: &[anywhere(RefPattern::KeyValue("message_filter_type"))],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::MESSAGE_GROUP_TYPE,
            icon: IconHint::Hierarchy,
            defs: Some(DefSource {
                dir_prefix: GROUP_DIR,
                shape: DefShape::TopLevel,
            }),
            // `group` names eight other things elsewhere, so this is gated to
            // the one directory where it means a message group.
            refs: &[RefRule {
                pattern: RefPattern::KeyValue("group"),
                gate: Some(FILTER_DIR),
                alt: &[],
            }],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (MESSAGES_DIR, Struct(&MESSAGE)),
        (FILTER_DIR, Struct(&FILTER_TYPE)),
        (GROUP_DIR, Struct(&GROUP_TYPE)),
    ];
}
