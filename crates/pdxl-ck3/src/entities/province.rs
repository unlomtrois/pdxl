//! Map provinces. Unlike every other kind, provinces are **not defined in
//! PDXScript**: the id table is `map_data/definition.csv` (`id;r;g;b;name;x;`,
//! one row per province — [`DefShape::IdCsv`], read by the project layer's CSV
//! reader behind the FileSet's `set_include_map_data` opt-in).
//!
//! References (all corpus-validated at 0 unresolved):
//! - `history/provinces/` — the top-level `8289 = { … }` keys reference
//!   province ids ([`RefPattern::TopLevelBlockKeys`]); the files declare no
//!   new entities.
//! - `common/landed_titles/` — a barony's `province = 1337` capital id.
//! - `province:1234` scope literals anywhere in script.
//!
//! `province = X` also appears outside landed titles (scripted effects, where
//! X is a scope chain, and `common/travel/` point-of-interest types where it
//! is a saved scope) — deliberately not gated in, they are runtime navigation.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Effect, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

const PROVINCE_HISTORY_DIR: &str = "history/provinces/";
const DEFINITION_CSV: &str = "map_data/definition.csv";

/// The holding type built in the province.
const fn holding() -> FieldSpec {
    scalar(Setting)
        .doc("The holding type built here (`none` for an empty slot).")
        .values(&[
            "castle_holding",
            "city_holding",
            "church_holding",
            "tribal_holding",
            "none",
        ])
}

/// The body of one dated block (`1100.1.1 = { … }`): the same historical
/// fields plus arbitrary effects run at that date.
static PROVINCE_DATE: StructSpec = StructSpec {
    name: "province date",
    fields: &[
        (
            "culture",
            scalar(Setting).doc("Change the province culture."),
        ),
        (
            "religion",
            scalar(Setting).doc("Change the province faith."),
        ),
        ("faith", scalar(Setting).doc("Change the province faith.")),
        ("holding", holding()),
        (
            "buildings",
            block(Struct(&PROVINCE_BUILDINGS)).doc("Buildings constructed by this date."),
        ),
        (
            "special_building_slot",
            scalar(Setting).doc("Add a special-building slot of this type."),
        ),
        (
            "special_building",
            scalar(Setting).doc("Construct this special building."),
        ),
        (
            "duchy_capital_building",
            scalar(Setting).doc("Construct this duchy-capital building."),
        ),
        (
            "effect",
            block(Effect).doc("Arbitrary effects run at this date (`root` is the province)."),
        ),
    ],
    // History date blocks freely use effects (`add_special_building = …`).
    fallback: Fallback::Effect,
};

/// `buildings = { X Y … }` — loose building names (refs live in building.rs).
static PROVINCE_BUILDINGS: StructSpec = StructSpec {
    name: "buildings",
    fields: &[],
    fallback: Fallback::Deny,
};

/// The body of one province entry (`<id> = { … }`). Unknown block-valued keys
/// are dates opening [`PROVINCE_DATE`].
static PROVINCE: StructSpec = StructSpec {
    name: "province",
    fields: &[
        ("culture", scalar(Setting).doc("The province's culture.")),
        (
            "religion",
            scalar(Setting).doc("The province's faith (legacy key; same as `faith`)."),
        ),
        ("faith", scalar(Setting).doc("The province's faith.")),
        ("holding", holding()),
        (
            "terrain",
            scalar(Setting).doc("Override the map-derived terrain type."),
        ),
        (
            "buildings",
            block(Struct(&PROVINCE_BUILDINGS)).doc("Buildings present at game start."),
        ),
        (
            "special_building_slot",
            scalar(Setting).doc("A special-building slot of this type."),
        ),
        (
            "special_building",
            scalar(Setting).doc("A constructed special building."),
        ),
        (
            "duchy_capital_building",
            scalar(Setting).doc("A constructed duchy-capital building."),
        ),
    ],
    // Unknown block-valued keys are dates (`1100.1.1 = { … }`).
    fallback: Fallback::Struct(&PROVINCE_DATE),
};

pub(crate) struct Province;

impl Entity for Province {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::PROVINCE,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: DEFINITION_CSV,
            shape: DefShape::IdCsv,
        }),
        refs: &[
            RefRule {
                pattern: RefPattern::TopLevelBlockKeys,
                gate: Some(PROVINCE_HISTORY_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("province"),
                gate: Some("common/landed_titles/"),
                alt: &[],
            },
            // `province:` also accepts a barony title key
            // (`province:b_constantinople`) — the engine resolves the title's
            // province, so titles chain as the alternate kind.
            RefRule {
                pattern: RefPattern::ScopePrefix("province"),
                gate: None,
                alt: &[kinds::TITLE],
            },
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(PROVINCE_HISTORY_DIR, ClauseKind::Struct(&PROVINCE))];
}
