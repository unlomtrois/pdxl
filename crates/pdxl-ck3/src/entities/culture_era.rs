//! Culture eras (`common/culture/eras/`, from `_culture_eras.info`) — the
//! four historical eras innovations are bound to.
//!
//! Cross-reference (corpus-validated, vanilla + T4N, 0 unresolved): every
//! innovation names its era via `culture_era = X` (120 refs). The key occurs
//! nowhere outside `common/culture/innovations/`, so the gate is optional —
//! kept anyway for symmetry with the other culture-domain rules. A literal
//! `culture_era:X` scope prefix never occurs in the corpus, so no scope rule.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::OPAQUE;
use super::culture_shared::{ERAS_DIR, in_innovations};

/// The body of one culture era (`_culture_eras.info`). The `unlock_*` fields
/// are tooltip-only (the unlock must be blocked on the object itself); they
/// carry zero references in the whole corpus, so only the era's own shape is
/// modeled here — the innovation-side `unlock_*` reference rules live with
/// their target kinds (casus belli, law).
static CULTURE_ERA: StructSpec = StructSpec {
    name: "culture_era",
    fields: &[
        (
            "year",
            scalar(Setting).doc(
                "Year when the era can start getting base spread. Must be 0 or greater; \
                 an error if not set.",
            ),
        ),
        (
            "character_modifier",
            block(Struct(&OPAQUE))
                .doc("Modifier applied to characters of the culture with a valid government."),
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
                "Key of a men-at-arms regiment that can be unlocked (repeatable). \
                 Tooltip-only: the unlock must be manually blocked on the regiment itself.",
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
            "invalid_for_government",
            scalar(Setting).doc(
                "Key of a government that can't use the innovations in this era \
                 (repeatable).",
            ),
        ),
        (
            "custom",
            scalar(LocKey).doc("A custom effect description added to the list of effects."),
        ),
        (
            "maa_upgrade",
            block(Struct(&super::culture_innovation::MAA_UPGRADE)).doc(
                "Stat upgrade to an existing men-at-arms type (repeatable) — same shape \
                 as the innovation field (undocumented in `_culture_eras.info`, but \
                 common in vanilla eras).",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct CultureEra;

impl Entity for CultureEra {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CULTURE_ERA,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: ERAS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[in_innovations(RefPattern::KeyValue("culture_era"))],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(ERAS_DIR, ClauseKind::Struct(&CULTURE_ERA))];
}
