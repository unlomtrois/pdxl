//! Culture pillars (`common/culture/pillars/`, from `_pillars.info`) — the
//! ethos / heritage / language / martial-custom / head-determination traits
//! every culture is built from. The body is the cultural-trait base structure
//! (`_cultural_traits.info`, shared via [`super::culture_shared`]) plus
//! `type`, `color` (languages) and `audio_parameter` (heritages).
//!
//! Cross-references (corpus-validated, vanilla + T4N, 0 unresolved):
//! - The five pillar attribute fields of a culture body (`ethos =`,
//!   `heritage =`, `language =`, `martial_custom =`, `head_determination =`)
//!   — all corpus occurrences are depth-1 inside `common/culture/cultures/`
//!   definitions, so [`RefPattern::KeyValueTop`] with the dir gate.
//! - `has_cultural_pillar = X` — trigger, clean corpus-wide (1666 refs).
//! - `culture_pillar:X` scope literals (500 refs).
//!
//! Note the culture attribute key is `martial_custom` — `_cultures.info`
//! misspells it as `martial_tradition` (0 corpus occurrences).

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, color, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::anywhere;
use super::culture_shared::{
    PILLARS_DIR, TRAIT_AI_WILL_DO, TRAIT_CAN_PICK, TRAIT_CAN_PICK_FOR_HYBRIDIZATION,
    TRAIT_CHARACTER_MODIFIER, TRAIT_COST, TRAIT_COUNTY_MODIFIER, TRAIT_CULTURE_MODIFIER,
    TRAIT_DESC, TRAIT_DOCTRINE_CHARACTER_MODIFIER, TRAIT_ICON, TRAIT_IS_SHOWN, TRAIT_NAME,
    TRAIT_PARAMETERS, TRAIT_PROVINCE_MODIFIER, in_cultures,
};

/// The body of one culture pillar: the cultural-trait base plus the
/// pillar-only fields (`_pillars.info`).
static CULTURE_PILLAR: StructSpec = StructSpec {
    name: "culture_pillar",
    fields: &[
        (
            "type",
            scalar(Setting)
                .doc("Which pillar slot of a culture this fills.")
                .values(&[
                    "ethos",
                    "heritage",
                    "language",
                    "martial_custom",
                    "head_determination",
                ]),
        ),
        (
            "color",
            color().doc("A scripted color or direct color. Only for languages (map coloring)."),
        ),
        (
            "audio_parameter",
            scalar(Setting).doc("Audio-system parameter key. Only for heritages."),
        ),
        (
            "head_determination_type",
            scalar(Setting)
                .doc(
                    "How the culture head is determined. Only for head-determination \
                     pillars (undocumented in `_pillars.info`, but used by vanilla).",
                )
                .values(&["domain", "herd"]),
        ),
        (
            "name",
            TRAIT_NAME.doc(
                "The name (dynamic description). If omitted, uses `<key>_name`; if no \
                 description matches the key either, `<type>_generic_label_desc` is used.",
            ),
        ),
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

pub(crate) struct CulturePillar;

impl Entity for CulturePillar {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CULTURE_PILLAR,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: PILLARS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            // The five pillar slots of a culture body (depth-1 only).
            in_cultures(RefPattern::KeyValueTop("ethos")),
            in_cultures(RefPattern::KeyValueTop("heritage")),
            in_cultures(RefPattern::KeyValueTop("language")),
            in_cultures(RefPattern::KeyValueTop("martial_custom")),
            in_cultures(RefPattern::KeyValueTop("head_determination")),
            anywhere(RefPattern::KeyValue("has_cultural_pillar")),
            anywhere(RefPattern::ScopePrefix("culture_pillar")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(PILLARS_DIR, ClauseKind::Struct(&CULTURE_PILLAR))];
}
