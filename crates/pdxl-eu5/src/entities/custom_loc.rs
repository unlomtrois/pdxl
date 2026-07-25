//! Customizable localization (`in_game/common/customizable_localization/`,
//! documented by `customizable_localization.info`) — scripted selectors whose
//! first (or random) valid `text` entry supplies a localization key.
//!
//! Corpus notes (76k lines / 284 definitions in vanilla): definitions either
//! contain `type` + repeated `text` entries, or derive from `parent` and append
//! `suffix`. `log_loc_errors` and `if_invalid_loc` are corpus fields omitted by
//! the format example. `localization_key` is deliberately documentation-only:
//! this directory contains language-specific keys which need not exist in the
//! configured localization language.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

pub(crate) const CUSTOM_LOC_DIR: &str = "in_game/common/customizable_localization/";

const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

static TEXT_ENTRY: StructSpec = StructSpec {
    name: "customizable localization text",
    fields: &[
        (
            "trigger",
            block(Trigger).doc("When this passes, this entry's localization key is used."),
        ),
        (
            "localization_key",
            scalar(LocKey).doc(
                "Localization key returned by this entry. It may be defined only in a non-default language, so it is not validated as a reference.",
            ),
        ),
        ("fallback", toggle("Use this entry if no triggered entry is valid.")),
    ],
    fallback: Fallback::Deny,
};

static CUSTOM_LOC: StructSpec = StructSpec {
    name: "customizable localization",
    fields: &[
        (
            "type",
            scalar(Setting)
                .doc("Scope type on which this selector is evaluated.")
                .values(&[
                    "country",
                    "character",
                    "location",
                    "special_status",
                    "international_organization",
                    "sub_unit",
                    "culture",
                    "none",
                    "situation",
                    "culture_groups",
                ]),
        ),
        (
            "text",
            block(Struct(&TEXT_ENTRY)).doc("A candidate localization result (repeatable)."),
        ),
        (
            "random_valid",
            toggle("Choose randomly among valid text entries instead of taking the first."),
        ),
        (
            "log_loc_errors",
            toggle("Whether missing localization should be logged. *(corpus)*"),
        ),
        (
            "if_invalid_loc",
            scalar(Setting)
                .doc("Behavior when the selected localization key is invalid. *(corpus)*")
                .values(&["return_empty", "fallback_to_next_entry", "return_loc_key"]),
        ),
        (
            "parent",
            scalar(Setting).doc("Run this selector's logic, then append `suffix`."),
        ),
        (
            "suffix",
            scalar(Setting).doc("Suffix appended to the localization key selected by `parent`."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct CustomLoc;

impl Entity for CustomLoc {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CUSTOM_LOC,
        icon: IconHint::Text,
        defs: Some(DefSource {
            dir_prefix: CUSTOM_LOC_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            RefRule {
                pattern: RefPattern::KeyValueTop("parent"),
                gate: Some(CUSTOM_LOC_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("custom_name"),
                gate: Some(super::international_organization::IO_DIR),
                alt: &[],
            },
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(CUSTOM_LOC_DIR, Struct(&CUSTOM_LOC))];
}
