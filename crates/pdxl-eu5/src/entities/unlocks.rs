//! Unlockable content kinds referenced by advances (per the advances
//! `readme.txt`) — def-only starters with their ungated `unlock_*`
//! references, all corpus-validated at 0 unresolved. The single
//! `unlock_unit = yes` toggle is skipped via the yes/no skip words;
//! `unlock_subject_type` lives in [`super::subject_type`].
//!
//! Production methods have TWO definition sites: top-level files in
//! `production_methods/` and `unique_production_methods = { … }` containers
//! nested in building defs — the first dual-rule directory (the def-rule
//! prefix-exclusivity requirement was relaxed for exactly this).

use crate::kinds;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::scripted::def_only;

/// An ungated `key = X` reference.
const fn unlock(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: None,
        alt: &[],
    }
}

pub(crate) struct Unlocks;

impl Entity for Unlocks {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            refs: &[unlock("unlock_building")],
            ..def_only(
                kinds::BUILDING,
                IconHint::Object,
                "in_game/common/building_types/",
            )
        },
        KindSpec {
            refs: &[unlock("unlock_unit")],
            ..def_only(kinds::UNIT, IconHint::Object, "in_game/common/unit_types/")
        },
        KindSpec {
            refs: &[unlock("unlock_law")],
            ..def_only(kinds::LAW, IconHint::Action, "in_game/common/laws/")
        },
        KindSpec {
            refs: &[unlock("unlock_ability")],
            ..def_only(
                kinds::UNIT_ABILITY,
                IconHint::Action,
                "in_game/common/unit_abilities/",
            )
        },
        KindSpec {
            refs: &[unlock("unlock_interaction")],
            ..def_only(
                kinds::CHARACTER_INTERACTION,
                IconHint::Action,
                "in_game/common/character_interactions/",
            )
        },
        KindSpec {
            refs: &[unlock("unlock_country_interaction")],
            ..def_only(
                kinds::COUNTRY_INTERACTION,
                IconHint::Action,
                "in_game/common/country_interactions/",
            )
        },
        KindSpec {
            refs: &[unlock("unlock_relation_type")],
            ..def_only(
                kinds::RELATION_TYPE,
                IconHint::Tag,
                "in_game/common/scripted_relations/",
            )
        },
        KindSpec {
            refs: &[unlock("unlock_levy")],
            ..def_only(kinds::LEVY, IconHint::Object, "in_game/common/levies/")
        },
        KindSpec {
            refs: &[unlock("unlock_government_reform")],
            ..def_only(
                kinds::GOVERNMENT_REFORM,
                IconHint::Action,
                "in_game/common/government_reforms/",
            )
        },
        KindSpec {
            refs: &[unlock("unlock_casus_belli")],
            ..def_only(
                kinds::CASUS_BELLI,
                IconHint::Action,
                "in_game/common/casus_belli/",
            )
        },
        // Production methods: top-level defs in their own dir…
        KindSpec {
            refs: &[unlock("unlock_production_method")],
            ..def_only(
                kinds::PRODUCTION_METHOD,
                IconHint::Function,
                "in_game/common/production_methods/",
            )
        },
        // …plus inline defs inside building bodies (dual-rule directory).
        KindSpec {
            kind: kinds::PRODUCTION_METHOD,
            icon: IconHint::Function,
            defs: Some(DefSource {
                dir_prefix: "in_game/common/building_types/",
                shape: DefShape::ChildrenOf {
                    containers: &["unique_production_methods"],
                },
            }),
            refs: &[],
            aliases: &[],
        },
    ];
}
