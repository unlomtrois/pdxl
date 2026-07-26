//! Named colors (`main_menu/common/named_colors/`) — the block children of
//! `colors = { }` containers (4,101 in vanilla; same shape as CK3),
//! referenced by the scalar forms of `color` / `color2` / `unit_color0–2`
//! in countries, cultures, and religions (4,504 refs, 0 unresolved, gated
//! to those dirs). Also declares the country-description-category kind
//! (3 defs, 67 `description_category` refs, 0 unresolved).

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::context::{Fallback, StructSpec};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::country::COUNTRIES_DIR;
use super::culture::CULTURES_DIR;
use super::religion::RELIGIONS_DIR;
use super::scripted::def_only;

const NAMED_COLORS_DIR: &str = "main_menu/common/named_colors/";

/// A gated named-color reference.
const fn color_in(dir: &'static str, key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(dir),
        alt: &[],
    }
}

/// The `colors = { … }` container: block-valued children are the colors.
static NAMED_COLORS: StructSpec = StructSpec {
    name: "named colors",
    fields: &[],
    fallback: Fallback::Color,
};

pub(crate) struct NamedColor;

impl Entity for NamedColor {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::NAMED_COLOR,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: NAMED_COLORS_DIR,
                shape: DefShape::ChildrenOf {
                    containers: &["colors"],
                },
            }),
            refs: &[
                color_in(COUNTRIES_DIR, "color"),
                color_in(COUNTRIES_DIR, "color2"),
                color_in(COUNTRIES_DIR, "unit_color0"),
                color_in(COUNTRIES_DIR, "unit_color1"),
                color_in(COUNTRIES_DIR, "unit_color2"),
                color_in(CULTURES_DIR, "color"),
                color_in(super::culture::CULTURE_GROUPS_DIR, "color"),
                color_in(super::culture::LANGUAGES_DIR, "color"),
                color_in(super::culture::LANGUAGE_FAMILIES_DIR, "color"),
                color_in(RELIGIONS_DIR, "color"),
                color_in(super::estate::ESTATES_DIR, "color"),
                color_in(super::subject_type::SUBJECT_TYPES_DIR, "color"),
                // Situation legend entries use `color = named_color`.
                color_in(super::situation::SITUATIONS_DIR, "color"),
            ],
            aliases: &[],
        },
        KindSpec {
            refs: &[RefRule {
                pattern: RefPattern::KeyValue("description_category"),
                gate: None,
                alt: &[],
            }],
            ..def_only(
                kinds::COUNTRY_DESCRIPTION_CATEGORY,
                IconHint::Tag,
                "in_game/common/country_description_categories/",
            )
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(NAMED_COLORS_DIR, ClauseKind::Struct(&NAMED_COLORS))];
}
