//! Holdings buildings (`common/buildings/`, from `_buildings.info`) —
//! top-level `NAME = { … }` definitions with a large documented body:
//! military stats, construction triggers (province scope), ~24 conditional
//! modifier collections, assets, AI weighting, and lifecycle effects.
//!
//! References (all corpus-validated at 0 unresolved; macro-concatenated
//! values like `outposts_$TIER$` are skipped by the engine automatically):
//! - `next_building = X` (upgrade chains, 777 refs) — anywhere;
//! - `has_building = X` / `has_building_or_higher = X` triggers and
//!   `add_building = X` effect — anywhere;
//! - `unlock_building = X` in culture innovations/eras — gated to
//!   `common/culture/` (the ref the culture domain had to drop before this
//!   kind existed);
//! - province history: `special_building` / `special_building_slot` scalars
//!   and `buildings = { X Y … }` lists — gated to `history/provinces/`.
//!
//! Not modeled: `holding` / `terrain` / `county_holder_dynasty_perk` /
//! `great_project_type` values (their target databases are not in the schema
//! yet), and building `flag`s (checked via `has_building_with_flag`, a flag
//! namespace rather than building names).

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, Effect, ScriptedModifier, StaticModifier, Struct, Trigger,
};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{
    Fallback, FieldSpec, StructSpec, block, block_scoped, scalar, scalar_or_block,
};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{COST, anywhere};

const BUILDINGS_DIR: &str = "common/buildings/";
const PROVINCE_HISTORY_DIR: &str = "history/provinces/";

/// A `yes`/`no` toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// A conditional static-modifier collection entry (`parameter`-gated bodies
/// carry the gate key alongside modifier tags — StaticModifier context is a
/// hint, never diagnosed).
const fn mods(doc: &'static str) -> FieldSpec {
    block(StaticModifier).doc(doc)
}

/// The ~24 modifier collections a building can carry — shared verbatim by the
/// building body, `fallback`, and `override_modifiers` (the `.info` defines
/// one list for all three).
macro_rules! modifier_collections {
    ($(($k:literal, $f:expr $(,)?)),* $(,)?) => {
        &[
            $(($k, $f),)*
            ("character_modifier", mods("Applied to the owner of the holding.")),
            ("character_culture_modifier", mods("Applied to the holder if their culture has `parameter`.")),
            ("character_faith_modifier", mods("Applied to the holder if their faith has `parameter`.")),
            ("character_dynasty_modifier", mods("Applied to the holder if the county holder's dynasty has `county_holder_dynasty_perk`.")),
            ("characer_dynasty_modifier", mods("Misspelled twin of `character_dynasty_modifier` (as in the `.info`).")),
            ("character_government_modifier", mods("Applied to the county holder if their government has `parameter`.")),
            ("character_situation_modifier", mods("Applied to the holder if a situation the county is in has `parameter` in its current phase.")),
            ("province_modifier", mods("Applied to the province.")),
            ("province_culture_modifier", mods("Applied to the province if the county culture has `parameter`.")),
            ("province_faith_modifier", mods("Applied to the province if the county faith has `parameter`.")),
            ("province_terrain_modifier", mods("Applied to the province matching `parameter`/`terrain`/`is_coastal`/`is_riverside`.")),
            ("province_dynasty_modifier", mods("Applied to the province if the county holder's dynasty has `county_holder_dynasty_perk`.")),
            ("province_government_modifier", mods("Applied to the province if the county holder's government has `parameter`.")),
            ("province_situation_modifier", mods("Applied to the province if a situation the county is in has `parameter` in its current phase.")),
            ("county_modifier", mods("Applied to the entire county (stacks across its provinces).")),
            ("county_culture_modifier", mods("Applied to the county if its culture has `parameter`.")),
            ("county_faith_modifier", mods("Applied to the county if its faith has `parameter`.")),
            ("county_dynasty_modifier", mods("Applied to the county if the county holder's dynasty has `county_holder_dynasty_perk`.")),
            ("county_holding_modifier", mods("Applied to every holding of the `holding` type in the county.")),
            ("county_holder_character_modifier", mods("Applied to the county holder.")),
            ("county_situation_modifier", mods("Applied to the county if a situation it is in has `parameter` in its current phase.")),
            ("duchy_capital_county_modifier", mods("Applied to every de jure county in the duchy (duchy capital buildings only).")),
            ("duchy_capital_county_culture_modifier", mods("Duchy-wide county modifier gated on county culture `parameter` (duchy capital buildings only).")),
            ("duchy_capital_county_faith_modifier", mods("Duchy-wide county modifier gated on county faith `parameter` (duchy capital buildings only).")),
            ("duchy_capital_county_situation_modifier", mods("Duchy-wide county modifier gated on a situation-phase `parameter` (duchy capital buildings only).")),
        ]
    };
}

/// `fallback = { … }` — modifiers applied while the building is disabled or
/// ruined (requires `is_enabled`).
static FALLBACK_MODS: StructSpec = StructSpec {
    name: "building fallback",
    fields: modifier_collections!(),
    fallback: Fallback::Deny,
};

/// `override_modifiers = { requires_dlc_flag = X … }` — alternative modifier
/// set when a DLC feature flag is available (first valid collection wins).
static OVERRIDE_MODS: StructSpec = StructSpec {
    name: "building override_modifiers",
    fields: modifier_collections!((
        "requires_dlc_flag",
        scalar(Setting).doc("The DLC feature flag this collection requires."),
    ),),
    fallback: Fallback::Deny,
};

/// One `asset = { … }` entry.
static ASSET: StructSpec = StructSpec {
    name: "building asset",
    fields: &[
        (
            "type",
            scalar(Setting)
                .doc("Mesh or entity (meshes are more performant — prefer them).")
                .values(&["pdxmesh", "entity"]),
        ),
        (
            "name",
            scalar(Setting)
                .doc("Mesh/entity name (repeatable; combined with `names`, all same type)."),
        ),
        (
            "names",
            block(Struct(&super::common::OPAQUE))
                .doc("Mesh/entity names to randomize between (combined with `name`)."),
        ),
        (
            "illustration",
            scalar(Setting).doc(
                "County-view illustration path; accessible in gui via \
                 `[Holding.GetIllustration]`.",
            ),
        ),
        (
            "soundeffect",
            scalar_or_block(Setting, Struct(&super::common::OPAQUE)).doc(
                "Ambient sound: `soundeffect = \"event:…\"`, or a block with `soundeffect` + \
                 `soundparameter` (repeatable).",
            ),
        ),
        (
            "governments",
            block(Struct(&super::common::OPAQUE)).doc("Governments that prefer this asset."),
        ),
        (
            "provinces",
            block(Struct(&super::common::OPAQUE))
                .doc("Province IDs preferring this asset (higher priority than regions)."),
        ),
        (
            "graphical_regions",
            block(Struct(&super::common::OPAQUE)).doc(
                "Geographical-region names preferring this asset (top criterion after \
                 government and province).",
            ),
        ),
        (
            "graphical_cultures",
            block(Struct(&super::common::OPAQUE))
                .doc("`building_gfx` flags from the culture database preferring this asset."),
        ),
        (
            "graphical_faiths",
            block(Struct(&super::common::OPAQUE)).doc(
                "`graphical_faith` values from the religion database (faith > religion > \
                 family priority).",
            ),
        ),
        (
            "requires_dlc_flag",
            scalar(Setting).doc("The asset requires this DLC to be enabled."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `assets = { asset = { … } … }` — optional grouping wrapper.
static ASSETS: StructSpec = StructSpec {
    name: "building assets",
    fields: &[(
        "asset",
        block(Struct(&ASSET)).doc("One asset candidate (repeatable)."),
    )],
    fallback: Fallback::Deny,
};

/// A province-scoped construction/validity trigger.
const fn ptrigger(doc: &'static str) -> FieldSpec {
    block_scoped(Trigger, "province").doc(doc)
}

/// A construction lifecycle effect (province scope; `scope:character` paid,
/// `scope:holding` when this is a holding's primary building).
const fn peffect(doc: &'static str) -> FieldSpec {
    block_scoped(Effect, "province").doc(doc)
}

/// The body of one building definition (`_buildings.info`).
static BUILDING: StructSpec = StructSpec {
    name: "building",
    fields: modifier_collections!(
        ("levy", scalar(Setting).doc("Levies the building gives (int or named value; default 0).")),
        ("max_garrison", scalar(Setting).doc("Garrison the building gives (int or named value; default 0).")),
        ("garrison_reinforcement_factor", scalar(Setting).doc("Monthly garrison refill as a fraction of max garrison, 0–1 (default 0).")),
        ("construction_time", scalar(Setting).doc("Days to construct (int or named value; default 0).")),
        (
            "type",
            scalar(Setting)
                .doc("Regular building, special building, or duchy capital building (regular by default).")
                .values(&["regular", "special", "duchy_capital"]),
        ),
        ("asset", block(Struct(&ASSET)).doc("One asset candidate (repeatable; the `assets` wrapper is optional).")),
        ("assets", block(Struct(&ASSETS)).doc("Optional grouping wrapper around `asset` entries (exists for editor folding).")),
        ("is_enabled", ptrigger("Is the building enabled? Otherwise no effects and not constructible. Scopes: root = province, scope:holder, scope:county.")),
        ("can_rebuild", ptrigger("Can the (great) building be repaired after being ruined?")),
        ("can_construct_potential", ptrigger("Whether the building appears in the build menu at all (always evaluated together with is_enabled).")),
        ("can_construct_showing_failures_only", ptrigger("Construction trigger showing only failures — use for temporary obstacles the player can overcome.")),
        ("can_construct", ptrigger("Construction trigger showing both met and missing requirements.")),
        ("show_disabled", toggle("Show the building in the build menu even when disabled (still uses can_construct_potential).")),
        ("cost", block(ClauseKind::Struct(&COST)).doc("Construction cost (scripted cost: gold/piety/prestige).")),
        ("next_building", scalar(Setting).doc("The next upgrade in this building chain.")),
        ("effect_desc", scalar_or_block(Setting, DynamicDesc).doc("Custom description for effects indirectly provided (dynamic description; no scope).")),
        ("fallback", block(Struct(&FALLBACK_MODS)).doc("Alternative modifiers applied while the building is disabled or ruined (requires is_enabled).")),
        ("override_modifiers", block(Struct(&OVERRIDE_MODS)).doc("Alternative modifier set when a DLC feature flag is available (repeatable; first valid wins; fallback still applies when disabled).")),
        ("type_icon", scalar(Setting).doc("Icon filename in the BUILDING_TYPE_ICON_PATH folder.")),
        ("flag", scalar(Setting).doc("A building flag checkable in triggers (repeatable).")),
        ("ai_value", block(ScriptedModifier).doc("AI desirability (MTTH): base + weighted modifiers. Buildings within 20% of the top score are candidates; one is picked at random. Scopes: root = province, scope:character (payer), scope:holding.")),
        ("is_graphical_background", toggle("Used only to pick the background map asset (walls etc.); the AI skips such buildings.")),
        ("on_start", peffect("Effect when construction starts.")),
        ("on_cancelled", peffect("Effect when construction is cancelled.")),
        ("on_complete", peffect("Effect when construction finishes.")),
        ("great_project_type", scalar(Setting).doc("The great-project type that upgrades this great building (progress lives on the project, not the slot).")),
    ),
    fallback: Fallback::Deny,
};

pub(crate) struct Building;

impl Entity for Building {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::BUILDING,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: BUILDINGS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("next_building")),
            anywhere(RefPattern::KeyValue("has_building")),
            anywhere(RefPattern::KeyValue("has_building_or_higher")),
            anywhere(RefPattern::KeyValue("add_building")),
            // Culture innovations/eras `unlock_building` (tooltip-only unlock
            // list) — the ref the culture domain dropped before this kind
            // existed. Gated: `unlock_building` is unambiguous there.
            RefRule {
                pattern: RefPattern::KeyValue("unlock_building"),
                gate: Some("common/culture/"),
                alt: &[],
            },
            // Holding types name the building raised on creation, plus the
            // first level of everything else constructible there. Gated: both
            // keys mean other things elsewhere (`buildings` is a province
            // history list, handled below).
            RefRule {
                pattern: RefPattern::KeyValue("primary_building"),
                gate: Some(super::holding::HOLDINGS_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyList("buildings"),
                gate: Some(super::holding::HOLDINGS_DIR),
                alt: &[],
            },
            // Province history: preplaced special buildings and slots, and
            // the `buildings = { X Y … }` list.
            RefRule {
                pattern: RefPattern::KeyValue("special_building"),
                gate: Some(PROVINCE_HISTORY_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("special_building_slot"),
                gate: Some(PROVINCE_HISTORY_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyList("buildings"),
                gate: Some(PROVINCE_HISTORY_DIR),
                alt: &[],
            },
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(BUILDINGS_DIR, ClauseKind::Struct(&BUILDING))];
}
