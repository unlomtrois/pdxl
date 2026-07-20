//! Culture innovations (`common/culture/innovations/`, from
//! `_culture_innovations.info`) — era-bound technologies cultures gradually
//! unlock.
//!
//! Cross-references (corpus-validated, vanilla + T4N, 0 unresolved):
//! - `has_innovation = X` — trigger, clean corpus-wide (1538 refs).
//! - `culture_innovation:X` scope literals (18 refs).
//!
//! The innovation body's own outbound references live with their target
//! kinds: `culture_era` in [`super::culture_era`], `unlock_casus_belli` in
//! [`super::casus_belli`], `unlock_law` in [`super::law`]
//! (`unlock_decision`/`unlock_building`/`unlock_maa` have no corpus
//! occurrences or no modeled target kind and stay unwired).

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, ScriptValue, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{OPAQUE, anywhere};
use super::culture_shared::INNOVATIONS_DIR;

/// A triggered name/icon style for an innovation (`asset = { … }`).
static INNOVATION_ASSET: StructSpec = StructSpec {
    name: "innovation_asset",
    fields: &[
        (
            "trigger",
            block_scoped(Trigger, "culture").doc(
                "Culture-scoped trigger deciding whether a culture uses this asset. Base it \
                 on static culture data (aesthetics, heritage); the game may not be fully \
                 loaded during evaluation.",
            ),
        ),
        (
            "name",
            scalar(LocKey).doc(
                "The base loc key to use in this case. Optional, but at least one of `name` \
                 and `icon` must be defined.",
            ),
        ),
        (
            "icon",
            scalar(Setting).doc(
                "The icon to use in this case. Optional, but at least one of `name` and \
                 `icon` must be defined.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `maa_upgrade = { … }` — a stat upgrade to an existing men-at-arms type.
/// Shared with [`super::culture_era`]: era bodies use the same shape
/// (undocumented in `_culture_eras.info`, but common in vanilla eras).
pub(super) static MAA_UPGRADE: StructSpec = StructSpec {
    name: "maa_upgrade",
    fields: &[
        (
            "type",
            scalar(Setting).doc("The base men-at-arms type to upgrade."),
        ),
        ("damage", scalar(Setting)),
        ("toughness", scalar(Setting)),
        ("pursue", scalar(Setting)),
        ("screen", scalar(Setting)),
        ("siege_value", scalar(Setting)),
        ("max_size", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// The body of one culture innovation (`_culture_innovations.info`).
static CULTURE_INNOVATION: StructSpec = StructSpec {
    name: "culture_innovation",
    fields: &[
        (
            "culture_era",
            scalar(Setting).doc(
                "Key of the cultural era this innovation belongs to (e.g. \
                 `culture_era_early_medieval`).",
            ),
        ),
        (
            "group",
            scalar(Setting).doc("The innovation group.").values(&[
                "culture_group_military",
                "culture_group_civic",
                "culture_group_regional",
            ]),
        ),
        (
            "icon",
            scalar(Setting).doc(
                "Path to the default icon. Falls back to \
                 `NGameIcons::DEFAULT_CULTURE_INNOVATION_TYPE_ICON_PATH` when unset.",
            ),
        ),
        (
            "skill",
            scalar(Setting)
                .doc(
                    "The skill the Head of Culture uses to compute fascination bonuses \
                     (default `learning`).",
                )
                .values(&[
                    "learning",
                    "martial",
                    "stewardship",
                    "diplomacy",
                    "intrigue",
                ]),
        ),
        (
            "ai_weight_for_spread",
            block(ScriptValue).doc(
                "Weight for randomly picking this innovation when selecting spread (AI \
                 culture heads). `root` = the culture head evaluating it.",
            ),
        ),
        (
            "ai_weight_for_fascination",
            block_scoped(ScriptValue, "culture").doc(
                "Weight for picking this innovation as the fascination. `root` = the \
                 culture, `scope:character` = the cultural head.",
            ),
        ),
        (
            "asset",
            block(Struct(&INNOVATION_ASSET)).doc(
                "Optional triggered assets (repeatable): the first whose trigger passes \
                 styles the innovation's name and icon. Calculated on startup or culture \
                 creation, then never updated; definition order is priority order.",
            ),
        ),
        (
            "region",
            scalar(Setting).doc(
                "Optional key of the region where this innovation can start getting base \
                 progress (the culture needs a minimum presence there). Empty means \
                 anywhere.",
            ),
        ),
        (
            "potential",
            block_scoped(Trigger, "culture").doc(
                "Can it be unlocked by the culture? Hidden otherwise (unlike \
                 `can_progress`). Culture scope; default `always = yes`.",
            ),
        ),
        (
            "can_progress",
            block_scoped(Trigger, "culture")
                .doc("Can it start being exposed? Culture scope; default `always = yes`."),
        ),
        (
            "character_modifier",
            block(Struct(&OPAQUE)).doc("Modifier applied to characters of the culture."),
        ),
        (
            "culture_modifier",
            block(Struct(&OPAQUE)).doc("Modifier applied to the culture itself."),
        ),
        (
            "county_modifier",
            block(Struct(&OPAQUE)).doc("Modifier applied to counties of the culture."),
        ),
        (
            "province_modifier",
            block(Struct(&OPAQUE)).doc("Modifier applied to provinces in a county of the culture."),
        ),
        (
            "parameters",
            block(Struct(&OPAQUE)).doc(
                "Optional boolean parameters, defined like trait/tradition parameters and \
                 queryable via script triggers.",
            ),
        ),
        (
            "flag",
            scalar(Setting).doc(
                "Optional free-form flag (repeatable), relevant for the \
                 `has_all_innovations` trigger — e.g. `global_regular`, \
                 `tribal_era_regional`, `silk_road_innovation`.",
            ),
        ),
        (
            "unlock_building",
            scalar(Setting).doc(
                "Key of a building that can be unlocked (repeatable). Tooltip-only: the \
                 unlock must be manually blocked on the building itself.",
            ),
        ),
        (
            "unlock_decision",
            scalar(Setting).doc(
                "Key of a decision that can be unlocked (repeatable). Tooltip-only: the \
                 unlock must be manually blocked on the decision itself.",
            ),
        ),
        (
            "unlock_casus_belli",
            scalar(Setting).doc(
                "Key of a casus belli that can be unlocked (repeatable). Tooltip-only: the \
                 unlock must be manually blocked on the CB itself.",
            ),
        ),
        (
            "unlock_maa",
            scalar(Setting).doc(
                "Key of a men-at-arms regiment that can be unlocked (repeatable). Actually \
                 does unlock the MaA.",
            ),
        ),
        (
            "unlock_law",
            scalar(Setting).doc(
                "Key of a law that can be unlocked (repeatable). Tooltip-only: the unlock \
                 must be manually blocked on the law itself.",
            ),
        ),
        (
            "custom",
            scalar(LocKey)
                .doc("A custom effect description added to the list of effects (repeatable)."),
        ),
        (
            "maa_upgrade",
            block(Struct(&MAA_UPGRADE))
                .doc("Optional stat upgrade to an existing men-at-arms type (repeatable)."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct CultureInnovation;

impl Entity for CultureInnovation {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CULTURE_INNOVATION,
        icon: IconHint::Function,
        defs: Some(DefSource {
            dir_prefix: INNOVATIONS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("has_innovation")),
            anywhere(RefPattern::ScopePrefix("culture_innovation")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(INNOVATIONS_DIR, ClauseKind::Struct(&CULTURE_INNOVATION))];
}
