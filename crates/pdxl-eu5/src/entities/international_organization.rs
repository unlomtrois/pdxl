//! International organizations (`in_game/common/international_organizations/`,
//! fully documented by the directory readme) — 33 top-level defs (HRE,
//! catholic church, unions, leagues, …) plus their three companion
//! directories: special statuses, payments, and land-ownership rules.
//!
//! References (corpus-validated, 0 unresolved):
//! - `international_organization:X` / `international_organization_type:X` /
//!   `special_status:X` literals are table-derived (`crate::derived` — the IO
//!   type-tag link alone carries 3,600+ refs);
//! - `international_organization_type = X` (bare type comparisons) and
//!   `is_member_of_international_organization_of_type = X`, script-wide;
//! - `special_statuses_implemented` / `payments_implemented` list items and
//!   `land_ownership_rule = X`, gated to the IO directory;
//! - the `declare_war_on_target_casus_belli` and `laws = { <law> = <policy> }`
//!   rules live with their target kinds in [`super::unlocks`].
//!
//! `custom_name = X` references live with the customizable-localization target
//! kind; `leadership_election_resolution` names a parliament resolution
//! (unmodeled, 3 refs); the readme's
//! `<currency type> = yes` treasury toggles are dynamic — the IO body keeps
//! [`Fallback::Ignore`] for them.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{
    self, Effect, ScriptValue, StaticModifier, Struct, Trigger,
};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{
    Fallback, FieldSpec, StructSpec, block, block_scoped, color, scalar, scalar_or_block,
};
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::SCALED_MODIFIER;
use super::scripted::def_only;

pub(crate) const IO_DIR: &str = "in_game/common/international_organizations/";
const STATUSES_DIR: &str = "in_game/common/international_organization_special_statuses/";
const PAYMENTS_DIR: &str = "in_game/common/international_organization_payments/";
const LAND_RULES_DIR: &str = "in_game/common/international_organization_land_ownership_rules/";

/// A trigger whose root is a country (member / joiner / enemy / leader …).
const fn country_trigger(doc: &'static str) -> FieldSpec {
    block_scoped(Trigger, "country").doc(doc)
}

/// A trigger whose root is the international organization itself.
const fn io_trigger(doc: &'static str) -> FieldSpec {
    block_scoped(Trigger, "international_organization").doc(doc)
}

/// A yes/no toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// A script-value field (scalar number, named script value, or block).
const fn value(doc: &'static str) -> FieldSpec {
    scalar_or_block(Setting, ScriptValue).doc(doc)
}

/// `variables = { <name> = { … } }` — dynamic variable names.
static VARIABLES: StructSpec = StructSpec {
    name: "organization variables",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// `special_statuses_implemented` / `payments_implemented` / `laws` — list
/// or map bodies whose items are references (rules live with their kinds).
static REF_LIST: StructSpec = StructSpec {
    name: "reference list",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// The body of one international organization (`readme.txt` + corpus).
static INTERNATIONAL_ORGANIZATION: StructSpec = StructSpec {
    name: "international organization",
    fields: &[
        // Identity & display.
        ("should_show_ruler_history", toggle("Show the ruler/leader history button in the IO view.")),
        ("background_texture", scalar(Setting).doc("Picture shown in the IO panel background (defaults to DEFAULT_IO_BACKGROUND_PATH).")),
        ("show_strength_comparison_with_target", toggle("Show the strength comparison between IO and target in the UI.")),
        ("unique", toggle("Only one organization of this type may exist.")),
        ("custom_name", scalar(Setting).doc("Scripted link to a custom string key in `customizable_localization/` (ROOT = IO, location = seat).")),
        ("show_on_diplomatic_map", toggle("Show this organization on the diplomatic map.")),
        ("show_as_overlord_on_map_trigger", country_trigger("Show the IO's own color/name as an overlord on the political map (root = member; scope:recipient = IO).")),
        ("show_leave_message", toggle("Show a message when a country leaves the IO (default yes).")),
        ("map_color_override", value("Optional color overriding leader/member colors (root = location, scope:recipient = IO).")),
        ("secondary_map_color_override", value("Optional color overriding the striped colors (root = location, scope:recipient = IO).")),
        ("tooltip", value("Returns a loc key shown as the location tooltip (root = location, scope:recipient = IO).")),
        ("leader_color", color().doc("Color of the leader on the diplomatic map.")),
        ("member_color", color().doc("Color of members on the diplomatic map.")),
        ("target_color", color().doc("Color of the target on the diplomatic map.")),
        ("fog_of_war_lifted", toggle("Members see each other through fog of war. *(corpus)*")),
        ("alert_view_tab", scalar(Setting).doc("Which IO-view tab the alert opens. *(corpus)*")),
        // Target & enemies.
        ("has_target", toggle("Does this type have a target country.")),
        ("declare_war_on_target_casus_belli", scalar(Setting).doc("CB used to declare war on the target from the IO page.")),
        ("potential_target_trigger", country_trigger("Are we close to becoming a target of this type (root = potential enemy; UI alerts).")),
        ("can_target_trigger", country_trigger("Can we become a target of this organization type (root = potential enemy).")),
        ("has_enemies", toggle("Does this type allow enemies to target it.")),
        ("can_be_enemy_trigger", country_trigger("Can we become an enemy of this type (root = potential enemy, recipient = organization).")),
        // Creation, invitation, joining, leaving.
        ("create_visible_trigger", country_trigger("Can we see a diplo action to create these (root = potential creator; default yes).")),
        ("create_enabled_trigger", country_trigger("Is the option to create these enabled (root = potential creator; default yes).")),
        ("invite_visible_trigger", country_trigger("Can we see a diplo action to invite members (root = inviter, scope:recipient = IO, scope:target = target).")),
        ("invite_enabled_trigger", country_trigger("Is the invite action enabled (root = inviter, scope:recipient = IO, scope:target = target).")),
        ("join_visible_trigger", country_trigger("Can we see a diplo action to join the IO (root = joiner, scope:recipient = IO).")),
        ("join_enabled_trigger", country_trigger("Is the join action enabled (root = joiner, scope:recipient = IO).")),
        ("can_join_trigger", country_trigger("Can we join this type (root = joiner, actor = potential leader, recipient = existing organization, target = target).")),
        ("can_leave_trigger", country_trigger("Can we leave this type (root = leaver, recipient = existing organization).")),
        ("auto_leave_trigger", country_trigger("Auto-leave trigger (root = country, scope:recipient = the organization).")),
        ("auto_disband_trigger", io_trigger("When the organization disbands itself (root = IO, scope:target = against who).")),
        ("disband_message_trigger", io_trigger("Only show the disband message when fulfilled (root = IO, actor = instigator).")),
        ("disband_minimum_member_count", scalar(Setting).doc("Disband below this member count. *(corpus)*")),
        ("join_diplo_chance", value("Acceptance chance factors for joining. *(corpus)*")),
        ("expel_members_who_are_targets_of_other_members", toggle("Come on.")),
        ("expel_members_who_target_the_leader", toggle("Expel members who target the leader.")),
        ("expel_members_who_are_attackers_at_war_with_other_members", toggle("Expel attacking members at war with other members.")),
        ("expel_members_who_are_defenders_at_war_with_other_members", toggle("Expel defending members at war with other members.")),
        ("use_laws_as_join_reason", toggle("AI considers the IO's enacted laws/policies as reasons to join (default yes).")),
        // Leadership.
        ("has_leader_country", toggle("Does this type have a leader country (default no).")),
        ("override_ruler_title", toggle("Whether this title outranks the ruler's country titles (for character leaders).")),
        ("leader_title_key", scalar(Setting).doc("Loc key for the leader title (character leaders get _MALE/_FEMALE appended).")),
        ("title_is_suffix", toggle("Whether the title goes at the end of the name.")),
        ("leader", block_scoped(Effect, "international_organization").doc("Fills the `leaders` list with the display leader(s) (root = the organization).")),
        ("leader_type", scalar(Setting).doc("Is the leader a country or a person (default country).").values(&["none", "country", "character"])),
        ("use_regnal_number", toggle("For character leaders: use regnal numbers.")),
        ("leader_change_trigger_type", scalar(Setting).doc("How a new leader gets chosen.").values(&["none", "rulerchange", "timed"])),
        ("leader_change_method", scalar(Setting).doc("How the leadership changes.").values(&["rotation", "vote", "lottery", "score", "none"])),
        ("leadership_election_resolution", scalar(Setting).doc("Resolution key of the IO's leadership voting system.")),
        ("months_between_leader_changes", scalar(Setting).doc("Months between leader changes (timed trigger type only).")),
        ("can_lead_trigger", country_trigger("Can we lead this organization type (root = potential leader).")),
        ("can_lead_tooltip_trigger", country_trigger("Tooltip-only variant of the lead trigger. *(corpus)*")),
        ("leader_score", value("Who should lead when `leader_change_method = score` (root = potential leader, recipient = organization).")),
        ("disband_if_no_leader", toggle("Disband when no leader can be found (default yes).")),
        ("promote_strongest_member_to_war_leader", toggle("Promote the strongest member to war leader. *(corpus)*")),
        // Parliament & resolutions.
        ("has_parliament", toggle("Whether this IO has a parliament system (default no).")),
        ("parliament_type", scalar(Setting).doc("Parliament type used by default (must be defined for IOs).")),
        ("resolution_widget", scalar(Setting).doc("Widget toggled by the resolutions-to-vote alert.")),
        ("max_active_resolutions", scalar(Setting).doc("Max number of resolutions active at once.")),
        ("can_initiate_policy_votes", country_trigger("Can a country initiate policy votes in the IO (root = country, recipient = IO).")),
        ("can_vote_in_parliament", country_trigger("Can a member vote in the parliament. *(corpus)*")),
        ("laws", block(Struct(&REF_LIST)).doc("Laws (and their initial policies) enacted when the IO is set up. *(corpus)*")),
        ("special_statuses_implemented", block(Struct(&REF_LIST)).doc("Special statuses available initially (from `international_organization_special_statuses/`).")),
        ("gold", toggle("Give the organization a treasury which can hold gold (the only currency enabled by a vanilla IO).")),
        ("payments_implemented", block(Struct(&REF_LIST)).doc("Payments implemented initially (from `international_organization_payments/`).")),
        // Land & buildings.
        ("land_ownership_rule", scalar(Setting).doc("Optional land-ownership rules (from `international_organization_land_ownership_rules/`).")),
        ("has_buildings", toggle("Can buildings be linked to this IO (owned by the leader, IO pays; destroyed with the IO).")),
        ("has_dynastic_power", toggle("Can influential dynasties act within this IO (for land-owning IOs).")),
        ("max_circles_at_formation", scalar(Setting).doc("HRE: imperial circles created at formation. *(corpus)*")),
        // Wars & military.
        ("join_defensive_wars_always", io_trigger("Automatically join defensive wars with a fellow member (root = IO, actor = caller, recipient = callee).")),
        ("join_defensive_wars_auto_call", io_trigger("Automatically get a call to arms in defensive wars (root = IO, actor = caller, recipient = callee).")),
        ("join_defensive_wars_can_call", io_trigger("Allow a call to arms in defensive wars (root = IO, actor = caller, recipient = callee).")),
        ("join_offensive_wars_always", io_trigger("Automatically join offensive wars with a fellow member (root = IO, actor = caller, recipient = callee).")),
        ("join_offensive_wars_auto_call", io_trigger("Automatically get a call to arms in offensive wars (root = IO, actor = caller, recipient = callee).")),
        ("join_offensive_wars_can_call", io_trigger("Allow a call to arms in offensive wars (root = IO, actor = caller, recipient = callee).")),
        ("join_defensive_wars", io_trigger("Join defensive wars. *(corpus)*")),
        ("join_offensive_wars", io_trigger("Join offensive wars. *(corpus)*")),
        ("only_leader_country_joins_defensive_wars", toggle("Only call the leader in defensive wars (default no; big-IO performance).")),
        ("only_leader_country_joins_offensive_wars", toggle("Only call the leader in offensive wars (default no; big-IO performance).")),
        ("joins_defensive_wars_as_co_belligerent", toggle("Members join defensive wars as co-belligerents (default no).")),
        ("joins_offensive_wars_as_co_belligerent", toggle("Members join offensive wars as co-belligerents (default no).")),
        ("take_over_wars_when_called", toggle("The leader takes over as warleader when joining as co-belligerent (default no).")),
        ("can_declare_war", io_trigger("Can war be declared (attacker/defender scopes, recipient = organization).")),
        ("has_military_access", io_trigger("Does a country have military access in another (root = IO, actor/recipient = the two countries).")),
        ("gives_military_access_to_all_when_at_war", toggle("Optimized access: any war member in the IO grants everyone in the war access.")),
        ("fleet_basing_rights", io_trigger("Does a country have fleet basing rights in another (root = IO, actor/recipient = the two countries).")),
        ("can_recruit_regiments_in_members", toggle("Members can recruit regiments in other members' locations.")),
        ("can_build_ships_in_members", toggle("Members can build ships in other members' locations.")),
        ("can_build_roads_in_members", toggle("Members can build roads in other members' locations.")),
        ("can_build_buildings_in_members", toggle("Members can build buildings in other members' locations.")),
        ("can_build_rgos_in_members", toggle("Members can build RGOs in other members' locations.")),
        // Diplomacy & opinion.
        ("subject_limited", toggle("Creation/maintenance limited for subjects with limited diplomacy (default yes).")),
        ("can_invite_countries", toggle("Can this IO invite other countries to join (default yes).")),
        ("gives_food_access_to_members", toggle("Automatically gives food access to members (default no).")),
        ("diplomatic_capacity_cost", value("Diplomatic capacity used by this IO (root = country, scope:recipient = IO).")),
        ("annulled_by_peace_treaty", toggle("An annul-treaties peace treaty forces a country out of this IO.")),
        ("annullment_favours_required", scalar(Setting).doc("Favours needed to annul this membership diplomatically.")),
        ("opinion_bonus", scalar(Setting).doc("Additional opinion bonus applied to all members.")),
        ("opinion_trust", scalar(Setting).doc("Additional trust bonus applied to all members.")),
        ("min_opinion", scalar(Setting).doc("Members must have at least this opinion of each other or the IO breaks (expensive in large IOs).")),
        ("min_trust", scalar(Setting).doc("Members must have at least this much trust in each other or the IO breaks (expensive in large IOs).")),
        ("antagonism_towards_leader_modifier", scalar(Setting).doc("Modifier for antagonism accumulated by members towards the leader.")),
        ("antagonism_modifier_for_taking_land_from_fellow_member", scalar(Setting).doc("Antagonism modifier for actions in IO territory between two members.")),
        ("antagonism_modifier_for_taking_land_from_member_as_outsider", scalar(Setting).doc("Antagonism modifier for outsiders acting against members in IO territory.")),
        ("no_cb_price_modifier_for_fellow_member", scalar(Setting).doc("Price modifier for no-CB wars against fellow members.")),
        // Member annexation.
        ("allow_member_annexation", toggle("Members may diplomatically annex each other regardless of subject status.")),
        ("annexation_min_years_before", value("Years of membership before a member can be annexed (root = annexer, scope:target = annexed, scope:recipient = IO).")),
        ("can_annex_members", country_trigger("Conditions before a member can start annexing another (root = annexer, scope:target = annexed, scope:recipient = IO).")),
        ("can_annex_visible", country_trigger("Conditions before the annexation action is even visible (root = annexer, scope:target = annexed, scope:recipient = IO).")),
        ("annexation_speed", value("How long members take to annex each other (root/scope:actor = annexer, scope:target = annexed, scope:recipient = IO).")),
        // Lifecycle effects.
        ("on_creation", block_scoped(Effect, "international_organization").doc("Fired when the organization gets created (root = IO, actor = creator, target = target).")),
        ("on_disband", block_scoped(Effect, "international_organization").doc("Fired when the organization gets disbanded (root = IO).")),
        ("on_joined", block_scoped(Effect, "country").doc("Fired when a country has joined (root = country, scope:recipient = organization).")),
        ("on_left", block_scoped(Effect, "country").doc("Fired when a country has left (root = country, scope:recipient = organization).")),
        ("monthly_effect", block_scoped(Effect, "international_organization").doc("Fired every monthly tick (root = IO).")),
        ("variables", block(Struct(&VARIABLES)).doc("Variables attached to this organization (`<name> = { format/start/min/max/monthly_change … }`).")),
        // Modifiers.
        ("modifier", block(Struct(&SCALED_MODIFIER)).doc("Scaled modifiers applied to members (root = country, scope:recipient = organization).")),
        ("leader_modifier", block(Struct(&SCALED_MODIFIER)).doc("Scaled modifiers applied to the leader.")),
        ("non_leader_modifier", block(Struct(&SCALED_MODIFIER)).doc("Scaled modifiers applied to every member who is not the leader.")),
        ("target_modifier", block(Struct(&SCALED_MODIFIER)).doc("Scaled modifiers applied to the target of the IO.")),
        ("owned_location_modifier", block(StaticModifier).doc("Modifiers applied to every location owned by the IO (not scaled).")),
        ("international_organization_modifier", block(StaticModifier).doc("Modifiers applied to the international organization itself.")),
        ("imperial_circle_leader_modifier", block(StaticModifier).doc("HRE: modifiers applied to imperial-circle leaders. *(corpus)*")),
        // AI.
        ("ai_desire_to_join", value("AI desire to join (root = joiner, actor = potential leader, recipient = organization, target = target).")),
        ("ai_desire_to_allow_new_member", value("AI desire to accept a new member (root = IO, actor = candidate, target = target).")),
        ("ai_desire_to_attack_other_members", value("AI desire to attack other members (root = attacker, defender = attacked, recipient = IO).")),
        ("ai_issue_voting_bias", value("Extra reasoning in resolutions (with the ai_issue_voting_bias trigger).")),
    ],
    // Other `<currency type> = yes` treasury toggles remain dynamically valid;
    // `gold` is explicit because it is the only currency used by the corpus.
    fallback: Fallback::Ignore,
};

/// The body of one special status (readme + corpus; the readme's
/// `auto_dismissal_trigger` ships as `auto_rescind_trigger` in the corpus).
static SPECIAL_STATUS: StructSpec = StructSpec {
    name: "special status",
    fields: &[
        // Limits and presentation priority come first in most vanilla rows.
        ("max_countries", value("Maximum number of countries with this status (root = IO, scope:source = status).")),
        ("priority", scalar(Setting).doc("GUI-list importance; higher is better (default 1; 0 = equal to regular membership).")),
        // Identity/role flags belong with the header, before behavior blocks.
        ("leader", toggle("This status marks the organization's leader. *(corpus)*")),
        ("elector", toggle("HRE: this status votes in imperial elections. *(corpus)*")),
        ("can_be_invited", toggle("Countries can be invited into this status. *(corpus)*")),
        // Passive behavior before conditional acquisition/loss logic.
        ("modifier", block(StaticModifier).doc("Modifier applied to countries with this status.")),
        ("leader_modifier", block(StaticModifier).doc("Modifier applied to the leader, multiplied by the number of countries with this status.")),
        ("can_bestow_trigger", country_trigger("Can a country have this status (root = country, scope:recipient = organization, scope:source = status).")),
        ("auto_bestowal_trigger", country_trigger("Should a country automatically gain this status.")),
        ("auto_rescind_trigger", country_trigger("Should a country automatically lose this status.")),
        ("on_bestowed_effect", block_scoped(Effect, "country").doc("Fired when the status is set on a country.")),
        ("on_rescinded_effect", block_scoped(Effect, "country").doc("Fired when the status is removed from a country.")),
        // Political weight, then map presentation.
        ("special_status_power", value("Political power of this status group, for IO parliament issues.")),
        ("map_color", value("Scripted color on the map (root = country, scope:recipient = organization).")),
    ],
    fallback: Fallback::Deny,
};

/// The body of one payment (readme-complete).
static IO_PAYMENT: StructSpec = StructSpec {
    name: "organization payment",
    fields: &[
        (
            "get_payer_list",
            block_scoped(Effect, "international_organization")
                .doc("Populates the `payers` local variable list (root = IO)."),
        ),
        (
            "get_payee_list",
            block_scoped(Effect, "international_organization")
                .doc("Populates the `payees` local variable list (root = IO)."),
        ),
        (
            "price",
            scalar(Setting).doc("Base price of the total payment (a price script value)."),
        ),
        (
            "price_multiplier",
            value("Multiplier to arrive at the total amount transferred (root = IO)."),
        ),
        (
            "uses_maintenance",
            toggle("Does this payment add a maintenance slider."),
        ),
        (
            "maintenance_modifier",
            block(StaticModifier).doc(
                "Modifier scaled by the country's payment maintenance (penalise short payers).",
            ),
        ),
        (
            "proportion_for_payer",
            value(
                "How much of the price this country pays (root = country, scope:recipient = IO).",
            ),
        ),
        (
            "proportion_for_payee",
            value(
                "How much of the price this country receives (root = country, scope:recipient = IO).",
            ),
        ),
        (
            "min_slider_value",
            scalar(Setting).doc("0..1 — how much the slider can reduce the payment (default 0.5)."),
        ),
        (
            "ai_maintenance_value",
            value("0..1 — AI slider value (root = country, recipient = IO)."),
        ),
        (
            "ai_maintenance_ignore_saving",
            toggle("Keep the AI maintenance value even in saving mode (default no)."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one land-ownership rule (readme-complete).
static LAND_OWNERSHIP_RULE: StructSpec = StructSpec {
    name: "land ownership rule",
    fields: &[
        (
            "modifier",
            block(StaticModifier)
                .doc("Applied to locations owned by the international organization."),
        ),
        (
            "can_add_trigger",
            io_trigger("Can locations be added to the organization in general (root = IO)."),
        ),
        (
            "can_remove_trigger",
            io_trigger("Can locations be removed from the organization in general (root = IO)."),
        ),
        (
            "can_add_location_trigger",
            block_scoped(Trigger, "location")
                .doc("Can this location be added (root = location, recipient = IO)."),
        ),
        (
            "can_remove_location_trigger",
            block_scoped(Trigger, "location")
                .doc("Can this location be removed (root = location, recipient = IO)."),
        ),
        (
            "on_added",
            block_scoped(Effect, "location")
                .doc("Fired when a location is added (root = location, recipient = IO)."),
        ),
        (
            "on_removed",
            block_scoped(Effect, "location")
                .doc("Fired when a location is removed (root = location, recipient = IO)."),
        ),
        (
            "ai_desire_to_add",
            value("How much the owner wants to add the land (root = location, recipient = IO)."),
        ),
        (
            "owned_location_color",
            color().doc("Color of owned locations on the diplomatic map (stripes)."),
        ),
        (
            "removed_by_peace_treaty",
            toggle("Land can only be removed from the IO via peace treaties (default no)."),
        ),
        (
            "remove_war_score_modifier",
            scalar(Setting).doc(
                "War-score cost modifier for removing provinces (needs removed_by_peace_treaty).",
            ),
        ),
        (
            "allow_control_propagation",
            toggle("Allow control propagation through IO land. *(corpus)*"),
        ),
    ],
    fallback: Fallback::Deny,
};

/// A `key = X` reference gated to the IO directory.
const fn in_io(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(IO_DIR),
        alt: &[],
    }
}

pub(crate) struct InternationalOrganization;

impl Entity for InternationalOrganization {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            // The `international_organization:` / `international_organization_type:`
            // literals are table-derived (`crate::derived`).
            refs: &[
                RefRule {
                    pattern: RefPattern::KeyValue("international_organization_type"),
                    gate: None,
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue(
                        "is_member_of_international_organization_of_type",
                    ),
                    gate: None,
                    alt: &[],
                },
            ],
            ..def_only(
                kinds::INTERNATIONAL_ORGANIZATION,
                IconHint::Hierarchy,
                IO_DIR,
            )
        },
        KindSpec {
            // The `special_status:` literal is table-derived (`crate::derived`).
            refs: &[RefRule {
                pattern: RefPattern::KeyList("special_statuses_implemented"),
                gate: Some(IO_DIR),
                alt: &[],
            }],
            ..def_only(kinds::IO_SPECIAL_STATUS, IconHint::Tag, STATUSES_DIR)
        },
        KindSpec {
            refs: &[RefRule {
                pattern: RefPattern::KeyList("payments_implemented"),
                gate: Some(IO_DIR),
                alt: &[],
            }],
            ..def_only(kinds::IO_PAYMENT, IconHint::Action, PAYMENTS_DIR)
        },
        KindSpec {
            refs: &[in_io("land_ownership_rule")],
            ..def_only(
                kinds::IO_LAND_OWNERSHIP_RULE,
                IconHint::Object,
                LAND_RULES_DIR,
            )
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (IO_DIR, ClauseKind::Struct(&INTERNATIONAL_ORGANIZATION)),
        (STATUSES_DIR, ClauseKind::Struct(&SPECIAL_STATUS)),
        (PAYMENTS_DIR, ClauseKind::Struct(&IO_PAYMENT)),
        (LAND_RULES_DIR, ClauseKind::Struct(&LAND_OWNERSHIP_RULE)),
    ];
}
