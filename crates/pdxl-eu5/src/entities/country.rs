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
//! References — all with formables and start countries as alternate kinds:
//! - `c:TAG` scope literals anywhere (6,384 in vanilla);
//! - `has_or_had_tag = X` anywhere (2,618, 0 unresolved);
//! - bare `tag = X` in the dirs where it always means a country tag
//!   (events, setup, formables, on_action, generic_actions — ~15k refs,
//!   1 unresolved). **Deliberately not** gated into
//!   `customizable_localization/`: its 44k `tag =` comparisons include ~3%
//!   dynamic/never-defined tags (`tag = MEDICI` grammar checks), which would
//!   be phantom noise;
//! - `first`/`second` diplomacy pairs and `coa` flag references in their
//!   setup dirs (809 + 809 + 262, 0 unresolved).
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
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern, RefRule,
};

use super::Entity;

pub(crate) const COUNTRIES_DIR: &str = "in_game/setup/countries/";
const FORMABLES_DIR: &str = "in_game/common/formable_countries/";
const START_DIR: &str = "main_menu/setup/start/";

/// The alternate kinds every country reference chains through (dynamic
/// countries are the script-created tags — `define_unique_country_tag`).
const COUNTRY_ALTS: &[pdxl_analysis::KindId] = &[
    kinds::FORMABLE_COUNTRY,
    kinds::START_COUNTRY,
    kinds::DYNAMIC_COUNTRY,
];

/// A `key = X` country-tag reference gated to one directory.
const fn tag_in(dir: &'static str, key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(dir),
        alt: COUNTRY_ALTS,
    }
}

/// The COUNTRY reference rules: the `c:` literal and `has_or_had_tag`
/// anywhere, the `first`/`second` diplomacy pairs, plus one gated `tag = X`
/// rule per directory where the key always means a country tag (surveyed
/// via `pdxl-graph` + full corpus scan; every listed dir is 0-unresolved
/// once dynamic tags count).
macro_rules! country_refs {
    ($($dir:literal),* $(,)?) => {
        [
            // The `c:` scope literal is table-derived (see `crate::derived`).
            RefRule {
                pattern: RefPattern::KeyValue("has_or_had_tag"),
                gate: None,
                alt: COUNTRY_ALTS,
            },
            tag_in("main_menu/setup/", "first"),
            tag_in("main_menu/setup/", "second"),
            // historical_scores' `tag` doubles as a display flag: a country
            // tag OR a coat-of-arms key (`tag = FRA_revolutionary_republic`
            // is Napoleon's tricolore) — the CoA joins the alt chain.
            RefRule {
                pattern: RefPattern::KeyValue("important_country"),
                gate: Some(super::religion::RELIGIONS_DIR),
                alt: &[kinds::FORMABLE_COUNTRY, kinds::START_COUNTRY, kinds::DYNAMIC_COUNTRY],
            },
            RefRule {
                pattern: RefPattern::KeyValue("tag"),
                gate: Some("in_game/common/historical_scores/"),
                alt: &[
                    kinds::FORMABLE_COUNTRY,
                    kinds::START_COUNTRY,
                    kinds::DYNAMIC_COUNTRY,
                    kinds::COAT_OF_ARMS,
                ],
            },
            $(tag_in($dir, "tag"),)*
        ]
    };
}

/// The `tag = X` gates. **Deliberately excluded**:
/// `in_game/common/customizable_localization/` (its 44k comparisons include
/// ~1,200 grammar-check tags that exist nowhere statically — `tag = MEDICI`)
/// and `main_menu/gfx/` (the city_data asset DSL's `tag = raw_resource`).
static COUNTRY_REFS: [RefRule; 36] = country_refs!(
    "in_game/events/",
    "in_game/setup/",
    "main_menu/setup/",
    "main_menu/common/achievements/",
    "in_game/common/ai_scripted_expansion_score/",
    "in_game/common/building_types/",
    "in_game/common/cabinet_actions/",
    "in_game/common/casus_belli/",
    "in_game/common/country_interactions/",
    "in_game/common/disasters/",
    "in_game/common/estate_privileges/",
    "in_game/common/formable_countries/",
    "in_game/common/generic_action_ai_lists/",
    "in_game/common/generic_actions/",
    "in_game/common/government_reforms/",
    "in_game/common/heir_selections/",
    "in_game/common/insults/",
    "in_game/common/international_organization_special_statuses/",
    "in_game/common/international_organizations/",
    "in_game/common/join_war_rules/",
    "in_game/common/laws/",
    "in_game/common/on_action/",
    "in_game/common/peace_treaties/",
    "in_game/common/resolutions/",
    "in_game/common/rival_criteria/",
    "in_game/common/scriptable_hints/",
    "in_game/common/scripted_country_names/",
    "in_game/common/scripted_relations/",
    "in_game/common/scripted_triggers/",
    "in_game/common/situations/",
    "in_game/common/subject_types/",
);

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
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::COUNTRY,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::COUNTRY,
            suffix: "_ADJ",
        },
        ImplicitLocPattern {
            kind: kinds::START_COUNTRY,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::START_COUNTRY,
            suffix: "_ADJ",
        },
        // Formable tags are aliases (`tag = RUS`) of the `RUS_f`
        // definition. Registering the convention on this kind lets the alias
        // backlink from `RUS`/`RUS_ADJ` reach that definition.
        ImplicitLocPattern {
            kind: kinds::FORMABLE_COUNTRY,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::FORMABLE_COUNTRY,
            suffix: "_ADJ",
        },
    ];

    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::COUNTRY,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: COUNTRIES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &COUNTRY_REFS,
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
