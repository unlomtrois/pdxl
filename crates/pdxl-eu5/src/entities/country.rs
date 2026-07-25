//! Countries (`in_game/setup/countries/`, from `00_readme.info`) — one
//! `TAG = { … }` block per country (2,340 in vanilla), plus **formable
//! countries** (`in_game/common/formable_countries/`, `TAG_f = { … }`),
//! whose `tag = X` body field declares the tag the formable creates — an
//! alias, so `c:RUS` resolves to the formable that forms Russia.
//!
//! **Start-scenario countries** (`main_menu/setup/start/`, the nested
//! `countries = { countries = { TAG = { … } } }` containers) are a third
//! declaration site — some tags (Genoa's `GEN`) exist *only* there. They
//! are their own kind so the ~2,000 tags declared in both places don't
//! read as duplicate definitions.
//!
//! References: `c:TAG` scope literals anywhere (6,384 in vanilla), with
//! formables and start countries as the alternate kinds.
//!
//! Body cross-references live with their target kinds:
//! `culture_definition` in [`super::culture`], `religion_definition` in
//! [`super::religion`], the color fields in [`super::named_color`].
//! `is_historic` and the `unit_color*` slots are corpus-real but absent
//! from the `.info`.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, color, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

pub(crate) const COUNTRIES_DIR: &str = "in_game/setup/countries/";
const FORMABLES_DIR: &str = "in_game/common/formable_countries/";
const START_DIR: &str = "main_menu/setup/start/";

/// A name-list block (`male_regnal_names = { … }`).
static NAME_LIST: StructSpec = StructSpec {
    name: "regnal names",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// The body of one country (`00_readme.info` + corpus).
static COUNTRY: StructSpec = StructSpec {
    name: "country",
    fields: &[
        (
            "color",
            color().doc("The country's map color — a named color or a color literal."),
        ),
        (
            "color2",
            color().doc("The secondary color (striped map modes, CoA defaults)."),
        ),
        (
            "unit_color0",
            color().doc("Unit model color slot 0 (corpus-real; not in the .info)."),
        ),
        (
            "unit_color1",
            color().doc("Unit model color slot 1 (corpus-real; not in the .info)."),
        ),
        (
            "unit_color2",
            color().doc("Unit model color slot 2 (corpus-real; not in the .info)."),
        ),
        (
            "culture_definition",
            scalar(Setting).doc("The country's primary culture (`in_game/common/cultures/`)."),
        ),
        (
            "religion_definition",
            scalar(Setting).doc("The country's religion (`in_game/common/religions/`)."),
        ),
        (
            "description_category",
            scalar(Setting).doc(
                "The country-description category \
                 (`in_game/common/country_description_categories/`).",
            ),
        ),
        (
            "difficulty",
            scalar(Setting)
                .doc("Suggested player difficulty, 1–5.")
                .values(&["1", "2", "3", "4", "5"]),
        ),
        (
            "is_historic",
            scalar(Setting)
                .doc("Whether the tag is historic (corpus-real; not in the .info).")
                .values(&["yes", "no"]),
        ),
        (
            "male_regnal_names",
            block(Struct(&NAME_LIST)).doc("Regnal names for male rulers."),
        ),
        (
            "female_regnal_names",
            block(Struct(&NAME_LIST)).doc("Regnal names for female rulers."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Country;

impl Entity for Country {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::COUNTRY,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: COUNTRIES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                // `c:TAG` anywhere; formables chain as the alternate kind
                // (their `tag = X` aliases carry the formed tags).
                RefRule {
                    pattern: RefPattern::ScopePrefix("c"),
                    gate: None,
                    alt: &[kinds::FORMABLE_COUNTRY, kinds::START_COUNTRY],
                },
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::START_COUNTRY,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: START_DIR,
                shape: DefShape::ChildrenOf {
                    containers: &["countries"],
                },
            }),
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::FORMABLE_COUNTRY,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: FORMABLES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[],
            // The formed tag resolves to the formable's definition.
            aliases: &["tag"],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(COUNTRIES_DIR, ClauseKind::Struct(&COUNTRY))];
}
