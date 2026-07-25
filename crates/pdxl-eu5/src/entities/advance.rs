//! Advances (`in_game/common/advances/`, documented by the inline header in
//! `0_age_of_traditions.txt`) — the tech tree: 3,159 top-level defs across
//! 215 files, organized by age.
//!
//! References (corpus-validated, 0 unresolved):
//! - `requires = X` — an advance's prerequisite advance (2,768 refs, gated);
//! - `has_advance = X` — the trigger, script-wide (486 refs);
//! - `age = X` inside advances (3,179) and `age:X` literals anywhere (20)
//!   resolve to the six ages (`in_game/common/age/`).
//!
//! The `unlock_building` / `unlock_unit` / `unlock_law` references live in
//! [`super::unlocks`]. Loose body keys are modifier tags
//! (`global_life_expectancy = 4`) — [`Fallback::Modifier`].

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, ScriptedModifier, StaticModifier, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::scripted::def_only;

pub(crate) const ADVANCES_DIR: &str = "in_game/common/advances/";
pub(crate) const AGE_DIR: &str = "in_game/common/age/";

/// `max/min_ai_privilege_per_estate = { <estate> = N … }` — the keys are
/// estate references (rules in [`super::estate`]); values are AI caps.
static PRIVILEGE_CAPS: StructSpec = StructSpec {
    name: "ai privilege caps",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// The body of one age (`00_default.txt`; six defs drive the whole game
/// pacing, so the corpus is the documentation).
static AGE: StructSpec = StructSpec {
    name: "age",
    fields: &[
        (
            "year",
            scalar(Setting).doc("The year the age begins (age 1 starts at game start)."),
        ),
        (
            "price_stability",
            scalar(Setting).doc("Market price stability during this age."),
        ),
        ("max_price", scalar(Setting).doc("Market price cap.")),
        (
            "known_goods_demand_threshold",
            scalar(Setting).doc("Awareness threshold before goods generate demand."),
        ),
        (
            "burgher_max_trade_range",
            scalar(Setting).doc("Burgher trade range during this age."),
        ),
        (
            "months_for_exploration_spread",
            scalar(Setting).doc("How fast map knowledge spreads (months)."),
        ),
        (
            "hegemons_allowed",
            scalar(Setting)
                .doc("Whether hegemonies can exist in this age.")
                .values(&["yes", "no"]),
        ),
        (
            "efficiency",
            scalar(Setting).doc("Age efficiency factor (declines over the ages)."),
        ),
        (
            "victory_card",
            scalar(Setting).doc("The victory-card slot unlocked by this age."),
        ),
        (
            "mercenaries",
            scalar(Setting).doc("Mercenary availability modifier."),
        ),
        (
            "war_score_from_battles",
            scalar(Setting).doc("War-score-from-battles modifier."),
        ),
        (
            "unique",
            block(StaticModifier).doc("Modifiers applied only while this age is active."),
        ),
        (
            "modifier",
            block(StaticModifier).doc("Modifiers applied from this age onward (they accumulate)."),
        ),
        (
            "goods_demand",
            block(StaticModifier)
                .doc("Per-good pop-demand modifiers (`global_<good>_pop_demand`)."),
        ),
        (
            "max_ai_privilege_per_estate",
            block(Struct(&PRIVILEGE_CAPS))
                .doc("AI cap on granted privileges per estate (`<estate> = N`)."),
        ),
        (
            "min_ai_privilege_per_estate",
            block(Struct(&PRIVILEGE_CAPS))
                .doc("AI floor on granted privileges per estate (`<estate> = N`)."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// A `key = X` reference gated to the advances directory.
const fn in_advances(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(ADVANCES_DIR),
        alt: &[],
    }
}

/// The body of one advance (`readme.txt` + corpus).
static ADVANCE: StructSpec = StructSpec {
    name: "advance",
    fields: &[
        (
            "age",
            scalar(Setting).doc("The age this advance belongs to (`in_game/common/age/`)."),
        ),
        (
            "requires",
            scalar(Setting).doc("A prerequisite advance (repeatable)."),
        ),
        (
            "depth",
            scalar(Setting).doc("Tree depth within the age (0 = a root of the tree)."),
        ),
        ("icon", scalar(Setting).doc("Icon override.")),
        (
            "content_priority",
            scalar(Setting).doc("Ordering priority within the tree UI."),
        ),
        (
            "potential",
            block_scoped(Trigger, "country").doc(
                "Whether the advance appears at all (e.g. `has_or_had_tag = TAG` for \
                 national trees).",
            ),
        ),
        (
            "allow",
            block_scoped(Trigger, "country").doc("Whether the advance can be taken once visible."),
        ),
        (
            "for",
            scalar(Setting)
                .doc(
                    "Only for a specialization your country can take at the start of \
                     an age.",
                )
                .values(&["adm", "dip", "mil"]),
        ),
        (
            "government",
            scalar(Setting).doc("Only available with this government type."),
        ),
        (
            "unlock_building",
            scalar(Setting).doc("Unlocks a building (`in_game/common/building_types/`)."),
        ),
        (
            "unlock_unit",
            scalar(Setting).doc("Unlocks a unit (`in_game/common/unit_types/`)."),
        ),
        (
            "unlock_law",
            scalar(Setting).doc("Unlocks a law (`in_game/common/laws/`)."),
        ),
        (
            "unlock_subject_type",
            scalar(Setting).doc("Unlocks a subject type (`in_game/common/subject_types/`)."),
        ),
        (
            "unlock_ability",
            scalar(Setting).doc("Unlocks a unit ability (`in_game/common/unit_abilities/`)."),
        ),
        (
            "unlock_interaction",
            scalar(Setting)
                .doc("Unlocks a character interaction (`in_game/common/character_interactions/`)."),
        ),
        (
            "unlock_country_interaction",
            scalar(Setting)
                .doc("Unlocks a country interaction (`in_game/common/country_interactions/`)."),
        ),
        (
            "unlock_relation_type",
            scalar(Setting).doc("Unlocks a relation type (`in_game/common/scripted_relations/`)."),
        ),
        (
            "unlock_levy",
            scalar(Setting).doc("Unlocks a levy (`in_game/common/levies/`)."),
        ),
        (
            "unlock_government_reform",
            scalar(Setting)
                .doc("Unlocks a government reform (`in_game/common/government_reforms/`)."),
        ),
        (
            "unlock_casus_belli",
            scalar(Setting).doc("Unlocks a casus belli (`in_game/common/casus_belli/`)."),
        ),
        (
            "unlock_production_method",
            scalar(Setting)
                .doc("Unlocks a production method (own dir, or inline in a building body)."),
        ),
        (
            "country_type",
            scalar(Setting)
                .doc("Only available to this country type.")
                .values(&["location", "pop", "building", "army"]),
        ),
        (
            "allow_children",
            scalar(Setting)
                .doc("Force the advance to be a child node (error log if violated).")
                .values(&["yes", "no"]),
        ),
        (
            "modifier_while_progressing",
            block(Struct(&super::common::SCALED_MODIFIER)).doc(
                "Triggered, scaled modifier applied to the country while this advance is \
                 being researched.",
            ),
        ),
        (
            "ai_weight",
            block(ScriptedModifier).doc("AI pick priority (base + weighted modifiers)."),
        ),
    ],
    // Every other key is a modifier tag applied to the country.
    fallback: Fallback::Modifier,
};

pub(crate) struct Advance;

impl Entity for Advance {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::ADVANCE,
            icon: IconHint::Function,
            defs: Some(DefSource {
                dir_prefix: ADVANCES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                in_advances("requires"),
                RefRule {
                    pattern: RefPattern::KeyValue("has_advance"),
                    gate: None,
                    alt: &[],
                },
            ],
            aliases: &[],
        },
        KindSpec {
            // The `age:` literal is table-derived (`crate::derived`).
            refs: &[
                in_advances("age"),
                // Government reforms name their availability age (161 refs,
                // 0 unresolved).
                RefRule {
                    pattern: RefPattern::KeyValue("age"),
                    gate: Some(super::government_reform::REFORMS_DIR),
                    alt: &[],
                },
            ],
            ..def_only(kinds::AGE, IconHint::Hierarchy, AGE_DIR)
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (ADVANCES_DIR, ClauseKind::Struct(&ADVANCE)),
        (AGE_DIR, ClauseKind::Struct(&AGE)),
    ];
}
