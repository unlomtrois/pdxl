//! Terrain types (`common/terrain_types/`, from `_terrains.info`) — top-level
//! definitions referenced by `terrain = X` everywhere: the province-history
//! terrain override, the `terrain` trigger (bare and inside
//! `county_has_province_with_terrain = { terrain = X }`), activity and travel
//! script.
//!
//! The ungated `terrain` rule is corpus-validated: every scalar value in
//! vanilla resolves except 24 × `terrain = mountain` in
//! `history/provinces/k_maharastra.txt` — a genuine vanilla typo for
//! `mountains`, which the diagnostic correctly flags. Macro values
//! (`$TERRAIN$`) and block forms are skipped by the engine for free.
//!
//! Each terrain also auto-generates `KEY_*` modifiers
//! (`hills_advantage`, `plains_supply_limit_mult`, …) documented in the
//! `.info`; those live in the generated modifier tables, not here.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, StaticModifier, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, color, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::anywhere;

const TERRAIN_DIR: &str = "common/terrain_types/";

/// `attacker_combat_effects` / `defender_combat_effects` — a combat-effect
/// payload (`common/combat_effects/_combat_effects.info`).
static COMBAT_EFFECT: StructSpec = StructSpec {
    name: "combat effect",
    fields: &[
        ("name", scalar(Setting).doc("The combat-effect key.")),
        ("image", scalar(Setting).doc("The icon shown in combat.")),
        (
            "advantage",
            scalar(Setting).doc("Advantage granted by this effect."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one terrain type (`_terrains.info`).
static TERRAIN: StructSpec = StructSpec {
    name: "terrain type",
    fields: &[
        (
            "movement_speed",
            scalar(Setting).doc("Speed on this type of terrain."),
        ),
        (
            "combat_width",
            scalar(Setting).doc("Multiplier on the combat width."),
        ),
        (
            "audio_parameter",
            scalar(Setting).doc("Used to check the audio to play."),
        ),
        (
            "color",
            color().doc("Terrain color for the terrain-type map mode."),
        ),
        (
            "travel_danger_color",
            color().doc(
                "Terrain color for the travel-planner map mode if the danger score is \
                 higher than the player's safety.",
            ),
        ),
        (
            "travel_danger_score",
            scalar(Setting)
                .doc("The amount of danger this terrain provides when travelling over it."),
        ),
        (
            "provision_cost",
            scalar(Setting)
                .doc("The provision cost for this terrain type when moving your domicile."),
        ),
        (
            "county_fertility",
            scalar(Setting)
                .doc("The Fertility contributed by this terrain type (Base County Fertility)."),
        ),
        (
            "entity",
            scalar(Setting).doc("Environmental graphical asset shown in this terrain."),
        ),
        (
            "province_modifier",
            block(StaticModifier).doc("Modifier applied to the province."),
        ),
        (
            "county_capital_modifier",
            block(StaticModifier)
                .doc("Modifier applied to the province if it is the county capital."),
        ),
        (
            "attacker_modifier",
            block(StaticModifier).doc("Modifiers for the attackers in a combat."),
        ),
        (
            "defender_modifier",
            block(StaticModifier).doc("Modifiers for the defender in a combat."),
        ),
        (
            "attacker_combat_effects",
            block(Struct(&COMBAT_EFFECT)).doc("Combat effect for the attackers."),
        ),
        (
            "defender_combat_effects",
            block(Struct(&COMBAT_EFFECT)).doc("Combat effect for the defenders."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Terrain;

impl Entity for Terrain {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::TERRAIN_TYPE,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: TERRAIN_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[anywhere(RefPattern::KeyValue("terrain"))],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(TERRAIN_DIR, ClauseKind::Struct(&TERRAIN))];
}
