//! Domiciles (`common/domiciles/`) — the movable seat a landless or
//! administrative character calls home, and the buildings raised inside it.
//!
//! Two kinds: **types** (`types/`, 5 defs — `camp`, `estate`, `yurt`,
//! `east_asian_estate`, `japanese_manor`) and **buildings** (`buildings/`,
//! 1620 defs, by far the largest def set in the CK3 schema).
//!
//! Both readmes match the corpus: every documented field is used, and the only
//! documented-but-unused ones are the building effects `on_start` and
//! `on_cancelled` (`on_complete` is attested 178 times). Nothing is
//! corpus-only, so nothing is marked *(corpus)*.
//!
//! Building references are heavy — `has_domicile_building` alone fires 3178
//! times — and about a third of the def set participates: 1238 distinct values
//! across the six keys, of which 1233 are literal and every one resolves. The
//! remaining five are macro-composed (`$BUILDING$_02`), which
//! `Schema::skip_ref_value` drops for free. `previous_building` is gated to
//! this directory because plain `buildings/` uses the same key for ordinary
//! holding buildings; all 1251 distinct upgrade parents resolve.
//!
//! **Deliberate omissions.** `parameters` are free-form keys read by
//! `has_domicile_parameter` — the readme is explicit that only their existence
//! matters, not their value. The slot names under `domicile_building_slots`
//! are likewise arbitrary ("any name you want"), and slot layering is
//! positional, so they are structure rather than symbols. `map_entity`,
//! `illustration`, `icon`, `texture` and `soundeffect` name assets and sound
//! banks this schema does not model.
//!
//! Two near-miss keys were checked and rejected: `reference = estate` in
//! `common/event_themes/` is an event-theme background that happens to share a
//! domicile name, and `NEW_DOMICILE_TYPE = japanese_manor` is a macro argument
//! whose key is arbitrary per scripted effect. Neither is a general rule.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{
    self, Effect, ScriptValue, StaticModifier, Struct, Trigger,
};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{COST, DURATION, OPAQUE, TRIGGERED_ASSET, anywhere, toggle};

pub(crate) const TYPES_DIR: &str = "common/domiciles/types/";
pub(crate) const BUILDINGS_DIR: &str = "common/domiciles/buildings/";

/// Slot types a building may occupy. Internal slots live inside another
/// building rather than in the domicile frame.
const SLOT_TYPES: &[&str] = &["main", "external", "internal"];

/// One entry of `domicile_building_slots` — a placement in the domicile frame.
/// The key is arbitrary, so these are structure, not definitions.
static BUILDING_SLOT: StructSpec = StructSpec {
    name: "domicile building slot",
    fields: &[
        (
            "slot_type",
            scalar(Setting)
                .doc(
                    "Which buildings may be constructed here. A `main` slot with no previous \
                     building is filled automatically on start. Default `external`; `internal` \
                     is handled by the buildings themselves.",
                )
                .values(&["main", "external"]),
        ),
        (
            "position",
            block(Struct(&OPAQUE))
                .doc("Placement in the domicile window; accepts percentages or direct values."),
        ),
        (
            "size",
            block(Struct(&OPAQUE))
                .doc("Size of buildings placed here; accepts percentages or direct values."),
        ),
        (
            "empty_slot_asset",
            block(Struct(&TRIGGERED_ASSET))
                .doc("Assets drawn while the slot is empty. The first valid match wins."),
        ),
        (
            "construction_slot_asset",
            block(Struct(&TRIGGERED_ASSET))
                .doc("Assets drawn while a building here is under construction."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `domicile_building_slots = { … }` — keyed by arbitrary slot name, unlocked
/// in definition order; layering front-to-back follows the same order.
static BUILDING_SLOTS: StructSpec = StructSpec {
    name: "domicile building slots",
    fields: &[],
    fallback: Fallback::Struct(&BUILDING_SLOT),
};

/// The body of one domicile type (`_domicile_types.info`).
static DOMICILE_TYPE: StructSpec = StructSpec {
    name: "domicile type",
    fields: &[
        (
            "allowed_for_character",
            block(Trigger)
                .doc("Whether this type is available to the character. Root is the character."),
        ),
        (
            "rename_window",
            scalar(Setting)
                .doc("Retitles the domicile window. Default `none`.")
                .values(&["none", "primary_title", "house"]),
        ),
        (
            "illustration",
            scalar(Setting).doc("Texture shown in the realm tab."),
        ),
        (
            "icon",
            scalar(Setting).doc("Flat icon representing the type."),
        ),
        (
            "map_pin_texture",
            scalar(Setting).doc("Texture for the map pin."),
        ),
        (
            "map_pin_anchor",
            scalar(Setting)
                .doc("Where the map pin anchors relative to its province. Default `right`.")
                .values(&["up", "right"]),
        ),
        (
            "map_pin_lobby",
            toggle("Whether this domicile appears in the game lobby."),
        ),
        (
            "provisions",
            toggle(
                "Whether the domicile manages provisions. Those that do travel to a new \
                 location; the rest move instantly.",
            ),
        ),
        ("travel", toggle("Whether the domicile may travel.")),
        (
            "herd",
            toggle("Whether the domicile manages the herd resource."),
        ),
        (
            "culture_and_faith",
            toggle("Whether the domicile stores a culture and faith. Default `no`."),
        ),
        (
            "move_with_realm_capital",
            toggle("Whether it relocates when the realm capital moves. Default `no`."),
        ),
        (
            "can_move_manually",
            toggle("Whether it can be moved without a bespoke feature."),
        ),
        (
            "move_cooldown",
            block(Struct(&DURATION)).doc("How long before the domicile may move again."),
        ),
        (
            "move_cost",
            block(Struct(&COST)).doc("Cost of moving the domicile."),
        ),
        (
            "domicile_temperament_low_modifier",
            block(StaticModifier).doc(
                "Applied to the owner when the majority of their court dislikes them. Accepts \
                 a `scale` script value.",
            ),
        ),
        (
            "domicile_temperament_high_modifier",
            block(StaticModifier)
                .doc("Applied to the owner when the majority of their court likes them."),
        ),
        (
            "base_external_slots",
            scalar(Setting).doc("How many external building slots start unlocked."),
        ),
        (
            "domicile_building_slots",
            block(Struct(&BUILDING_SLOTS)).doc(
                "Every main and external slot, unlocked in definition order. Internal slots \
                 are declared by the buildings instead.",
            ),
        ),
        (
            "domicile_asset",
            block(Struct(&OPAQUE)).doc(
                "Background, foreground and ambience for the domicile window. The first valid \
                 match wins.",
            ),
        ),
        (
            "map_entity",
            scalar_or_block(Setting, Struct(&TRIGGERED_ASSET)).doc(
                "Map entity for the domicile — a bare name, or a `trigger`/`reference` pair. \
                 The first match in read order wins. Takes the holding locator when the \
                 location has no holding, otherwise the activity locator if it is free.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one domicile building (`_domicile_buildings.info`).
static DOMICILE_BUILDING: StructSpec = StructSpec {
    name: "domicile building",
    fields: &[
        (
            "can_construct",
            block(Trigger).doc("Whether the owner can construct this. Root is the character."),
        ),
        (
            "can_construct_potential",
            block(Trigger).doc("Whether the owner can even consider constructing this."),
        ),
        (
            "on_start",
            block(Effect).doc(
                "Fires when construction begins. Root is the domicile, `scope:owner` its owner.",
            ),
        ),
        (
            "on_cancelled",
            block(Effect).doc("Fires when construction is cancelled."),
        ),
        (
            "on_complete",
            block(Effect).doc("Fires when construction finishes."),
        ),
        (
            "construction_time",
            scalar_or_block(Setting, ScriptValue)
                .doc("Days to construct. Affected by `build_speed` and `domicile_build_speed`."),
        ),
        (
            "parameters",
            block(Struct(&OPAQUE)).doc(
                "Arbitrary keys read by `has_domicile_parameter`, which only checks existence — \
                 `yes` or `no` makes no difference. Not inherited from `previous_building`, so \
                 they must be repeated on each upgrade.",
            ),
        ),
        (
            "slot_type",
            scalar(Setting)
                .doc(
                    "The only slot type this may occupy. Internal slots need a previous \
                     building and sit inside a main or external one. A `main` building with no \
                     previous building is constructed automatically on start. Default \
                     `external`.",
                )
                .values(SLOT_TYPES),
        ),
        (
            "internal_slots",
            scalar(Setting).doc("How many internal slots this building unlocks. Default 0."),
        ),
        (
            "allowed_domicile_types",
            block(Struct(&OPAQUE)).doc(
                "Domicile types that may build this. **Leaving it unset hides the building \
                 entirely.**",
            ),
        ),
        (
            "previous_building",
            scalar(Setting).doc(
                "The building this upgrades from. Absent means this is a base building — the \
                 first tier of a chain. Two buildings naming the same previous building fork \
                 the upgrade path.",
            ),
        ),
        ("cost", block(Struct(&COST)).doc("Construction cost.")),
        (
            "refund",
            block(Struct(&COST)).doc("Refund on demolition. Falls back to `cost` when unset."),
        ),
        (
            "character_modifier",
            block(StaticModifier).doc(
                "Applied to the domicile owner. Buildings inherit the character modifiers of \
                 earlier buildings on the same track. Prefer `domicile_monthly_gold_add`/`_mult` \
                 (and the `_prestige`/`_piety`/`_influence` variants) over holding income \
                 modifiers.",
            ),
        ),
        (
            "province_modifier",
            block(StaticModifier).doc("Applied to the province the domicile sits in."),
        ),
        (
            "ai_value",
            block(ScriptValue).doc(
                "How much the AI wants this, weighed against ordinary buildings. Root is the \
                 domicile, `scope:owner` its owner.",
            ),
        ),
        (
            "asset",
            block(Struct(&TRIGGERED_ASSET)).doc(
                "Which asset the building uses. `soundeffect` supports sound parameters \
                 appended from the building's tier.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Domicile;

impl Entity for Domicile {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::DOMICILE_TYPE,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: TYPES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                anywhere(RefPattern::KeyValue("is_domicile_type")),
                anywhere(RefPattern::KeyValue("domicile_type")),
                RefRule {
                    pattern: RefPattern::KeyList("allowed_domicile_types"),
                    gate: Some(BUILDINGS_DIR),
                    alt: &[],
                },
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::DOMICILE_BUILDING,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: BUILDINGS_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                anywhere(RefPattern::KeyValue("has_domicile_building")),
                anywhere(RefPattern::KeyValue("has_domicile_building_or_higher")),
                anywhere(RefPattern::KeyValue("add_domicile_building")),
                anywhere(RefPattern::KeyValue("remove_domicile_building")),
                anywhere(RefPattern::KeyValue("remove_domicile_building_no_refund")),
                anywhere(RefPattern::KeyValue("lower_domicile_building_no_refund")),
                // Holding buildings use the same key, so gate the upgrade link.
                RefRule {
                    pattern: RefPattern::KeyValue("previous_building"),
                    gate: Some(BUILDINGS_DIR),
                    alt: &[],
                },
            ],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (TYPES_DIR, ClauseKind::Struct(&DOMICILE_TYPE)),
        (BUILDINGS_DIR, ClauseKind::Struct(&DOMICILE_BUILDING)),
    ];
}
