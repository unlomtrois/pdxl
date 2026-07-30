//! Situations (`common/situation/`, from `_situations.info` /
//! `_catalysts.info` / `_situation_group_types.info`): the Khans-of-the-Steppe
//! situation system: region-bound objects that move through phases, with
//! characters sorted into participant groups, driven by *catalysts*.
//!
//! Three kinds live here:
//! - **situation_type** (`situations/`): the situation definitions. Referenced
//!   by the `situation:X` scope literal (713 corpus refs, 0 unresolved) and by
//!   `situation_type = X` fields (create/history effects).
//! - **catalyst** (`catalysts/`): named database entries (mostly empty bodies)
//!   that push phase transitions. Referenced by the `catalysts = { X = n }`
//!   block *keys* inside a phase's `future_phases` (gated to the situations
//!   dir) and by `catalyst = X` inside the five situation-catalyst
//!   effects/triggers. **Not** `activate_struggle_catalyst`, struggle
//!   catalysts are a separate database (`common/struggle/catalysts/`), so the
//!   `catalyst` field is matched only under the situation effects, never bare
//!   (corpus-validated: 54 gated refs, 0 unresolved; the 100 struggle-catalyst
//!   values are correctly excluded).
//! - **situation_group_type** (`situation_group_types/`): the foldable gui
//!   groupings. Referenced by `situation_group_type = X` at the top of a
//!   situation body (gated, depth 1).
//!
//! Not modeled as refs: `gui_window_name` / `gui_participation_window_name`
//! (gui widget names; the gui layer is name-gated, not enumerable),
//! `start_phase` / future-phase keys (phase keys are local to one situation,
//! no cross-file kind), `map_province_effect` and `geographical_regions`
//! (kinds not modeled yet). The deep `modifier_named_sets` payload is left
//! opaque.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Config, Effect, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, color, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{DURATION, OPAQUE, TRIGGERED_ASSET, anywhere, toggle};

const SITUATIONS_DIR: &str = "common/situation/situations/";
const CATALYSTS_DIR: &str = "common/situation/catalysts/";
const GROUP_TYPES_DIR: &str = "common/situation/situation_group_types/";

/// A reference rule gated to the situations directory (attribute keys reused
/// elsewhere in script (`icon`, `catalysts`) mean situation things only here).
const fn in_situations(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(SITUATIONS_DIR),
        alt: &[],
    }
}

// ── nested bodies ────────────────────────────────────────────────────────────

/// One geographical sub-region of a situation.
static SUB_REGION: StructSpec = StructSpec {
    name: "sub_region",
    fields: &[
        ("illustration", scalar(Setting).doc("`.dds` shown in the situation windows (`[SituationSubRegion.GetIllustration]`).")),
        ("icon", scalar(Setting).doc("`.dds` icon (`[SituationSubRegion.GetIcon]`).")),
        ("map_color", color().doc("RGB `{ r g b }` used by some map modes.")),
        ("geographical_regions", block(Config).doc("Array of pre-defined `map_data` geographical regions that make up this sub-region.")),
    ],
    fallback: Fallback::Deny,
};

/// The `sub_regions = { <key> = { … } }` container.
static SUB_REGIONS: StructSpec = StructSpec {
    name: "sub_regions",
    fields: &[],
    fallback: Fallback::Struct(&SUB_REGION),
};

/// One participant group within a situation.
static PARTICIPANT_GROUP: StructSpec = StructSpec {
    name: "participant_group",
    fields: &[
        ("icon", scalar(Setting).doc("`.dds` icon for the group.")),
        ("auto_add_rulers", toggle("Automatically consider landed rulers in a sub-region as potential participants (default `yes`).")),
        ("auto_add_landless_rulers", toggle("Automatically consider rulers with their domicile in a sub-region (default `yes`).")),
        ("map_color", color().doc("RGB `{ r g b }` used by some map modes.")),
        ("require_capital_in_sub_region", toggle("Require the participant's capital to be in the region (default `no`).")),
        ("require_domain_in_sub_region", toggle("Require part of the participant's domain to be in the region (default `no`).")),
        ("require_realm_in_sub_region", toggle("Require part of the participant's realm to be in the region (default `yes`).")),
        ("require_domicile_in_sub_region", toggle("Require the participant's domicile (if any) to be in the region (default `no`).")),
        ("is_character_valid", block(Trigger).doc("Whether a character may be added to / stay in this group. `root` = character; `scope:situation`, `scope:situation_sub_region`.")),
        ("on_join", block(Effect).doc("Effect when a character joins the group. `root` = character; `scope:situation`, `scope:situation_participant_group`, `scope:situation_sub_region`.")),
        ("on_leave", block(Effect).doc("Effect when a character leaves the group (not fired when the situation ends).")),
    ],
    fallback: Fallback::Deny,
};

/// The `participant_groups = { <key> = { … } }` container.
static PARTICIPANT_GROUPS: StructSpec = StructSpec {
    name: "participant_groups",
    fields: &[],
    fallback: Fallback::Struct(&PARTICIPANT_GROUP),
};

/// One possible future phase inside a phase's `future_phases`.
static FUTURE_PHASE: StructSpec = StructSpec {
    name: "future_phase",
    fields: &[
        (
            "takeover_type",
            scalar(Setting)
                .doc("How this phase can take over the active phase (default `none`).")
                .values(&["none", "points", "duration"]),
        ),
        ("takeover_points", scalar(Setting).doc("Catalyst points at which this phase takes over (with `takeover_type = points`; not with `takeover_duration`).")),
        ("weight", scalar(Setting).doc("Scripted value weighting selection of this phase as the next phase.")),
        ("takeover_duration", block(Struct(&DURATION)).doc("Duration the active phase must have run before takeover (with `takeover_type = duration`; not with `takeover_points`).")),
        ("catalysts", block(Config).doc("`<catalyst> = <points>`: which catalysts contribute points toward this phase taking over (catalysts from `common/situation/catalysts/`).")),
    ],
    fallback: Fallback::Deny,
};

/// The `future_phases = { <phase key> = { … } }` container.
static FUTURE_PHASES: StructSpec = StructSpec {
    name: "future_phases",
    fields: &[],
    fallback: Fallback::Struct(&FUTURE_PHASE),
};

/// One situation phase.
static PHASE: StructSpec = StructSpec {
    name: "phase",
    fields: &[
        ("parameters", block(Config).doc("Arbitrary `<name> = yes` phase parameters (checked in gui via `SituationPhaseType.HasParameter`; loc `situation_parameter_<key>`).")),
        ("on_start", block(Effect).doc("Effect when the phase starts in a sub-region. `root` = situation; `scope:situation_sub_region`.")),
        ("on_end", block(Effect).doc("Effect when the phase ends in a sub-region. `root` = situation; `scope:situation_sub_region`.")),
        ("illustration", scalar(Setting).doc("`.dds` shown in the situation windows.")),
        ("icon", scalar(Setting).doc("`.dds` icon for the phase.")),
        ("map_province_effect", scalar(Setting).doc("Map province effect applied to all provinces of the sub-region while this phase is active.")),
        ("map_province_effect_intensity", scalar(Setting).doc("Intensity 0.0–1.0 (default 1.0).")),
        ("max_duration", block(Struct(&DURATION)).doc("Maximum duration this phase runs before `max_duration_next_phase` selects a successor (scripted duration).")),
        (
            "max_duration_next_phase",
            scalar(Setting)
                .doc("How the next phase is picked when `max_duration` is met.")
                .values(&[
                    "highest_points",
                    "weighted_random_points",
                    "random_non_takeover",
                    "weighted_non_takover",
                ]),
        ),
        ("future_phases", block(Struct(&FUTURE_PHASES)).doc("Phases this phase can transition into (keyed by phase key).")),
        ("modifier_named_sets", block(Struct(&OPAQUE)).doc("Named sets of modifiers/parameters active while this phase is active (applied to participant groups; the set key is its own loc key).")),
    ],
    fallback: Fallback::Deny,
};

/// The `phases = { <phase key> = { … } }` container.
static PHASES: StructSpec = StructSpec {
    name: "phases",
    fields: &[],
    fallback: Fallback::Struct(&PHASE),
};

/// The body of one `situation_type` definition.
static SITUATION: StructSpec = StructSpec {
    name: "situation_type",
    fields: &[
        (
            "window",
            scalar(Setting)
                .doc("Which code window drives the gui (default `situation`).")
                .values(&["situation", "the_great_steppe", "silk_road", "dynastic_cycle"]),
        ),
        ("gui_window_name", scalar(Setting).doc("`.gui` widget name for the situation window (under `gui/`, default `window_situation`).")),
        ("gui_participation_window_name", scalar(Setting).doc("`.gui` widget name for the participation sub-window (default `window_situation_participation`).")),
        ("gui_tooltip_group_focused", toggle("Show phase-effect tooltips by participant group instead of by named modifier set.")),
        ("illustration", scalar(Setting).doc("`.dds` shown in the situation list window.")),
        ("icon", scalar_or_block(Setting, ClauseKind::Struct(&TRIGGERED_ASSET)).doc("Triggered icon (`trigger` + `reference`) on a Situation scope, or a plain `.dds`. `[Situation.GetIcon]`.")),
        ("situation_group_type", scalar(Setting).doc("The foldable situation group this situation appears in (default `minor`).")),
        ("sort_order", scalar(Setting).doc("Sort order within the situation group (higher first; ties by definition order; default 0).")),
        (
            "map_mode",
            scalar(Setting)
                .doc("Map mode for this situation (default `participant_groups`).")
                .values(&["participant_groups", "sub_regions"]),
        ),
        ("sub_regions", block(Struct(&SUB_REGIONS)).doc("Geographical sub-regions (1–255). Each has its own participant groups and active phase.")),
        ("participant_groups", block(Struct(&PARTICIPANT_GROUPS)).doc("Participant groups (1–255). A character belongs to the first group it is valid for, per sub-region.")),
        ("on_start", block(Effect).doc("Effect when the situation starts (after setup). `root` = situation.")),
        ("on_end", block(Effect).doc("Effect when the situation ends. `root` = situation.")),
        ("on_monthly", block(Effect).doc("Effect every month. `root` = situation.")),
        ("on_yearly", block(Effect).doc("Effect every year. `root` = situation.")),
        ("on_join", block(Effect).doc("Effect on a character as they join the situation. `root` = character.")),
        ("on_leave", block(Effect).doc("Effect on a character as they leave the situation (not fired when the situation ends). `root` = character.")),
        ("is_unique", toggle("Situation can only exist once in the world, enabling `situation:<type>` access (default `no`).")),
        ("keep_full_history", toggle("Keep full catalyst history (can grow very large; default `no`).")),
        ("migration", toggle("Enable migration AI for involved rulers using county fertility (default `no`).")),
        ("start_phase", scalar(Setting).doc("Phase key the situation starts in when none is given on creation.")),
        ("use_situation_phase_flat_icons", toggle("Whether phase icons are flat (default `yes`; mismatch looks wrong in tooltips).")),
        ("phases", block(Struct(&PHASES)).doc("The phases this situation can be in (at least one required).")),
    ],
    fallback: Fallback::Deny,
};

/// The body of one `situation_group_type` definition.
static SITUATION_GROUP_TYPE_BODY: StructSpec = StructSpec {
    name: "situation_group_type",
    fields: &[
        ("sort_order", scalar(Setting).doc("Order of situation groups in the situations view (higher first; ties by definition order; default 0).")),
        ("gui_tags", block(Config).doc("List of gui tags used to set size etc. in gui views.")),
    ],
    fallback: Fallback::Deny,
};

/// Catalyst bodies are database entries, usually empty; keep unknown keys
/// non-fatal.
static CATALYST_BODY: StructSpec = StructSpec {
    name: "catalyst",
    fields: &[],
    fallback: Fallback::Ignore,
};

pub(crate) struct Situation;

impl Entity for Situation {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::SITUATION_TYPE,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: SITUATIONS_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                anywhere(RefPattern::ScopePrefix("situation")),
                anywhere(RefPattern::KeyValue("situation_type")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::CATALYST,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: CATALYSTS_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: CATALYST_REFS,
            aliases: &[],
        },
        KindSpec {
            kind: kinds::SITUATION_GROUP_TYPE,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: GROUP_TYPES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[in_situations(RefPattern::KeyValueTop(
                "situation_group_type",
            ))],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (SITUATIONS_DIR, ClauseKind::Struct(&SITUATION)),
        (
            GROUP_TYPES_DIR,
            ClauseKind::Struct(&SITUATION_GROUP_TYPE_BODY),
        ),
        (CATALYSTS_DIR, ClauseKind::Struct(&CATALYST_BODY)),
    ];
}

/// Catalyst references: the `future_phases` `catalysts = { X = n }` block keys
/// (gated to the situations dir), plus `catalyst = X` inside each of the five
/// situation-catalyst effects/triggers (never `activate_struggle_catalyst`,
/// whose catalysts are a separate database).
const CATALYST_REFS: &[RefRule] = &[
    in_situations(RefPattern::KeyBlockKeys("catalysts")),
    anywhere(RefPattern::KeyBlockField("phase_has_catalyst", "catalyst")),
    anywhere(RefPattern::KeyBlockField(
        "situation_top_has_catalyst",
        "catalyst",
    )),
    anywhere(RefPattern::KeyBlockField(
        "trigger_situation_catalyst",
        "catalyst",
    )),
    anywhere(RefPattern::KeyBlockField(
        "situation_has_catalyst",
        "catalyst",
    )),
    anywhere(RefPattern::KeyBlockField(
        "trigger_sub_region_catalyst",
        "catalyst",
    )),
];
