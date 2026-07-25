//! Parliament types (`in_game/common/parliament_types/`) are shared database
//! objects used by countries and international organizations. Vanilla keeps
//! both forms in this directory; the definition's `type` field selects the
//! root scope of its trigger blocks.
//!
//! References are bare `parliament_type = X` comparisons/selections,
//! `set_parliament_type = X`, and table-derived `parliament_type:X` literals.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, StaticModifier, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

pub(crate) const PARLIAMENT_TYPES_DIR: &str = "in_game/common/parliament_types/";

/// The body of one parliament type, complete against the directory readme.
static PARLIAMENT_TYPE: StructSpec = StructSpec {
    name: "parliament type",
    fields: &[
        (
            "type",
            scalar(Setting)
                .doc("Whose parliament this is (decides the trigger root).")
                .values(&["country", "international_organization"]),
        ),
        (
            "potential",
            block(Trigger).doc("Is this parliament type visible at all (root follows `type`)."),
        ),
        (
            "allow",
            block(Trigger).doc("Can this parliament type be used (root follows `type`)."),
        ),
        (
            "locked",
            block(Trigger).doc("Is this parliament type locked (root follows `type`)."),
        ),
        (
            "modifier",
            block(StaticModifier).doc("Country modifiers applied by this parliament type."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct ParliamentType;

impl Entity for ParliamentType {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::PARLIAMENT_TYPE,
        icon: IconHint::Hierarchy,
        defs: Some(DefSource {
            dir_prefix: PARLIAMENT_TYPES_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            RefRule {
                pattern: RefPattern::KeyValue("parliament_type"),
                gate: None,
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("set_parliament_type"),
                gate: None,
                alt: &[],
            },
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(PARLIAMENT_TYPES_DIR, Struct(&PARLIAMENT_TYPE))];
}
