//! Unlockable content kinds referenced by advances (and elsewhere):
//! buildings, units, and laws — def-only starters with their `unlock_*`
//! references (corpus-validated: 184/187/43 refs, 0 unresolved; the single
//! `unlock_unit = yes` toggle is skipped via the yes/no skip words).

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

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
    ];
}
