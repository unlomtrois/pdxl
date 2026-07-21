//! Fragments shared across the culture domain (`common/culture/*`): the
//! directory constants every culture concept gates on, and the cultural-trait
//! body fields from `_cultural_traits.info` — pillars and traditions share
//! the exact same base structure (name/desc/icon, cost, modifiers, pick
//! triggers, parameters, AI), each adding a few fields of its own.

use pdxl_analysis::context::ClauseKind::{DynamicDesc, ScriptValue, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{FieldSpec, block, block_scoped, scalar, scalar_or_block};
use pdxl_analysis::{RefPattern, RefRule};

use super::common::{COST, OPAQUE};

pub(crate) const CULTURES_DIR: &str = "common/culture/cultures/";
pub(crate) const PILLARS_DIR: &str = "common/culture/pillars/";
pub(crate) const TRADITIONS_DIR: &str = "common/culture/traditions/";
pub(crate) const ERAS_DIR: &str = "common/culture/eras/";
pub(crate) const INNOVATIONS_DIR: &str = "common/culture/innovations/";
pub(crate) const NAME_LISTS_DIR: &str = "common/culture/name_lists/";

/// A reference rule gated to culture definition bodies
/// (`common/culture/cultures/`).
pub(crate) const fn in_cultures(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(CULTURES_DIR),
        alt: &[],
    }
}

/// A reference rule gated to innovation bodies (`common/culture/innovations/`).
pub(crate) const fn in_innovations(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(INNOVATIONS_DIR),
        alt: &[],
    }
}

// ── cultural-trait base fields (`_cultural_traits.info`) ────────────────────
// Shared by pillars and traditions; docs live here once.

pub(crate) const TRAIT_NAME: FieldSpec = scalar_or_block(LocKey, DynamicDesc)
    .doc("The name (dynamic description). If omitted, uses `<key>_name`.");

pub(crate) const TRAIT_DESC: FieldSpec = scalar_or_block(LocKey, DynamicDesc)
    .doc("The description (dynamic description). If omitted, uses `<key>_desc`.");

pub(crate) const TRAIT_ICON: FieldSpec =
    scalar(Setting).doc("The icon key. If omitted, uses the trait's own key.");

pub(crate) const TRAIT_COST: FieldSpec =
    block(Struct(&COST)).doc("Cost to pick this trait: `gold`/`prestige`/`piety` script values.");

pub(crate) const TRAIT_CHARACTER_MODIFIER: FieldSpec =
    block(Struct(&OPAQUE)).doc("Modifier applied to characters of any culture that has the trait.");

pub(crate) const TRAIT_PROVINCE_MODIFIER: FieldSpec =
    block(Struct(&OPAQUE)).doc("Modifier applied to provinces of any culture that has the trait.");

pub(crate) const TRAIT_COUNTY_MODIFIER: FieldSpec =
    block(Struct(&OPAQUE)).doc("Modifier applied to counties of any culture that has the trait.");

pub(crate) const TRAIT_CULTURE_MODIFIER: FieldSpec = block(Struct(&OPAQUE)).doc(
    "Modifier applied to the culture itself (undocumented in `_cultural_traits.info`, \
     but used by vanilla pillars and traditions).",
);

pub(crate) const TRAIT_DOCTRINE_CHARACTER_MODIFIER: FieldSpec = block(Struct(&OPAQUE)).doc(
    "Applied to characters of cultures with the trait if they have the given doctrine \
     (`doctrine = <key>` plus modifier lines).",
);

pub(crate) const TRAIT_CAN_PICK: FieldSpec = block_scoped(Trigger, "culture").doc(
    "Can this trait be picked? Culture scope, plus `scope:character`; the list `traits` \
    holds all traits currently picked (for mutual exclusivity — which must go in BOTH \
    directions, or the AI can trap itself mid-divergence). When a trait is being \
    replaced, `scope:replacing` is a flag of a culture with the replacing culture's \
    key (never set for pillars). Hybridization ignores this and `is_shown` entirely.",
);

pub(crate) const TRAIT_CAN_PICK_FOR_HYBRIDIZATION: FieldSpec = block_scoped(Trigger, "culture")
    .doc(
        "Like `can_pick`, but used specifically — and only — for hybridization. Never has \
         `scope:replacing`.",
    );

pub(crate) const TRAIT_IS_SHOWN: FieldSpec = block_scoped(Trigger, "culture").doc(
    "Should this trait be shown at all when picking traits? Culture scope, plus \
     `scope:character`.",
);

pub(crate) const TRAIT_PARAMETERS: FieldSpec = block(Struct(&OPAQUE)).doc(
    "`param_name = yes/no` (or a fixed-point number). Queried via \
     `has_cultural_parameter`; some parameters (e.g. `number_of_spouses`) interact \
     directly with code.",
);

pub(crate) const TRAIT_AI_WILL_DO: FieldSpec = block_scoped(ScriptValue, "culture")
    .doc("AI pick weight (script value). Same scopes as `can_pick`.");
