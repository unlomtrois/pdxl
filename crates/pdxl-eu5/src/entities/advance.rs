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
use pdxl_analysis::context::ClauseKind::{self, ScriptedModifier, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::scripted::def_only;

pub(crate) const ADVANCES_DIR: &str = "in_game/common/advances/";
const AGE_DIR: &str = "in_game/common/age/";

/// A `key = X` reference gated to the advances directory.
const fn in_advances(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(ADVANCES_DIR),
        alt: &[],
    }
}

/// The body of one advance (inline doc header + corpus).
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
            block(Trigger).doc(
                "Whether the advance appears at all (e.g. `has_or_had_tag = TAG` for \
                 national trees).",
            ),
        ),
        (
            "allow",
            block(Trigger).doc("Whether the advance can be taken once visible."),
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
            refs: &[
                in_advances("age"),
                RefRule {
                    pattern: RefPattern::ScopePrefix("age"),
                    gate: None,
                    alt: &[],
                },
            ],
            ..def_only(kinds::AGE, IconHint::Hierarchy, AGE_DIR)
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(ADVANCES_DIR, ClauseKind::Struct(&ADVANCE))];
}
