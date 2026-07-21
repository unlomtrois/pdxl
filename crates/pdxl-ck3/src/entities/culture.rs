//! Cultures (`common/culture/cultures/`, from `_cultures.info`) — templates
//! for the cultural parameters of an ethnicity; the actual cultures are
//! instanced dynamically (hybridization/divergence) from these.
//!
//! Referenced by `culture:x` scope literals, the `culture =` attribute of
//! history characters and dynasties, and — inside culture bodies — the
//! `parents = { X Y }` list of hybrid/divergent ancestry (97 refs, corpus-
//! validated at 0 unresolved; the only culture-name-referencing key found in
//! culture bodies).
//!
//! The body's other outbound references live with their target kinds
//! (loc-rule precedent): pillar fields in [`super::culture_pillar`],
//! `traditions` / `dlc_tradition` in [`super::culture_tradition`],
//! `name_list` in [`super::name_list`]. The `ethnicities` weight-keyed block
//! resolves against `common/ethnicities/` — a kind not yet modeled, so it
//! stays unwired.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{OPAQUE, anywhere};
use super::culture_shared::{CULTURES_DIR, in_cultures};

/// `culture = X` gated to one directory (a bare `culture =` is a scope
/// assignment elsewhere).
const fn culture_in(dir: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue("culture"),
        gate: Some(dir),
        alt: &[],
    }
}

/// `dlc_fallback_pillar = { … }` — replace a pillar when a DLC is missing.
static DLC_FALLBACK_PILLAR: StructSpec = StructSpec {
    name: "dlc_fallback_pillar",
    fields: &[
        (
            "fallback",
            scalar(Setting).doc("Replace with this pillar if you lack the DLC feature."),
        ),
        (
            "requires_dlc_flag",
            scalar(Setting).doc("The DLC feature flag this pillar depends on."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `dlc_tradition = { … }` — a tradition added only with a DLC feature.
static DLC_TRADITION: StructSpec = StructSpec {
    name: "dlc_tradition",
    fields: &[
        (
            "trait",
            scalar(Setting).doc("Add this tradition if you have the DLC feature."),
        ),
        (
            "requires_dlc_flag",
            scalar(Setting).doc("The DLC feature flag this tradition depends on."),
        ),
        (
            "fallback",
            scalar(Setting).doc("Add this tradition if you don't (optional)."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one culture template (`_cultures.info`; field set confirmed
/// against the vanilla + T4N corpus).
static CULTURE: StructSpec = StructSpec {
    name: "culture",
    fields: &[
        (
            "color",
            scalar_or_block(Setting, Struct(&OPAQUE)).doc(
                "The color of the culture, used e.g. on the map — `{ r g b }`, `hsv{ … }`, \
                 or a named scripted color.",
            ),
        ),
        ("created", scalar(Setting).doc("Optional creation date.")),
        (
            "history_loc_override",
            scalar(LocKey).doc("A customloc key for history rather than the default one."),
        ),
        (
            "traditions",
            block(Struct(&OPAQUE)).doc("The culture's traditions (a list of tradition keys)."),
        ),
        (
            "name_order_convention",
            scalar(Setting)
                .doc(
                    "How a person's name behaves with respect to the dynasty: a lowercase \
                     suffix appended to the base character localization keys \
                     (`CHARACTER_FIRST_NAME_NICKNAMED_<SUFFIX>`). Omit for the default \
                     western convention; only relevant for characters with a dynasty.",
                )
                .values(&["dynasty_always_first", "dynasty_first", "japanese"]),
        ),
        ("ethos", scalar(Setting).doc("The culture's ethos pillar.")),
        (
            "heritage",
            scalar(Setting).doc("The culture's heritage pillar."),
        ),
        (
            "language",
            scalar(Setting).doc("The culture's language pillar."),
        ),
        (
            "martial_custom",
            scalar(Setting).doc("The culture's martial-custom pillar."),
        ),
        (
            "head_determination",
            scalar(Setting).doc("The culture's head-determination pillar."),
        ),
        (
            "name_list",
            scalar(Setting).doc(
                "How to name things. Repeatable; the first entry is the primary one, used \
                 for things like prefixes where randomizing between lists makes no sense.",
            ),
        ),
        (
            "dlc_fallback_pillar",
            block(Struct(&DLC_FALLBACK_PILLAR))
                .doc("Replace a pillar with a fallback if the DLC feature is missing."),
        ),
        (
            "dlc_tradition",
            block(Struct(&DLC_TRADITION)).doc(
                "Add a tradition only with the DLC feature (optionally with a non-DLC \
                 fallback).",
            ),
        ),
        (
            "character_modifier",
            block(Struct(&OPAQUE)).doc("Modifier effects on all characters of the culture."),
        ),
        (
            "ethnicities",
            block(Struct(&OPAQUE)).doc(
                "`<weight> = <ethnicity>` entries; the weight says how common the \
                 ethnicity is within the culture.",
            ),
        ),
        (
            "parents",
            block(Struct(&OPAQUE)).doc(
                "The parent cultures of a hybrid/divergent culture (up to two culture \
                 keys).",
            ),
        ),
        (
            "building_gfx",
            block(Struct(&OPAQUE)).doc(
                "Building GFX set keys. The first key names the GFX culture; the sequence \
                 must be identical for every set starting with the same GFX culture.",
            ),
        ),
        (
            "clothing_gfx",
            block(Struct(&OPAQUE))
                .doc("Clothing GFX set keys (multiple sections can represent hybrids)."),
        ),
        ("unit_gfx", block(Struct(&OPAQUE)).doc("Unit GFX set keys.")),
        (
            "coa_gfx",
            block(Struct(&OPAQUE))
                .doc("Coat-of-arms GFX set keys (multiple sections can represent hybrids)."),
        ),
        (
            "house_coa_frame",
            scalar(Setting).doc(
                "CoA frame for houses of this culture; `<frame>.dds` and \
                 `<frame>_mask.dds` must exist in `gfx/interface/coat_of_arms/frames`.",
            ),
        ),
        (
            "dynasty_coa_frame",
            scalar(Setting).doc("CoA frame for dynasties of this culture."),
        ),
        (
            "house_coa_mask_offset",
            block(Struct(&OPAQUE)).doc("`{ x y }` offset of the house CoA mask."),
        ),
        (
            "house_coa_mask_scale",
            block(Struct(&OPAQUE)).doc("`{ x y }` scale of the house CoA mask."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Culture;

impl Entity for Culture {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CULTURE,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: CULTURES_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::ScopePrefix("culture")),
            culture_in("history/characters/"),
            culture_in("common/dynasties/"),
            // Hybrid/divergent ancestry inside a culture body.
            in_cultures(RefPattern::KeyList("parents")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(CULTURES_DIR, ClauseKind::Struct(&CULTURE))];
}
