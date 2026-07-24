//! Doctrines (`common/religion/doctrine_types/`) — top-level definitions
//! (183 in vanilla), referenced everywhere doctrines are named. All rules
//! corpus-validated at 0 unresolved, ungated (no colliding meanings):
//!
//! - `doctrine = X` — religions, faiths, and `doctrine_selection_pair`s
//!   (2027 refs);
//! - `fallback_doctrine = X` — selection pairs (15);
//! - `hostility_doctrine = X` — religion families (4);
//! - `has_doctrine = X` — the trigger, script-wide (2397; the only misses
//!   are `$MACRO$` names, skipped by the engine).
//!
//! No `_*.info` documents this directory; the body below is corpus-derived.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, ScriptValue, StaticModifier, Struct, Trigger,
};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{OPAQUE, anywhere};
use super::faith::RELIGION_TRAITS;

const DOCTRINES_DIR: &str = "common/religion/doctrine_types/";

/// The body of one doctrine (corpus-derived; no `.info` exists).
static DOCTRINE: StructSpec = StructSpec {
    name: "doctrine",
    fields: &[
        (
            "name",
            scalar_or_block(LocKey, DynamicDesc).doc("Override the doctrine's display name."),
        ),
        (
            "desc",
            scalar_or_block(LocKey, DynamicDesc).doc("Override the doctrine's description."),
        ),
        ("icon", scalar(Setting).doc("The doctrine icon.")),
        (
            "visible",
            scalar_or_block(Setting, Trigger)
                .doc("Whether the doctrine is visible in the faith view."),
        ),
        (
            "is_shown",
            block(Trigger).doc("Trigger: whether the doctrine is shown (faith scope)."),
        ),
        (
            "can_pick",
            block(Trigger)
                .doc("Trigger: whether the doctrine can be picked at faith creation/reformation."),
        ),
        (
            "piety_cost",
            scalar_or_block(Setting, ScriptValue)
                .doc("The piety cost to pick this doctrine (script value)."),
        ),
        (
            "character_modifier",
            block(StaticModifier).doc("Modifier applied to all characters of the faith."),
        ),
        (
            "clergy_modifier",
            block(StaticModifier).doc("Modifier applied to the faith's clergy."),
        ),
        (
            "parameters",
            block(Struct(&OPAQUE))
                .doc("Arbitrary parameters checked by script (`parameter = yes` entries)."),
        ),
        (
            "traits",
            block(Struct(&RELIGION_TRAITS)).doc("Virtues and sins granted by this doctrine."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Doctrine;

impl Entity for Doctrine {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::DOCTRINE,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: DOCTRINES_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("doctrine")),
            anywhere(RefPattern::KeyValue("fallback_doctrine")),
            anywhere(RefPattern::KeyValue("hostility_doctrine")),
            anywhere(RefPattern::KeyValue("has_doctrine")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(DOCTRINES_DIR, ClauseKind::Struct(&DOCTRINE))];
}
