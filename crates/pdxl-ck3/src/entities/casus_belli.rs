//! Casus belli (`common/casus_belli_types/` + `common/casus_belli_groups/`,
//! from `_casus_belli.info`) — war justifications and the groups that add
//! shared restrictions on top of them.
//!
//! Cross-references (corpus-validated, vanilla + T4N):
//! - `casus_belli = X`, `cb = X` (scalar) and `using_cb = X` resolve to a CB
//!   type *anywhere* — all three keys are unambiguous in the whole corpus
//!   (0 unresolved).
//! - `cb = { X Y … }` — the list form inside `ai_start_best_war` (same key,
//!   different node kind; `KeyValue` and `KeyList` coexist on one key).
//! - `group = X` names a CB group — but only as a *direct body field* of a CB
//!   definition: `static_group_filter = { group = … }` nests a different
//!   `group` inside trigger blocks (vanilla's china wars), so the rule uses
//!   [`RefPattern::KeyValueTop`] (depth-1 only) plus the dir gate.
//!
//! All effects and triggers in a CB body are in CB scope unless the `.info`
//! says otherwise.
//!
//! Text fields are loc-key references. The named ones — the four war/CB name
//! keys and the four `on_*_desc` outcome fields — carry that themselves, via
//! the `scalar(LocKey)` in their own `FieldSpec` (see `FieldSpec::refs`); only
//! `desc` still needs a rule in `loc.rs`, because it also appears at depths no
//! body enumerates (dynamic-description leaves, script-value `modifier`
//! tooltips). Before this, a CB body produced *zero* loc references: the name
//! fields were already declared `scalar(LocKey)`, but a `ScalarKind` drove only
//! completion and hover, and nothing turned it into extraction.
//! Corpus: 1621 refs in vanilla, 3 unresolved across game + T4N
//! (`REMOVE_REGENT_WAR_NAME_BASE`, `REMOVE_REGENT_CB_NAME`,
//! `DEJURE_WAR_NAME_BASE` — present in no localization file in any language,
//! genuine dead-loc bugs the tool now surfaces).
//!
//! `cb_name` / `cb_name_no_target` / `on_invalidated_desc` were missing from the
//! body entirely; `on_invalidated_desc` left its whole subtree at `Unknown`, so
//! the triggers nested in it lost their clause context too. All four `on_*_desc`
//! fields take a bare key or a block — the corpus uses the block form for the
//! three peace outcomes and mostly the scalar form for `on_invalidated_desc`
//! (91 vs 30).
//!
//! `common/casus_belli_groups/` carries no text fields, so it needs no rules.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, DynamicDesc, Effect, ScriptValue, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{COST, DURATION, anywhere};
use super::culture_shared::in_innovations;

const TYPES_DIR: &str = "common/casus_belli_types/";
const GROUPS_DIR: &str = "common/casus_belli_groups/";

/// A `yes`/`no` toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// A numeric war-score tuning knob (defines-based when unset).
const fn knob(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc)
}

/// The body of one CB type definition (`_casus_belli.info`).
static CASUS_BELLI: StructSpec = StructSpec {
    name: "casus_belli",
    fields: &[
        (
            "group",
            scalar(Setting)
                .doc("The CB group this belongs to; the group can define extra restrictions."),
        ),
        (
            "icon",
            scalar(Setting).doc("The icon to use (defaults to the CB key)."),
        ),
        // War-score tuning (defines-based when unset).
        (
            "attacker_ticking_warscore_delay",
            block(ClauseKind::Struct(&DURATION))
                .doc("Delay before ticking war score starts increasing for the attacker."),
        ),
        (
            "defender_ticking_warscore_delay",
            block(ClauseKind::Struct(&DURATION))
                .doc("Delay before ticking war score starts increasing for the defender."),
        ),
        (
            "attacker_ticking_warscore",
            knob("How much ticking war score increases every day for the attacker."),
        ),
        (
            "defender_ticking_warscore",
            knob("How much ticking war score increases every day for the defender."),
        ),
        (
            "attacker_wargoal_percentage",
            knob(
                "How much of the wargoal the attacker must occupy to gain ticking war score \
                 (`0.0` = at least one occupation).",
            ),
        ),
        (
            "defender_wargoal_percentage",
            knob(
                "How much of the wargoal the defender must occupy to gain ticking war score \
                 (`0.0` = at least one occupation).",
            ),
        ),
        (
            "attacker_score_from_occupation_scale",
            knob("War score from occupation by the attacker is modified by this value."),
        ),
        (
            "defender_score_from_occupation_scale",
            knob("War score from occupation by the defender is modified by this value."),
        ),
        (
            "attacker_score_from_battles_scale",
            knob("War score from battles won by the attacker is modified by this value."),
        ),
        (
            "defender_score_from_battles_scale",
            knob("War score from battles won by the defender is modified by this value."),
        ),
        (
            "max_attacker_score_from_battles",
            knob("Total war score the attacker can gain from battles."),
        ),
        (
            "max_defender_score_from_battles",
            knob("Total war score the defender can gain from battles."),
        ),
        (
            "max_attacker_score_from_occupation",
            knob("Total war score the attacker can gain from occupation."),
        ),
        (
            "max_defender_score_from_occupation",
            knob("Total war score the defender can gain from occupation."),
        ),
        (
            "full_occupation_by_defender_gives_victory",
            toggle("Whether full occupation by the defender automatically gives 100% war score."),
        ),
        (
            "full_occupation_by_attacker_gives_victory",
            toggle("Whether full occupation by the attacker automatically gives 100% war score."),
        ),
        (
            "landless_attacker_needs_armies",
            toggle(
                "If `no`, being landless with no armies doesn't automatically give the other \
                 side 100% war score.",
            ),
        ),
        (
            "allow_hostages",
            toggle("Whether hostages can be used in peace negotiations (default `yes`)."),
        ),
        (
            "occupation_participation_mult",
            knob("Multiplier on occupation participation scoring (default 1)."),
        ),
        (
            "siege_participation_mult",
            knob("Multiplier on siege participation scoring (default 1)."),
        ),
        (
            "battle_participation_mult",
            knob("Multiplier on battle participation scoring (default 1)."),
        ),
        (
            "cost",
            block(ClauseKind::Struct(&COST)).doc(
                "Cost to declare the war. Add a `CB_BASE_COST` desc key to the value if you \
                 have no conditions.",
            ),
        ),
        (
            "attacker_capital_gives_war_score",
            toggle("Whether the attacker's capital gives war score."),
        ),
        (
            "defender_capital_gives_war_score",
            toggle("Whether the defender's capital gives war score."),
        ),
        (
            "imprisonment_by_attacker_give_war_score",
            toggle("Whether imprisonments by the attacker give war score."),
        ),
        (
            "imprisonment_by_defender_give_war_score",
            toggle("Whether imprisonments by the defender give war score."),
        ),
        // War lifecycle effects (CB scope).
        (
            "on_declaration",
            block(Effect).doc("Effect on declaration."),
        ),
        ("on_victory", block(Effect).doc("Effect on victory.")),
        (
            "on_white_peace",
            block(Effect).doc("Effect on white peace."),
        ),
        ("on_defeat", block(Effect).doc("Effect on defeat.")),
        (
            "on_invalidated",
            block(Effect).doc("Effect when the war is invalidated."),
        ),
        // Outcome descriptions. Each takes a bare loc key or a dynamic-description
        // block; the corpus uses the block form for the three peace outcomes and
        // overwhelmingly the scalar form for `on_invalidated_desc` (91 vs 30).
        (
            "on_victory_desc",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Description of the victory outcome (same scopes as the effect)."),
        ),
        (
            "on_defeat_desc",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Description of the defeat outcome (same scopes as the effect)."),
        ),
        (
            "on_white_peace_desc",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Description of the white-peace outcome (same scopes as the effect)."),
        ),
        (
            "on_invalidated_desc",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Description shown when the war is invalidated."),
        ),
        (
            "should_invalidate",
            block(Trigger).doc("When this passes, the war is invalidated."),
        ),
        (
            "mutually_exclusive_titles",
            block(Trigger).doc("If this evaluates to true, only one title can be targeted."),
        ),
        (
            "combine_into_one",
            toggle(
                "Show all instances of this CB (Holy War for X/Y/Z) as a single entry that \
                 lets you select between the targets.",
            ),
        ),
        // Availability triggers. Attacker/defender scopes per the `.info`.
        (
            "allowed_for_character",
            block(Trigger).doc(
                "`scope:attacker` and `scope:defender`; `root` is the attacker. \
                 `scope:defender` may not be valid depending on how the trigger is tested.",
            ),
        ),
        (
            "allowed_for_character_display_regardless",
            block(Trigger)
                .doc("Like `allowed_for_character`, but failing it still displays the CB."),
        ),
        (
            "allowed_against_character",
            block(Trigger).doc("`scope:attacker` and `scope:defender`; `root` is the defender."),
        ),
        (
            "allowed_against_character_display_regardless",
            block(Trigger)
                .doc("Like `allowed_against_character`, but failing it still displays the CB."),
        ),
        (
            "valid_to_start",
            block(Trigger).doc(
                "`scope:attacker`, `scope:defender`, and `scope:target` (if there's a target \
                 title); `root` is the attacker.",
            ),
        ),
        (
            "valid_to_start_display_regardless",
            block(Trigger).doc("Like `valid_to_start`, but failing it still displays the CB."),
        ),
        (
            "is_allowed_claim_title",
            block(Trigger).doc(
                "`scope:attacker`, `scope:defender`, and `scope:claimant`; `root` is the title.",
            ),
        ),
        // Targeting.
        (
            "target_titles",
            scalar(Setting)
                .doc(
                    "What titles this CB can be used against (`none` if it targets a realm \
                     rather than a title).",
                )
                .values(&[
                    "none",
                    "neighbor_land",
                    "neighbor_land_or_water",
                    "neighbor_land_tributary",
                    "neighbor_land_or_water_tributary",
                    "de_jure",
                    "claim",
                    "independence_domain",
                    "all",
                ]),
        ),
        (
            "target_title_tier",
            scalar(Setting)
                .doc("If set, the CB can only be used against this tier of title.")
                .values(&["barony", "county", "duchy", "kingdom", "empire", "all"]),
        ),
        (
            "target_de_jure_regions_above",
            toggle(
                "If set, the CB can target anyone who holds de jure land within a valid \
                 title, rather than just anyone who holds a valid title.",
            ),
        ),
        (
            "use_de_jure_wargoal_only",
            toggle(
                "If set, everything de jure under the target title counts as wargoal for \
                 ticking score; otherwise everything de facto under it that isn't de jure \
                 under another title the defender personally holds.",
            ),
        ),
        // Naming (loc keys with war-name substitutions).
        ("war_name", scalar(LocKey).doc("The war name.")),
        (
            "my_war_name",
            scalar(LocKey).doc("Used when the claimant and attacker is the same person."),
        ),
        ("war_name_base", scalar(LocKey).doc("The base war name.")),
        (
            "my_war_name_base",
            scalar(LocKey).doc("Base name used when the claimant and attacker is the same person."),
        ),
        (
            "cb_name",
            scalar(LocKey).doc("The CB's own name, as shown in the war-declaration interface."),
        ),
        (
            "cb_name_no_target",
            scalar(LocKey).doc("The CB name used when no target title is selected."),
        ),
        ("truce_days", knob("Days of truce after the war.")),
        (
            "ignore_effect",
            scalar(Setting).doc(
                "This kind of effect is skipped in the effects desc (repeatable; e.g. \
                 `ignore_effect = change_title_holder`).",
            ),
        ),
        // Death / inheritance behavior.
        (
            "on_primary_attacker_death",
            scalar(Setting)
                .doc("What happens to the war when the primary attacker dies.")
                .values(&["invalidate", "inherit", "inherit_faction"]),
        ),
        (
            "on_primary_defender_death",
            scalar(Setting)
                .doc("What happens to the war when the primary defender dies.")
                .values(&["invalidate", "inherit", "inherit_faction"]),
        ),
        (
            "transfer_behavior",
            scalar(Setting)
                .doc("What happens to the war when the target is transferred.")
                .values(&["invalidate", "transfer"]),
        ),
        (
            "check_attacker_inheritance_validity",
            toggle("If `no`, we don't check if the replacement is valid before doing it."),
        ),
        (
            "check_defender_inheritance_validity",
            toggle("If `no`, we don't check if the replacement is valid before doing it."),
        ),
        (
            "attacker_allies_inherit",
            toggle("Should allies in war inherit being in the war?"),
        ),
        (
            "defender_allies_inherit",
            toggle("Should allies in war inherit being in the war?"),
        ),
        (
            "interface_priority",
            knob(
                "Order in the CB list — higher shows up higher; ties broken by definition \
                 order.",
            ),
        ),
        // AI.
        (
            "max_ai_diplo_distance_to_title",
            knob("The AI never considers titles further away than this."),
        ),
        (
            "ai_only_against_liege",
            toggle("If set, the AI only checks this CB against its liege."),
        ),
        (
            "ai_only_against_neighbors",
            toggle("If set, the AI only checks this CB against its land and sea neighbors."),
        ),
        (
            "ai_can_target_all_titles",
            block(Trigger).doc(
                "Character-scope trigger: when it succeeds the AI uses the scripted title \
                 target, otherwise `neighbor_land_or_water`.",
            ),
        ),
        ("ai", toggle("If `no`, the AI ignores this CB entirely.")),
        (
            "ai_overlord_defensive_power_impact",
            block(ScriptValue).doc(
                "Overlord join chance, 0–1: the weight of the overlord's power when the AI \
                 evaluates attacking an unprotected subject. Scopes: `root`/`attacker` = the \
                 evaluating character, `defender` = the subject, `overlord` = the defender's \
                 overlord.",
            ),
        ),
        (
            "white_peace_possible",
            toggle("If `no`, only victory, defeat, or invalidation can end the war."),
        ),
        (
            "check_all_defenders_for_ticking_war_score",
            toggle("If `yes`, land held by all defenders within the wargoal is checked."),
        ),
        (
            "ticking_war_score_targets_entire_realm",
            toggle("If `yes`, the whole realm is checked instead of the wargoal."),
        ),
        (
            "gui_attacker_faith_might_join",
            toggle(
                "Show a warning that others of the attacker's faith might join (no gameplay \
                 effect).",
            ),
        ),
        (
            "gui_defender_faith_might_join",
            toggle(
                "Show a warning that others of the defender's faith might join (no gameplay \
                 effect).",
            ),
        ),
        (
            "defender_faith_can_join",
            toggle(
                "If set, same-faith defenders join when they fulfill the \
                 `can_defensively_join_holy_war` script rule with a positive join value.",
            ),
        ),
        ("is_great_holy_war", toggle("Is this a Great Holy War?")),
        (
            "target_top_liege_if_outside_realm",
            toggle(
                "Bypass the outside-realm top-liege-only targeting check. Only for scripted \
                 wars (Peasant Revolts); does not work in the UI.",
            ),
        ),
        (
            "should_check_for_interface_availability",
            toggle(
                "If `no`, this CB is skipped when checking CB availability (e.g. the \
                 `has_any_cb_on` trigger).",
            ),
        ),
        (
            "ai_score",
            scalar_or_block(Setting, ScriptValue)
                .doc("Script value, standard war scopes — added to the hard-coded title scoring."),
        ),
        (
            "ai_score_mult",
            scalar_or_block(Setting, ScriptValue)
                .doc("Script value, standard war scopes — multiplied with the title scoring."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one CB group — shared restrictions layered onto every CB in
/// the group (fields observed in `common/casus_belli_groups/`).
static CASUS_BELLI_GROUP: StructSpec = StructSpec {
    name: "casus_belli_group",
    fields: &[
        (
            "allowed_for_character",
            block(Trigger)
                .doc("Extra restriction on every CB in this group (attacker character scope)."),
        ),
        (
            "can_only_start_via_script",
            toggle("CBs in this group can only be started from script, not the UI."),
        ),
        (
            "should_check_for_interface_availability",
            toggle("Whether CBs in this group count for CB-availability checks."),
        ),
        ("debug", toggle("Marks the group as debug-only.")),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct CasusBelli;

impl Entity for CasusBelli {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::CASUS_BELLI,
            icon: IconHint::Action,
            defs: Some(DefSource {
                dir_prefix: TYPES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                anywhere(RefPattern::KeyValue("casus_belli")),
                anywhere(RefPattern::KeyValue("cb")),
                anywhere(RefPattern::KeyValue("using_cb")),
                // `ai_start_best_war = { cb = { X Y } }` — same key, list form.
                anywhere(RefPattern::KeyList("cb")),
                // An innovation's tooltip-only CB unlock (corpus-validated:
                // the key never occurs in eras/ or anywhere else).
                in_innovations(RefPattern::KeyValue("unlock_casus_belli")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::CASUS_BELLI_GROUP,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: GROUPS_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[RefRule {
                pattern: RefPattern::KeyValueTop("group"),
                gate: Some(TYPES_DIR),
                alt: &[],
            }],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (TYPES_DIR, ClauseKind::Struct(&CASUS_BELLI)),
        (GROUPS_DIR, ClauseKind::Struct(&CASUS_BELLI_GROUP)),
    ];
}
