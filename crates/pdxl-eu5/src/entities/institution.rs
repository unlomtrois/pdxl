//! Institutions (`in_game/common/institution/`), modeled from `readme.txt`
//! plus the two corpus fields omitted there (`spread_from_any_export` and
//! `spread_from_was_possible_spawn`). Definitions are top-level blocks.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, ScriptValue, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block_scoped, scalar_or_block};
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern, RefRule,
};

use super::Entity;

pub(crate) const INSTITUTIONS_DIR: &str = "in_game/common/institution/";

static INSTITUTION: StructSpec = StructSpec {
    name: "institution",
    fields: &[
        (
            "age",
            pdxl_analysis::context::scalar(Setting).doc("Age in which the institution belongs."),
        ),
        (
            "can_spawn",
            block_scoped(Trigger, "location")
                .doc("Whether the institution can spawn at this location."),
        ),
        (
            "promote_chance",
            scalar_or_block(Setting, ScriptValue)
                .doc("Script value controlling institution promotion chance."),
        ),
        (
            "spread_from_friendly_coast_border_location",
            scalar_or_block(Setting, ScriptValue)
                .doc("Spread from a friendly coastal neighboring location."),
        ),
        (
            "spread_from_any_coast_border_location",
            scalar_or_block(Setting, ScriptValue)
                .doc("Spread from any coastal neighboring location."),
        ),
        (
            "spread_from_any_import",
            scalar_or_block(Setting, ScriptValue).doc("Spread through imported goods."),
        ),
        (
            "spread_from_any_export",
            scalar_or_block(Setting, ScriptValue).doc("Spread through exported goods. *(corpus)*"),
        ),
        (
            "spread_scale_on_control_if_owner_embraced",
            scalar_or_block(Setting, ScriptValue)
                .doc("Control-based spread scale when the owner has embraced it."),
        ),
        (
            "spread_embraced_to_capital",
            scalar_or_block(Setting, ScriptValue)
                .doc("Spread from an embracing owner to its capital."),
        ),
        (
            "spread_to_market_member",
            scalar_or_block(Setting, ScriptValue).doc("Spread to another member of the market."),
        ),
        (
            "spread_to_market_center",
            scalar_or_block(Setting, ScriptValue).doc("Spread to the market center."),
        ),
        (
            "spread_from_was_possible_spawn",
            scalar_or_block(Setting, ScriptValue)
                .doc("Spread from a location where spawning was previously possible. *(corpus)*"),
        ),
        (
            "spread",
            scalar_or_block(Setting, ScriptValue).doc("General institution spread speed."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Institution;

impl Entity for Institution {
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::INSTITUTION,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::INSTITUTION,
            suffix: "_desc",
        },
    ];

    const LOC_DATAFN_ARG_REFS: &'static [(&'static str, pdxl_analysis::KindId)] = &[
        ("GetInstitutionByKey", kinds::INSTITUTION),
        ("ShowInstitutionName", kinds::INSTITUTION),
        ("ShowInstitutionNameWithNoTooltip", kinds::INSTITUTION),
    ];

    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::INSTITUTION,
        icon: IconHint::Hierarchy,
        defs: Some(DefSource {
            dir_prefix: INSTITUTIONS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            RefRule {
                pattern: RefPattern::KeyValue("institution"),
                gate: None,
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyBlockKeys("institutions"),
                gate: Some(super::setup_manager::START_SETUP_DIR),
                alt: &[],
            },
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(INSTITUTIONS_DIR, ClauseKind::Struct(&INSTITUTION))];
}
