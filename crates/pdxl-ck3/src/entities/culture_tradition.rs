//! Culture traditions (`common/culture/traditions/`, from `_traditions.info`)
//! — pickable cultural traits. The body is the cultural-trait base structure
//! (`_cultural_traits.info`, shared via [`super::culture_shared`]) plus
//! `category` and `layers`.
//!
//! Cross-references (corpus-validated, vanilla + T4N, 0 unresolved):
//! - `traditions = { X Y … }` — the culture body's tradition list (933 refs,
//!   gated to `common/culture/cultures/`).
//! - `dlc_tradition = { trait = X fallback = Y }` — a culture's conditional
//!   DLC tradition and its non-DLC replacement (169 + 71 refs, same gate).
//! - `has_cultural_tradition = X` (trigger, 2764 refs; the `= prev`
//!   comparison form is skipped by the scope-keyword rule) and the
//!   `add_culture_tradition` / `remove_culture_tradition` effects.
//! - `culture_tradition:X` scope literals (392 refs).

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{OPAQUE, anywhere};
use super::culture_shared::{
    TRADITIONS_DIR, TRAIT_AI_WILL_DO, TRAIT_CAN_PICK, TRAIT_CAN_PICK_FOR_HYBRIDIZATION,
    TRAIT_CHARACTER_MODIFIER, TRAIT_COST, TRAIT_COUNTY_MODIFIER, TRAIT_CULTURE_MODIFIER,
    TRAIT_DESC, TRAIT_DOCTRINE_CHARACTER_MODIFIER, TRAIT_ICON, TRAIT_IS_SHOWN, TRAIT_NAME,
    TRAIT_PARAMETERS, TRAIT_PROVINCE_MODIFIER, in_cultures,
};

/// The body of one culture tradition: the cultural-trait base plus the
/// tradition-only fields (`_traditions.info`).
static CULTURE_TRADITION: StructSpec = StructSpec {
    name: "culture_tradition",
    fields: &[
        (
            "category",
            scalar(Setting)
                .doc(
                    "Grouping in the Add Tradition view and the Divergence view. The corpus \
                     vocabulary is a clean five-value enum.",
                )
                .values(&["regional", "realm", "societal", "combat", "ritual"]),
        ),
        (
            "layers",
            block(Struct(&OPAQUE)).doc(
                "Icon layers, matching the `CULTURE_TRADITION_LAYER_PATHS` define (index \
                 starts at 0): `0 = martial` picks a random icon from that subfolder, \
                 `3 = letter1.dds` a specific file.",
            ),
        ),
        ("name", TRAIT_NAME),
        ("desc", TRAIT_DESC),
        ("icon", TRAIT_ICON),
        ("cost", TRAIT_COST),
        ("character_modifier", TRAIT_CHARACTER_MODIFIER),
        ("province_modifier", TRAIT_PROVINCE_MODIFIER),
        ("county_modifier", TRAIT_COUNTY_MODIFIER),
        ("culture_modifier", TRAIT_CULTURE_MODIFIER),
        (
            "doctrine_character_modifier",
            TRAIT_DOCTRINE_CHARACTER_MODIFIER,
        ),
        ("can_pick", TRAIT_CAN_PICK),
        (
            "can_pick_for_hybridization",
            TRAIT_CAN_PICK_FOR_HYBRIDIZATION,
        ),
        ("is_shown", TRAIT_IS_SHOWN),
        ("parameters", TRAIT_PARAMETERS),
        ("ai_will_do", TRAIT_AI_WILL_DO),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct CultureTradition;

impl Entity for CultureTradition {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CULTURE_TRADITION,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: TRADITIONS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            in_cultures(RefPattern::KeyList("traditions")),
            in_cultures(RefPattern::KeyBlockField("dlc_tradition", "trait")),
            in_cultures(RefPattern::KeyBlockField("dlc_tradition", "fallback")),
            anywhere(RefPattern::KeyValue("has_cultural_tradition")),
            anywhere(RefPattern::KeyValue("add_culture_tradition")),
            anywhere(RefPattern::KeyValue("remove_culture_tradition")),
            anywhere(RefPattern::ScopePrefix("culture_tradition")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(TRADITIONS_DIR, ClauseKind::Struct(&CULTURE_TRADITION))];
}
