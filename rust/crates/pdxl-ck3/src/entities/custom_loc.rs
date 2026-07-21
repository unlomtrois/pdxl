//! Customizable localization (`common/customizable_localization/`, from
//! `_custom_loc.info`) — scripted text selectors: a key holds `text` entries
//! whose first (or, with `random_valid`, a random) passing trigger picks a
//! localization key. Invoked from loc/gui text via the `Custom('Key')` /
//! `Custom2('Key', scope)` datafunctions.
//!
//! References:
//! - `parent = X` (variant defs: run the parent's logic, then append
//!   `suffix`) — gated, depth-1; corpus-validated at 0 unresolved.
//! - `localization_key = X` is **deliberately not** a resolvable loc-key
//!   reference: ~12% of the corpus values (`CustomLoc_ES_del`,
//!   `CustomLoc_DE_Blank`, …) are defined only in non-English localization,
//!   which pdxl does not load — a strict rule would flag thousands of
//!   phantom errors. Documented as a field instead.
//! - `Custom('X')` argument references (17k in loc `.yml` + `.gui`, ~22
//!   genuinely dangling) need argument-level datafunction extraction — a
//!   future loc/gui-layer feature, not a schema rule.
//!
//! `log_loc_errors` (1,028 corpus uses) is real but absent from the `.info`.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Effect, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

const CUSTOM_LOC_DIR: &str = "common/customizable_localization/";

/// A `yes`/`no` toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// One `text = { … }` entry: the first entry whose trigger passes supplies
/// the localization key.
static TEXT_ENTRY: StructSpec = StructSpec {
    name: "custom_loc text",
    fields: &[
        (
            "setup_scope",
            block(Effect).doc(
                "Run before the trigger — interface effects only (game state cannot be \
                 modified); saved scopes are visible to the trigger and the loc key.",
            ),
        ),
        (
            "trigger",
            block(Trigger).doc(
                "When this passes, this entry's `localization_key` is used. Interface \
                 triggers (e.g. window checks) are valid.",
            ),
        ),
        (
            "localization_key",
            scalar(LocKey).doc(
                "The localization key returned (scopes from `setup_scope` are accessible). \
                 May live in any language's localization, so it is not resolved as a \
                 reference.",
            ),
        ),
        (
            "fallback",
            toggle("Pick this entry when no other entry is valid."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one customizable-localization definition.
static CUSTOM_LOC: StructSpec = StructSpec {
    name: "custom_loc",
    fields: &[
        (
            "type",
            scalar(Setting)
                .doc(
                    "The scope type the custom loc is called on — must match the scope of \
                     the `Custom(…)` call site. `all` accepts any scope (but limits which \
                     triggers are safe).",
                )
                .values(&[
                    "character",
                    "landed_title",
                    "province",
                    "artifact",
                    "activity",
                    "secret",
                    "scheme",
                    "combat",
                    "combat_side",
                    "title_and_vassal_change",
                    "faith",
                    "dynasty",
                    "all",
                ]),
        ),
        (
            "text",
            block(Struct(&TEXT_ENTRY)).doc(
                "A candidate text (repeatable): the first entry whose trigger passes — or \
                 a random valid one with `random_valid` — supplies the localization key.",
            ),
        ),
        (
            "random_valid",
            toggle("Pick a random valid `text` entry instead of the first."),
        ),
        (
            "log_loc_errors",
            toggle(
                "Whether missing-localization errors are logged for this key \
                 (undocumented in the `.info`; 1k corpus uses).",
            ),
        ),
        (
            "parent",
            scalar(Setting).doc(
                "Variant form: run this other custom loc's logic, then append `suffix` to \
                 the resulting key.",
            ),
        ),
        (
            "suffix",
            scalar(Setting).doc("The suffix appended to the parent's resulting localization key."),
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
        refs: &[RefRule {
            // Depth-1 only: `parent` is a generic word other dirs use freely.
            pattern: RefPattern::KeyValueTop("parent"),
            gate: Some(CUSTOM_LOC_DIR),
            alt: &[],
        }],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(CUSTOM_LOC_DIR, ClauseKind::Struct(&CUSTOM_LOC))];
}
