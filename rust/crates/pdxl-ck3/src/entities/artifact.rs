//! Artifacts (`common/artifacts/*`) — seven interlocking concepts, one per
//! subdirectory (from the `_*.info` files): **types** (slot + feature
//! requirements), **templates** (scripted can_equip/can_benefit logic),
//! **visuals** (2d/3d assets), **features** + **feature_groups** (procedural
//! decoration), **blueprints** (reforge recipes), and **slots** (inventory /
//! court positions).
//!
//! Cross-references (all corpus-validated at 0 unresolved):
//! - `create_artifact = { type/visuals/template = X }` — the builtin effect's
//!   direct-child fields (the nested `history = { type = … }` is a *different*
//!   `type`; `KeyBlockField` only matches direct children, so it's safe).
//! - blueprints: `in_type`/`out_type` → types, `in_visuals`/`out_visuals` →
//!   visuals, `template` → templates (all gated to the blueprints dir).
//! - types: `default_visuals` → visuals, `required_features` /
//!   `optional_features` list items → feature groups.
//! - features: `group` → feature groups; visuals: `default_type` → types.
//!
//! Deliberately **not** referenced: `slot = X` in types names a slot *type
//! attribute* (helmet/regalia/…), not the slot keys (crown/trinket_1/…) — no
//! resolvable def set; and bare `feature = X` is overloaded by DLC checks
//! (`feature = royal_court`). Slots and features stay def-only.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, ScriptValue, StaticModifier, Struct, Trigger,
};
use pdxl_analysis::context::ScalarKind::{Setting, Target};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{OPAQUE, TRIGGERED_ASSET, anywhere};

/// A scalar reference gated to one `common/artifacts/<sub>/` directory.
const fn in_dir(dir: &'static str, pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(dir),
    }
}

const TYPES_DIR: &str = "common/artifacts/types/";
const TEMPLATES_DIR: &str = "common/artifacts/templates/";
const VISUALS_DIR: &str = "common/artifacts/visuals/";
const FEATURES_DIR: &str = "common/artifacts/features/";
const FEATURE_GROUPS_DIR: &str = "common/artifacts/feature_groups/";
const BLUEPRINTS_DIR: &str = "common/artifacts/blueprints/";
const SLOTS_DIR: &str = "common/artifacts/slots/";

/// Top-level `NAME = { … }` definitions in one artifacts subdirectory.
const fn defs(dir: &'static str) -> Option<DefSource> {
    Some(DefSource {
        dir_prefix: dir,
        shape: DefShape::TopLevel,
    })
}

// ── structural bodies (from the `_*.info` files) ────────────────────────────

/// The slot types vanilla declares in `common/artifacts/slots/` (`type = "…"`).
/// Suggestions only — mods add their own (T4N: `throne_corona`).
const SLOT_TYPES: &[&str] = &[
    "helmet",
    "regalia",
    "armor",
    "primary_armament",
    "miscellaneous",
    "wall_big",
    "wall_small",
    "throne",
    "sculpture",
    "book",
    "pedestal",
    "journal",
];

/// The body of one artifact **type** (`_types.info`).
static ARTIFACT_TYPE: StructSpec = StructSpec {
    name: "artifact_type",
    fields: &[
        (
            "slot",
            scalar(Setting)
                .doc(
                    "The inventory slot *type* this artifact occupies (e.g. `helmet`, `wall_big`).",
                )
                .values(SLOT_TYPES),
        ),
        (
            "required_features",
            block(Struct(&OPAQUE)).doc(
                "Feature groups automatically assigned on creation — one feature from each \
                 listed group.",
            ),
        ),
        (
            "optional_features",
            block(Struct(&OPAQUE))
                .doc("Feature groups that can be added after creation in script."),
        ),
        (
            "default_visuals",
            scalar(Setting).doc(
                "Optional, no gameplay effect — only used for automatic test artifact \
                 generation.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one artifact **template** (`_templates.info`). Root scope is
/// character for all triggers; the artifact is `scope:artifact`.
static ARTIFACT_TEMPLATE: StructSpec = StructSpec {
    name: "artifact_template",
    fields: &[
        (
            "can_equip",
            block(Trigger).doc("Can this character equip this artifact?"),
        ),
        (
            "can_benefit",
            block(Trigger)
                .doc("Can this character benefit from the full modifiers of the artifact?"),
        ),
        (
            "can_reforge",
            block(Trigger).doc("Can this character reforge this artifact (turn it into another)?"),
        ),
        (
            "can_repair",
            block(Trigger).doc("Can this character repair this artifact (restore durability)?"),
        ),
        (
            "fallback",
            block(StaticModifier)
                .doc("Applied instead of the artifact's modifiers when `can_benefit` fails."),
        ),
        (
            "ai_score",
            block(ScriptValue).doc(
                "Added to the AI equipping score (`can_benefit` takes precedence; see also \
                 `artifact_ai_will_equip_score` in `common/script_values/`).",
            ),
        ),
        (
            "unique",
            scalar(Setting).doc("Artifacts with this template show as unique (default `no`)."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one artifact **feature** (`_features.info`). Trigger/weight
/// scopes are passed in from `create_artifact` and similar.
static ARTIFACT_FEATURE: StructSpec = StructSpec {
    name: "artifact_feature",
    fields: &[
        (
            "group",
            scalar(Setting).doc("The feature group this feature belongs to."),
        ),
        (
            "trigger",
            block(Trigger).doc("Scopes are passed in from `create_artifact` and similar."),
        ),
        (
            "weight",
            scalar_or_block(Setting, ScriptValue)
                .doc("Selection weight (same scopes as `trigger`)."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `replacement_modifiers = { common/masterwork/famed/illustrious = { … } }` —
/// per-rarity lists of static-modifier names (loose scalars).
static REPLACEMENT_MODIFIERS: StructSpec = StructSpec {
    name: "replacement_modifiers",
    fields: &[
        ("common", block(Struct(&OPAQUE))),
        ("masterwork", block(Struct(&OPAQUE))),
        ("famed", block(Struct(&OPAQUE))),
        ("illustrious", block(Struct(&OPAQUE))),
    ],
    fallback: Fallback::Deny,
};

/// The body of one **blueprint** (`_blueprints.info`) — a reforge recipe.
static ARTIFACT_BLUEPRINT: StructSpec = StructSpec {
    name: "artifact_blueprint",
    fields: &[
        (
            "in_type",
            scalar(Setting)
                .doc("The artifact type required to use this blueprint (slot + category)."),
        ),
        (
            "in_visuals",
            scalar(Setting).doc("The artifact visual type required to use this blueprint."),
        ),
        (
            "out_type",
            scalar(Setting).doc("The artifact type after the reforge."),
        ),
        (
            "out_visuals",
            scalar(Setting).doc("The artifact visual type after the reforge."),
        ),
        (
            "disallowed_modifiers",
            block(Struct(&OPAQUE))
                .doc("Modifier types not allowed to persist on the artifact post-reforge."),
        ),
        (
            "replacement_modifiers",
            block(Struct(&REPLACEMENT_MODIFIERS)).doc(
                "Per-rarity static modifiers used instead of any disallowed ones — a random \
                 pick (no duplicates) from the matching rarity list.",
            ),
        ),
        (
            "template",
            scalar(Setting)
                .doc("Change the artifact's template to this scripted template post-reforge."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one **visuals** entry (`_visuals.info`). Trigger scopes: `root`
/// = the owner, `scope:artifact` = the artifact, `scope:artifact.creator`.
static ARTIFACT_VISUAL: StructSpec = StructSpec {
    name: "artifact_visual",
    fields: &[
        (
            "icon",
            scalar_or_block(Setting, Struct(&TRIGGERED_ASSET)).doc(
                "The 2d icon (`.dds` name), or a `trigger` + `reference` block picked when the \
                 trigger passes.",
            ),
        ),
        (
            "asset",
            scalar_or_block(Setting, Struct(&TRIGGERED_ASSET)).doc(
                "The 3d asset name, or a `trigger` + `reference` block picked when the trigger \
                 passes.",
            ),
        ),
        (
            "default_type",
            scalar(Setting).doc(
                "Optional, no gameplay effect — only used for automatic test artifact \
                 generation.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one inventory **slot** (`slots/00_default.txt`).
static ARTIFACT_SLOT: StructSpec = StructSpec {
    name: "artifact_slot",
    fields: &[
        (
            "type",
            scalar(Setting)
                .doc("The slot type artifact types refer to via `slot =`.")
                .values(SLOT_TYPES),
        ),
        (
            "category",
            scalar(Setting)
                .doc("`inventory` (character) or `court` (royal court furniture).")
                .values(&["inventory", "court"]),
        ),
        ("icon", scalar(Setting).doc("Optional icon override.")),
    ],
    fallback: Fallback::Deny,
};

/// A feature group has no body: `key = {}`.
static ARTIFACT_FEATURE_GROUP: StructSpec = StructSpec {
    name: "artifact_feature_group",
    fields: &[],
    fallback: Fallback::Deny,
};

// ── the `create_artifact = { … }` builtin-effect struct (`effects.log`) ─────

/// `history = { type = … }` — a custom artifact history entry.
static ARTIFACT_HISTORY: StructSpec = StructSpec {
    name: "artifact_history",
    fields: &[
        (
            "type",
            scalar(Setting)
                .doc("The history entry type (the full list from `effects.log`).")
                .values(&[
                    "created_before_history",
                    "created",
                    "prize_created",
                    "discovered",
                    "creator_discovered",
                    "claimed_by_house",
                    "given",
                    "stolen",
                    "inherited",
                    "conquest",
                    "taken_in_siege",
                    "taken_in_battle",
                    "won_in_duel",
                    "purchased",
                    "prize_awarded",
                    "ransomed",
                    "reforged",
                ]),
        ),
        ("date", scalar(Setting).doc("When this event took place.")),
        (
            "actor",
            scalar(Target).doc("Who acted (e.g. who created it)."),
        ),
        (
            "recipient",
            scalar(Target).doc("Who received (e.g. whom it was given to)."),
        ),
        (
            "location",
            scalar(Target).doc("Where the event took place (a province)."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of `create_artifact = { … }` (registered as an effect struct in
/// [`crate::contexts`]). Current scopes are implicitly available to the
/// visuals triggers.
pub(crate) static CREATE_ARTIFACT: StructSpec = StructSpec {
    name: "create_artifact",
    fields: &[
        (
            "name",
            scalar_or_block(Setting, DynamicDesc).doc("The artifact name (dynamic description)."),
        ),
        (
            "description",
            scalar_or_block(Setting, DynamicDesc)
                .doc("The artifact description (dynamic description)."),
        ),
        (
            "rarity",
            scalar(Setting)
                .doc("`common` / `masterwork` / `famed` / `illustrious` (artifact rarity).")
                .values(&["common", "masterwork", "famed", "illustrious"]),
        ),
        (
            "type",
            scalar(Setting).doc("The artifact type (`common/artifacts/types/`) — fixes the slot."),
        ),
        (
            "modifier",
            scalar(Setting).doc("A static modifier applied to the wielding character."),
        ),
        (
            "template",
            scalar(Setting).doc(
                "A scripted base template (`common/artifacts/templates/`) with triggers and \
                 modifiers.",
            ),
        ),
        (
            "visuals",
            scalar(Setting)
                .doc("The artifact visual type (`common/artifacts/visuals/`) — 2d/3d assets."),
        ),
        (
            "visuals_source",
            scalar(Target).doc(
                "A scope containing a landed title, dynasty or house — coat-of-arms source for \
                 the artifact graphics (banners, mostly).",
            ),
        ),
        (
            "durability",
            scalar_or_block(Setting, ScriptValue).doc("New durability (max by default)."),
        ),
        (
            "max_durability",
            scalar_or_block(Setting, ScriptValue)
                .doc("Optional max-durability override (defines-assigned otherwise)."),
        ),
        (
            "decaying",
            scalar(Setting).doc("Does the artifact decay with time? (`yes` by default)."),
        ),
        (
            "history",
            block(Struct(&ARTIFACT_HISTORY)).doc(
                "A custom history entry (e.g. this artifact was reforged by someone other than \
                 the owner).",
            ),
        ),
        (
            "generate_history",
            scalar(Setting)
                .doc("Automatically generate a new history entry if none has been scripted?"),
        ),
        (
            "quality",
            scalar_or_block(Setting, ScriptValue).doc("New quality, used in AI scoring."),
        ),
        (
            "wealth",
            scalar_or_block(Setting, ScriptValue).doc("New wealth, used in AI scoring."),
        ),
        (
            "creator",
            scalar(Target).doc("A custom creator of the artifact (default: the owner)."),
        ),
        (
            "save_scope_as",
            scalar(Setting).doc("Save a reference to the newly created artifact."),
        ),
        (
            "title_history",
            scalar(Target).doc("Add the given title's history entries to the artifact history."),
        ),
        (
            "title_history_date",
            scalar(Setting).doc("From which date onwards to copy the title's history entries."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Artifact;

impl Entity for Artifact {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::ARTIFACT_TYPE,
            icon: IconHint::Object,
            defs: defs(TYPES_DIR),
            refs: &[
                anywhere(RefPattern::KeyBlockField("create_artifact", "type")),
                in_dir(BLUEPRINTS_DIR, RefPattern::KeyValue("in_type")),
                in_dir(BLUEPRINTS_DIR, RefPattern::KeyValue("out_type")),
                in_dir(VISUALS_DIR, RefPattern::KeyValue("default_type")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ARTIFACT_TEMPLATE,
            icon: IconHint::Function,
            defs: defs(TEMPLATES_DIR),
            refs: &[
                anywhere(RefPattern::KeyBlockField("create_artifact", "template")),
                in_dir(BLUEPRINTS_DIR, RefPattern::KeyValue("template")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ARTIFACT_VISUAL,
            icon: IconHint::Object,
            defs: defs(VISUALS_DIR),
            refs: &[
                anywhere(RefPattern::KeyBlockField("create_artifact", "visuals")),
                in_dir(BLUEPRINTS_DIR, RefPattern::KeyValue("in_visuals")),
                in_dir(BLUEPRINTS_DIR, RefPattern::KeyValue("out_visuals")),
                in_dir(TYPES_DIR, RefPattern::KeyValue("default_visuals")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ARTIFACT_FEATURE,
            icon: IconHint::Tag,
            defs: defs(FEATURES_DIR),
            // No refs: bare `feature = X` is overloaded by DLC checks
            // (`feature = royal_court`) — def-only until a clean shape exists.
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ARTIFACT_FEATURE_GROUP,
            icon: IconHint::Tag,
            defs: defs(FEATURE_GROUPS_DIR),
            refs: &[
                in_dir(FEATURES_DIR, RefPattern::KeyValue("group")),
                in_dir(TYPES_DIR, RefPattern::KeyList("required_features")),
                in_dir(TYPES_DIR, RefPattern::KeyList("optional_features")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ARTIFACT_BLUEPRINT,
            icon: IconHint::Action,
            defs: defs(BLUEPRINTS_DIR),
            // No refs: nothing in the corpus names blueprints (the reforge
            // window matches them by in_type/in_visuals).
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::ARTIFACT_SLOT,
            icon: IconHint::Object,
            defs: defs(SLOTS_DIR),
            // `slot = X` in types names the slot *type* attribute, not these
            // slot keys — but death reasons name a slot key directly.
            refs: &[in_dir(
                "common/deathreasons/",
                RefPattern::KeyValue("use_equipped_artifact_in_slot"),
            )],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (TYPES_DIR, ClauseKind::Struct(&ARTIFACT_TYPE)),
        (TEMPLATES_DIR, ClauseKind::Struct(&ARTIFACT_TEMPLATE)),
        (VISUALS_DIR, ClauseKind::Struct(&ARTIFACT_VISUAL)),
        (FEATURES_DIR, ClauseKind::Struct(&ARTIFACT_FEATURE)),
        (
            FEATURE_GROUPS_DIR,
            ClauseKind::Struct(&ARTIFACT_FEATURE_GROUP),
        ),
        (BLUEPRINTS_DIR, ClauseKind::Struct(&ARTIFACT_BLUEPRINT)),
        (SLOTS_DIR, ClauseKind::Struct(&ARTIFACT_SLOT)),
    ];
}
