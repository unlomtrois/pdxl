//! Situations (`in_game/common/situations/`), modeled from the directory
//! readme plus corpus-only map legend and content-filter fields.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Effect, ScriptValue, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar, scalar_or_block};
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern, RefRule,
};

use super::Entity;

pub(crate) const SITUATIONS_DIR: &str = "in_game/common/situations/";

static LEGEND_KEY: StructSpec = StructSpec {
    name: "situation map legend key",
    fields: &[
        (
            "desc",
            scalar(Setting).doc("Localization shown for this legend entry."),
        ),
        ("color", scalar_or_block(Setting, ClauseKind::Config)),
        (
            "require_color_on_map",
            scalar(Setting).values(&["yes", "no"]),
        ),
    ],
    fallback: Fallback::Deny,
};

static SITUATION: StructSpec = StructSpec {
    name: "situation",
    fields: &[
        (
            "custom_description",
            scalar(Setting).doc("Customizable-localization function used as the description."),
        ),
        (
            "monthly_spawn_chance",
            scalar_or_block(Setting, ScriptValue)
                .doc("Monthly spawn probability, evaluated with root = situation."),
        ),
        ("international_organization_type", scalar(Setting)),
        ("resolution", scalar(Setting)),
        (
            "voters",
            scalar(Setting).doc("Global-list tag containing eligible voters."),
        ),
        ("can_start", block_scoped(Trigger, "situation")),
        ("can_end", block_scoped(Trigger, "situation")),
        (
            "visible",
            block_scoped(Trigger, "country").doc("Player visibility; `target` is the situation."),
        ),
        ("on_start", block_scoped(Effect, "situation")),
        ("on_monthly", block_scoped(Effect, "situation")),
        ("on_ending", block_scoped(Effect, "situation")),
        ("on_ended", block_scoped(Effect, "situation")),
        (
            "tooltip",
            block_scoped(Effect, "location")
                .doc("Map tooltip generation; `target` is the situation."),
        ),
        (
            "map_color",
            scalar_or_block(Setting, ScriptValue)
                .doc("Script color evaluated with root = location and target = situation."),
        ),
        (
            "secondary_map_color",
            scalar_or_block(Setting, ScriptValue)
                .doc("Striped script color evaluated with root = location and target = situation."),
        ),
        (
            "hint_tag",
            scalar(Setting).doc("Localization key for the situation hint."),
        ),
        (
            "content_trigger",
            block_scoped(Trigger, "country").doc("Content-availability trigger. *(corpus)*"),
        ),
        (
            "is_data_map",
            scalar(Setting)
                .values(&["yes", "no"])
                .doc("Whether this situation uses a data map. *(corpus)*"),
        ),
        (
            "legend_key",
            block(ClauseKind::Struct(&LEGEND_KEY)).doc("Repeated map legend entry. *(corpus)*"),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Situation;

impl Entity for Situation {
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::SITUATION,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::SITUATION,
            suffix: "_desc",
        },
    ];

    const LOC_DATAFN_ARG_REFS: &'static [(&'static str, pdxl_analysis::KindId)] = &[
        ("GetSituationByKey", kinds::SITUATION),
        ("ShowSituationName", kinds::SITUATION),
        ("ShowSituationNameWithNoTooltip", kinds::SITUATION),
    ];

    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::SITUATION,
        icon: IconHint::Event,
        defs: Some(DefSource {
            dir_prefix: SITUATIONS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[RefRule {
            pattern: RefPattern::KeyValue("situation"),
            gate: None,
            alt: &[],
        }],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(SITUATIONS_DIR, ClauseKind::Struct(&SITUATION))];
}
