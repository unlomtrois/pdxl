//! Great projects (`common/great_projects/types/`, from
//! `_great_project_types.info`) — the multi-stage constructions rulers fund
//! together — plus the contributions nested inside each one.
//!
//! Two kinds, because a contribution is a named thing in its own right: 20
//! project types in vanilla hold 139 distinct contribution keys, and the AI,
//! the cost model and the completion effects all hang off the contribution
//! rather than the project. They are `ScopedChildrenOf` rather than plain
//! defs — the same key recurs under different projects, so a repeat must
//! gap-fill instead of reporting a duplicate.
//!
//! Cross-references:
//! - `great_project_type:X` resolves through the table-derived scope-link rule
//!   (see `derived.rs`) once this kind exists — 14 distinct uses, all resolving.
//! - `invite_interaction = X` names the character interaction used to ask for
//!   contributions; the info notes it must carry the
//!   `request_great_project_contribution` special-interaction type.
//! - `government_type = { … }` inside `ai_target_quick_trigger` lists
//!   governments, gated to this directory (the bare key means other things
//!   elsewhere).
//!
//! Implicit localization: `great_project_type_<key>` is universal (20/20).
//! The info documents three more — `great_project_type_tooltip_<key>`, and
//! `great_project_name_<key>` / `_possessive_<key>` for an in-progress project
//! — which no file uses yet; they are listed anyway, since an unmatched
//! pattern is skipped and a modder following the docs should get the link.
//!
//! Deliberate omission: a contribution's own keys are composed from *both*
//! names (`great_project_type_<project>_contribution_<key>`).
//! [`ImplicitLocPattern`] suffixes one entity name, so it cannot express a
//! pair; those links would need a bespoke rule.
//!
//! Where the info and the corpus disagree the corpus wins: the info's example
//! writes `group = major`, but the only values in use are the long forms its
//! own "possible values" list gives (`environmental_project`).

use pdxl_analysis::context::ClauseKind::{self, DynamicDesc, Effect, ScriptValue, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern, RefRule,
};

use crate::kinds;

use super::Entity;
use super::common::{COST, OPAQUE, TRIGGERED_ASSET};

const TYPES_DIR: &str = "common/great_projects/types/";

/// `allowed_contributor_filter = { vassals = yes owner = yes }` — which rulers
/// may invest. A bitmask written as toggles.
static CONTRIBUTOR_FILTER: StructSpec = StructSpec {
    name: "allowed_contributor_filter",
    fields: &[
        ("vassals", scalar(Setting).values(&["yes", "no"])),
        ("tributaries", scalar(Setting).values(&["yes", "no"])),
        ("liege", scalar(Setting).values(&["yes", "no"])),
        ("top_liege", scalar(Setting).values(&["yes", "no"])),
        ("owner", scalar(Setting).values(&["yes", "no"])),
        (
            "allies",
            scalar(Setting)
                .doc(
                    "Allies of the owner. Also grantable from a house aspiration via \
                     `can_request_great_project_contributions_from_allies`.",
                )
                .values(&["yes", "no"]),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `ai_check_interval_by_tier = { barony = 0 … }`. Every tier is required.
static CHECK_INTERVAL_BY_TIER: StructSpec = StructSpec {
    name: "ai_check_interval_by_tier",
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

/// `ai_target_quick_trigger = { adult = yes rank = … government_type = { … } }`
/// — cheap engine prefilters on the prospective founder.
static AI_TARGET_QUICK_TRIGGER: StructSpec = StructSpec {
    name: "ai_target_quick_trigger",
    fields: &[
        (
            "adult",
            scalar(Setting)
                .doc("The founder must be an adult.")
                .values(&["yes", "no"]),
        ),
        (
            "rank",
            scalar(Setting)
                .doc("Minimum primary-title tier. Assumes `hegemony` when unset.")
                .values(&["barony", "county", "duchy", "kingdom", "empire", "hegemony"]),
        ),
        (
            "government_type",
            block(Struct(&OPAQUE))
                .doc("Government keys the founder may hold; omit to skip the check."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// One `project_contributions = { <key> = { … } }` entry.
static CONTRIBUTION: StructSpec = StructSpec {
    name: "great_project_contribution",
    fields: &[
        (
            "is_shown",
            block(Trigger).doc("Can the contributor see this contribution at all?"),
        ),
        (
            "show_in_planning_phase",
            scalar(Setting)
                .doc("Show during planning. Only optional contributions may hide. Default `yes`.")
                .values(&["yes", "no"]),
        ),
        (
            "contributor_is_valid",
            block(Trigger).doc(
                "May the scoped character contribute? Printed unevaluated, so \
                 `trigger_if` needs a custom desc.",
            ),
        ),
        (
            "context_allows_contributions",
            block(Trigger)
                .doc("Contextual gate — timing or phase, not the contributor's own state."),
        ),
        (
            "allowed_contributor_filter",
            block(Struct(&CONTRIBUTOR_FILTER))
                .doc("Overrides the project's filter; empty means inherit it."),
        ),
        ("cost", block(Struct(&COST))),
        (
            "contributor_cooldown",
            scalar_or_block(Setting, ScriptValue)
                .doc("Days before contributing again to the same project."),
        ),
        (
            "is_required",
            scalar(Setting)
                .doc("Required to finish the project, or optional. Default `yes`.")
                .values(&["yes", "no"]),
        ),
        (
            "on_contribution_funded",
            block(Effect).doc("Fires when this contribution is funded."),
        ),
        (
            "on_complete",
            block(Effect).doc("Fires when the parent project completes."),
        ),
        (
            "ai_will_do",
            block(ScriptValue).doc("Weight for the AI's weighted-random pick."),
        ),
        (
            "ai_check_interval",
            scalar(Setting).doc("Months between AI considerations. Default 12."),
        ),
        (
            "ai_check_interval_by_tier",
            block(Struct(&CHECK_INTERVAL_BY_TIER))
                .doc("Months per tier, used instead of `ai_check_interval`; `0` never."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `project_contributions = { <key> = { … } }`.
static PROJECT_CONTRIBUTIONS: StructSpec = StructSpec {
    name: "project_contributions",
    fields: &[],
    fallback: Fallback::Struct(&CONTRIBUTION),
};

/// The body of one great-project type.
static GREAT_PROJECT: StructSpec = StructSpec {
    name: "great_project_type",
    fields: &[
        (
            "icon",
            scalar(Setting)
                .doc("Image under `GREAT_PROJECT_ICON_PATH`; defaults to the project key."),
        ),
        (
            "illustration",
            block(Struct(&TRIGGERED_ASSET)).doc(
                "Triggered illustration list; the first passing entry wins, so end \
                 with an untriggered fallback.",
            ),
        ),
        (
            "name",
            block(DynamicDesc)
                .doc("Triggered name list, root is the planning character; end with a fallback."),
        ),
        (
            "is_shown",
            block(Trigger).doc("Can the character see this project in the interface?"),
        ),
        (
            "can_start_planning",
            block(Trigger).doc("May the character begin planning it?"),
        ),
        (
            "can_cancel",
            block(Trigger).doc("May the character cancel it? Non-owners are refused regardless."),
        ),
        (
            "is_location_valid",
            block(Trigger).doc("Province-specific checks for the chosen location."),
        ),
        (
            "is_valid",
            block(Trigger).doc("Still valid? Failing fires `on_invalidated`, then `on_cancel`."),
        ),
        (
            "province_filter",
            scalar(Setting).doc(
                "Broad province selection, as in activities; narrowed by \
                      `can_start_planning`.",
            ),
        ),
        (
            "province_filter_target",
            scalar(Setting).doc(
                "The target for a filter that needs one (`landed_title`, \
                      `geographical_region`).",
            ),
        ),
        (
            "ai_province_filter",
            scalar(Setting).doc("Province selection for the AI; defaults to `province_filter`."),
        ),
        (
            "owner",
            scalar(Setting)
                .doc("Who owns the finished project. Default `province_owner`.")
                .values(&[
                    "province_owner_top_liege",
                    "province_owner",
                    "founder_primary_title_owner",
                    "founder_top_liege_title_owner",
                ]),
        ),
        ("cost", block(Struct(&COST))),
        (
            "construction_time",
            scalar_or_block(Setting, ScriptValue).doc("Days to completion."),
        ),
        (
            "contribution_threshold",
            scalar(Setting).doc(
                "Progress percentage after which optional contributions can no longer \
                      be funded.",
            ),
        ),
        (
            "contributor_cooldown",
            scalar_or_block(Setting, ScriptValue)
                .doc("Days before others may fund contributions — a head start for the founder."),
        ),
        (
            "allowed_contributor_filter",
            block(Struct(&CONTRIBUTOR_FILTER))
                .doc("Which rulers may invest. Default: vassals and the owner."),
        ),
        (
            "project_contributions",
            block(Struct(&PROJECT_CONTRIBUTIONS))
                .doc("The fundable contributions; at least one is required."),
        ),
        (
            "invite_interaction",
            scalar(Setting).doc(
                "Interaction used to request contributions. Must carry the \
                 `request_great_project_contribution` special-interaction type. \
                 Default `request_great_project_contribution_interaction`.",
            ),
        ),
        (
            "on_complete",
            block(Effect).doc("Fires when the project completes."),
        ),
        (
            "on_cancel",
            block(Effect)
                .doc("Fires when destroyed, or when planning or construction is cancelled."),
        ),
        (
            "on_plan_build",
            block(Effect).doc("Fires when planned — the project is funded, contributions are not."),
        ),
        (
            "on_start_build",
            block(Effect)
                .doc("Fires when construction starts — every mandatory contribution is funded."),
        ),
        (
            "on_invalidated",
            block(Effect).doc("Fires when it becomes invalid, just before removal."),
        ),
        (
            "ai_will_do",
            block(ScriptValue).doc("Weight for the AI's weighted-random pick."),
        ),
        (
            "ai_check_interval",
            scalar(Setting).doc("Years between AI considerations. Default 10."),
        ),
        (
            "ai_check_interval_by_tier",
            block(Struct(&CHECK_INTERVAL_BY_TIER))
                .doc("Years per tier, used instead of `ai_check_interval`; `0` never."),
        ),
        (
            "ai_target_quick_trigger",
            block(Struct(&AI_TARGET_QUICK_TRIGGER))
                .doc("Cheap founder prefilter applied before the scripted triggers."),
        ),
        (
            "show_in_list",
            scalar(Setting)
                .doc("Show in the Great Projects list. Default `yes`.")
                .values(&["yes", "no"]),
        ),
        (
            "is_important",
            scalar(Setting)
                .doc("Warrant special notifications. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "target_title_tier",
            scalar(Setting)
                .doc("Tier highlighted on the map while planning. Default `barony`.")
                .values(&["barony", "county", "duchy", "kingdom", "empire"]),
        ),
        (
            "group",
            scalar(Setting)
                .doc(
                    "Interface behaviour. `environmental_project` hides the owner portrait; \
                     major and minor are not otherwise distinguished.",
                )
                .values(&["major_project", "minor_project", "environmental_project"]),
        ),
        (
            "completion_sound_effect",
            scalar(Setting).doc("Sound played on completion. Empty by default."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct GreatProject;

impl Entity for GreatProject {
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::GREAT_PROJECT_TYPE,
            suffix: "great_project_type_{}",
        },
        ImplicitLocPattern {
            kind: kinds::GREAT_PROJECT_TYPE,
            suffix: "great_project_type_tooltip_{}",
        },
        ImplicitLocPattern {
            kind: kinds::GREAT_PROJECT_TYPE,
            suffix: "great_project_name_{}",
        },
        ImplicitLocPattern {
            kind: kinds::GREAT_PROJECT_TYPE,
            suffix: "great_project_name_possessive_{}",
        },
    ];

    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::GREAT_PROJECT_TYPE,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: TYPES_DIR,
                shape: DefShape::TopLevel,
            }),
            // `great_project_type:X` comes from the derived scope-link rule.
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::GREAT_PROJECT_CONTRIBUTION,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: TYPES_DIR,
                // Scoped: the same contribution key recurs under different
                // projects, so a repeat gap-fills rather than duplicating.
                shape: DefShape::ScopedChildrenOf {
                    containers: &["project_contributions"],
                },
            }),
            refs: &[],
            aliases: &[],
        },
        // The interaction that asks for contributions, gated because
        // `invite_interaction` means nothing outside this directory.
        KindSpec {
            kind: kinds::CHARACTER_INTERACTION,
            icon: IconHint::Action,
            defs: None,
            refs: &[RefRule {
                pattern: RefPattern::KeyValue("invite_interaction"),
                gate: Some(TYPES_DIR),
                alt: &[],
            }],
            aliases: &[],
        },
        // `government_type = { mandala_government }` inside
        // `ai_target_quick_trigger` — a bare word list, so `KeyList` rather
        // than `KeyBlockKeys`, which would look for `key = value` pairs.
        KindSpec {
            kind: kinds::GOVERNMENT,
            icon: IconHint::Hierarchy,
            defs: None,
            refs: &[RefRule {
                pattern: RefPattern::KeyList("government_type"),
                gate: Some(TYPES_DIR),
                alt: &[],
            }],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(TYPES_DIR, Struct(&GREAT_PROJECT))];
}
