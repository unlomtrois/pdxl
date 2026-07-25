//! Subject types (`in_game/common/subject_types/`, fully documented by the
//! directory's `readme.txt`) — 20 top-level defs (vassal, march, tributary,
//! colonial nation, HRE membership tiers, …).
//!
//! References (corpus-validated, 0 unresolved): `subject_type = X` (665 —
//! the `subject_type = subject_type:x` / `scope:x` chain forms skip for
//! free) and `subject_type:X` scope literals (151), both anywhere.
//!
//! The `color = X` named-color references live in [`super::named_color`];
//! `subject_pays` / `institution_spread_*` values are script-value names,
//! resolved by the project-level call-by-name pass. Keys marked
//! *(corpus)* are corpus-real but absent from the readme.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Effect, ScriptValue, StaticModifier, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{
    Fallback, FieldSpec, StructSpec, block, color, scalar, scalar_or_block,
};
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::scripted::def_only;

pub(crate) const SUBJECT_TYPES_DIR: &str = "in_game/common/subject_types/";

/// A trigger-block field (the readme names its scopes per key).
const fn trigger(doc: &'static str) -> FieldSpec {
    block(ClauseKind::Trigger).doc(doc)
}

/// A yes/no toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// A script-value field (scalar number, named script value, or block).
const fn value(doc: &'static str) -> FieldSpec {
    scalar_or_block(Setting, ScriptValue).doc(doc)
}

/// `diplo_chance_accept_* = { <factor> = <weight> … }` — dynamic factor keys.
static DIPLO_CHANCE: StructSpec = StructSpec {
    name: "diplo acceptance factors",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// The body of one subject type (`readme.txt` + corpus).
static SUBJECT_TYPE: StructSpec = StructSpec {
    name: "subject type",
    fields: &[
        // Availability triggers (root = overlord, target = subject).
        ("visible", trigger("Is this subject type visible at all (root = overlord, target = subject).")),
        ("enabled", trigger("Is this subject type enabled at all (root = overlord, target = potential subject).")),
        ("visible_through_diplomacy", trigger("Visible through diplomacy, additional to `visible`.")),
        ("enabled_through_diplomacy", trigger("Enabled through diplomacy, additional to `enabled`.")),
        ("visible_through_treaty", trigger("Visible in a peace treaty (recipient/war scopes available).")),
        ("enabled_through_treaty", trigger("Enabled in a peace treaty (recipient/war scopes available).")),
        ("creation_visible", trigger("Can this subject type be created in general (root = overlord).")),
        ("subject_creation_enabled", trigger("Can a province/building subject be released as this type (target_province scope).")),
        ("release_country_enabled", trigger("Can the overlord create released countries as this type.")),
        // Relationship triggers (root = subject type).
        ("can_attack", trigger("Can attack (overlord/subject/attacker/defender scopes).")),
        ("can_rival", trigger("Can rival (actor/recipient scopes).")),
        ("can_marry", trigger("Can marry (actor/recipient scopes).")),
        ("join_offensive_wars_always", trigger("Automatically joins the other's offensive wars (actor/recipient/target scopes).")),
        ("join_offensive_wars_auto_call", trigger("Automatically called to arms in offensive wars.")),
        ("join_offensive_wars_can_call", trigger("Allows a call to arms in offensive wars.")),
        ("join_defensive_wars_always", trigger("Automatically joins the other's defensive wars.")),
        ("join_defensive_wars_auto_call", trigger("Automatically called to arms in defensive wars.")),
        ("join_defensive_wars_can_call", trigger("Allows a call to arms in defensive wars.")),
        ("allow_declaring_wars", trigger("Whether the subject can declare its own wars (attacker/defender scopes).")),
        // Lifecycle effects (root = subject).
        ("on_enable", block(Effect).doc("Run when the subject relation is created (`future_overlord` scope).")),
        ("on_disable", block(Effect).doc("Run when the subject relation is broken (`former_overlord` scope).")),
        ("on_monthly", block(Effect).doc("Run on the subject each month.")),
        // Modifiers.
        ("overlord_modifier", block(StaticModifier).doc("Modifiers given to the overlord country.")),
        ("subject_modifier", block(StaticModifier).doc("Modifiers given to the subject country.")),
        // Script values.
        ("annexation_speed", value("Annexation progress per month (default 1; actor/target scopes).")),
        ("war_score_cost", value("War-score cost to establish in wars (modifies the base calculation).")),
        ("base_antagonism", value("Max antagonism created when established in wars (overrides code unless ≤ 0).")),
        ("monthly_favor_gain", value("Extra favors gained in the relationship (overlord/subject scopes).")),
        ("institution_spread_to_overlord", value("How fast embraced institutions spread subject → overlord capital.")),
        ("institution_spread_to_subject", value("How fast embraced institutions spread overlord → subject capital.")),
        ("subject_pays", value("What the subject pays the overlord monthly (a price script value).")),
        ("ai_wants_to_be_overlord", value("AI desire to become this overlord (overlord/subject scopes).")),
        ("ai_wants_to_be_subject", value("AI desire to become this subject (overlord/subject scopes).")),
        // Acceptance factor maps.
        ("diplo_chance_accept_subject", block(Struct(&DIPLO_CHANCE)).doc("Acceptance factors for becoming this subject (`<factor> = <weight>`).")),
        ("diplo_chance_accept_overlord", block(Struct(&DIPLO_CHANCE)).doc("Acceptance factors for becoming this overlord (`<factor> = <weight>`).")),
        // Identity & data.
        ("color", color().doc("The subject type's map/UI color — a named color or literal.")),
        ("level", scalar(Setting).doc("Subject level: higher = less autonomy; rule of thumb, 3 = annexation-track, 0 = subject in name only.")),
        (
            "type",
            scalar(Setting)
                .doc("The kind of country valid for this subject type (also accepts a `special_status:` literal).")
                .values(&["location", "pop", "building", "army"]),
        ),
        ("government", scalar(Setting).doc("Government type given to the subject on creation.")),
        ("great_power_score_transfer", scalar(Setting).doc("Fraction of the subject's great-power score added to the overlord.")),
        ("minimum_opinion_for_offer", scalar(Setting).doc("Minimum subject opinion of the overlord before this can be offered.")),
        ("annexation_min_years_before", scalar(Setting).doc("Years the relation must exist before annexation.")),
        ("annexation_min_opinion", scalar(Setting).doc("Minimum opinion before annexation can start.")),
        ("annexation_stall_opinion", scalar(Setting).doc("Opinion below which annexation halts.")),
        ("annullment_favours_required", scalar(Setting).doc("Favours needed to annul this membership diplomatically.")),
        ("diplomatic_capacity_cost_scale", scalar(Setting).doc("Diplomatic-capacity cost multiplier.")),
        ("strength_vs_overlord", scalar(Setting).doc("Relative-strength adjustment vs the overlord.")),
        ("maritime_path_tolerance", scalar(Setting).doc("Maritime path tolerance adjustment. *(corpus)*")),
        ("merchants_to_overlord_fraction", scalar(Setting).doc("Fraction of merchant power that goes to the overlord. *(corpus)*")),
        ("content_priority", scalar(Setting).doc("UI ordering priority. *(corpus)*")),
        (
            "on_overlord_becomes_a_subject",
            scalar(Setting)
                .doc("What happens when the overlord itself becomes a subject (default `nothing` = stays as sub-subject).")
                .values(&["cancel_subjects", "transfer_subjects", "nothing"]),
        ),
        // Toggles.
        ("can_be_annexed", toggle("Whether the subject type can be annexed at all (default yes).")),
        ("annulled_by_peace_treaty", toggle("Broken/unavailable when treaties get annulled (default yes).")),
        ("use_overlord_map_color", toggle("Show in the overlord's color on the map.")),
        ("use_overlord_map_name", toggle("Show under the overlord's name on the map.")),
        ("use_overlord_laws", toggle("Uses the overlord's laws and policies. *(corpus)*")),
        ("only_overlord_culture", toggle("Subject must have the overlord's culture.")),
        ("only_overlord_or_kindred_culture", toggle("Subject must have the overlord's culture or a kindred one.")),
        ("only_overlord_court_language", toggle("Subject must have the overlord's court language.")),
        ("can_overlord_recruit_regiments", toggle("Overlord can recruit regiments in the subject's territory.")),
        ("can_overlord_build_ships", toggle("Overlord can build ships in the subject's ports.")),
        ("can_overlord_build_roads", toggle("Overlord can build roads in the subject's land.")),
        ("can_overlord_build_buildings", toggle("Overlord can build buildings in the subject's locations.")),
        ("can_overlord_build_rgos", toggle("Overlord can build RGOs in the subject's locations.")),
        ("overlord_share_exploration", toggle("Overlord shares their exploration.")),
        ("shares_exploration_with_overlord", toggle("Subject shares exploration with the overlord. *(corpus)*")),
        ("overlord_protects_external", toggle("Overlord protects against external attackers (default yes).")),
        ("overlord_protects_other_subjects", toggle("Overlord protects against other subjects (default no).")),
        ("counts_as_external", toggle("Attacking fellow subjects counts as an external threat (default no).")),
        ("can_be_force_broken_in_peace_treaty", toggle("Can be demanded broken in a peace treaty.")),
        ("overlord_can_enforce_peace_on_subject", toggle("Overlord can demand the subject leave its wars.")),
        ("has_overlords_ruler", toggle("The subject shares the overlord's ruler.")),
        ("has_overlords_religion", toggle("The subject must share the overlord's religion. *(corpus)*")),
        ("overlord_inherit_if_no_heir", toggle("Overlord inherits when the subject has no heir. *(corpus)*")),
        ("will_join_independence_wars", toggle("Joins other subjects' independence wars.")),
        ("subject_can_cancel", toggle("The subject can cancel the relation.")),
        ("overlord_can_cancel", toggle("The overlord can cancel the relation.")),
        ("has_limited_diplomacy", toggle("The subject has limited diplomacy.")),
        ("food_access", toggle("The subject shares food access.")),
        ("can_change_rank", toggle("The subject can change country rank.")),
        ("can_change_heir_selection", toggle("The subject can change heir selection.")),
        ("allow_subjects", toggle("The subject may hold subjects of its own. *(corpus)*")),
        ("fleet_basing_rights", toggle("Fleet basing rights between the two. *(corpus)*")),
        ("is_colonial_subject", toggle("Counts as a colonial subject. *(corpus)*")),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct SubjectType;

impl Entity for SubjectType {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        refs: &[
            RefRule {
                pattern: RefPattern::KeyValue("subject_type"),
                gate: None,
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::ScopePrefix("subject_type"),
                gate: None,
                alt: &[],
            },
            // Advances unlock subject types (8 refs, 0 unresolved).
            RefRule {
                pattern: RefPattern::KeyValue("unlock_subject_type"),
                gate: None,
                alt: &[],
            },
        ],
        ..def_only(kinds::SUBJECT_TYPE, IconHint::Hierarchy, SUBJECT_TYPES_DIR)
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(SUBJECT_TYPES_DIR, ClauseKind::Struct(&SUBJECT_TYPE))];
}
