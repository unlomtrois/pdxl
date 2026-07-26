//! Doctrines (`common/religion/doctrine_types/`) — top-level definitions
//! (183 in vanilla), referenced everywhere doctrines are named. All rules
//! corpus-validated at 0 unresolved, ungated (no colliding meanings):
//!
//! - `doctrine = X` — religions, faiths, and `doctrine_selection_pair`s
//!   (2027 refs);
//! - `fallback_doctrine = X` — selection pairs (15);
//! - `hostility_doctrine = X` — religion families (4);
//! - `has_doctrine = X` — the trigger, script-wide (2397; the only misses
//!   are `$MACRO$` names, skipped by the engine);
//! - `doctrine_types = { X … }` — the group listing its doctrines, gated to
//!   `doctrine_group_types/` (all values resolve).
//!
//! **Doctrine groups** (`doctrine_group_types/`, 49 defs) live here too: they
//! are the same domain, and the only reference they carry points *out* at
//! doctrines. Nothing in script names a group — the one near-miss,
//! `save_scope_value_as = { name = doctrine_adultery_women }`, is a scope name
//! that happens to match a group key, so no rule keys off `name`.
//!
//! The doctrine body was corpus-derived before `_doctrine_types.info` existed;
//! it now agrees with that readme, which adds only `doctrine_character_modifier`
//! (documented, unused in this corpus). `clergy_modifier` runs the other way —
//! used but undocumented, so marked *(corpus)*.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, ScriptValue, StaticModifier, Struct, Trigger,
};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{OPAQUE, anywhere};
use super::faith::RELIGION_TRAITS;

const DOCTRINES_DIR: &str = "common/religion/doctrine_types/";
const DOCTRINE_GROUPS_DIR: &str = "common/religion/doctrine_group_types/";

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
            "doctrine_character_modifier",
            block(StaticModifier).doc(
                "Applied to characters of the faith only when the faith also has the \
                 doctrine named inside by `doctrine = …`.",
            ),
        ),
        (
            "clergy_modifier",
            block(StaticModifier).doc("Modifier applied to the faith's clergy. *(corpus)*"),
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

/// The body of one doctrine group (`_doctrine_group_types.info`). The key also
/// picks the icon, via `FAITH_DOCTRINE_ICON_PATH`, and the `<key>_name` loc.
static DOCTRINE_GROUP: StructSpec = StructSpec {
    name: "doctrine group",
    fields: &[
        (
            "category",
            scalar(Setting).doc("How the group is categorized in the UI."),
        ),
        (
            "number_of_picks",
            scalar(Setting)
                .doc("How many unique doctrines of the group the player picks. Default 1."),
        ),
        (
            "is_available_on_create",
            block(Trigger).doc(
                "Whether the group is offered when choosing doctrines. A group that is not \
                 shown is dropped entirely on create, even if the old faith had it. Root is \
                 the faith.",
            ),
        ),
        (
            "doctrine_types",
            block(Struct(&OPAQUE)).doc("The doctrines belonging to this group."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Doctrine;

impl Entity for Doctrine {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
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
                // Group → doctrine. Gated: bare `doctrine_types` would be a
                // reasonable key name anywhere.
                RefRule {
                    pattern: RefPattern::KeyList("doctrine_types"),
                    gate: Some(DOCTRINE_GROUPS_DIR),
                    alt: &[],
                },
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::DOCTRINE_GROUP,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: DOCTRINE_GROUPS_DIR,
                shape: DefShape::TopLevel,
            }),
            // Nothing in script names a group; see the module doc.
            refs: &[],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (DOCTRINES_DIR, ClauseKind::Struct(&DOCTRINE)),
        (DOCTRINE_GROUPS_DIR, ClauseKind::Struct(&DOCTRINE_GROUP)),
    ];
}
