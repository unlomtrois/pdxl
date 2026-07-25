//! Estates (`in_game/common/estates/`) — the eight top-level defs
//! (crown/nobles/clergy/burghers/peasants/dhimmi/tribes/cossacks).
//!
//! References (corpus-validated, 0 unresolved):
//! - `estate_type:X` scope literals anywhere (14,767);
//! - bare `estate = X` anywhere (4,094 — the `estate = estate_type:X` and
//!   `estate = scope:x` chain forms are skipped by the engine for free);
//! - the age bodies' `max/min_ai_privilege_per_estate = { <estate> = N }`
//!   maps, whose block *keys* are the references
//!   ([`RefPattern::KeyBlockKeys`], gated to the age dir).
//!
//! The estate `color = X` named-color references live in
//! [`super::named_color`]. Full body model is a later worklist item.

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::advance::AGE_DIR;
use super::scripted::def_only;

pub(crate) const ESTATES_DIR: &str = "in_game/common/estates/";

/// A privilege-cap map reference rule (`key = { <estate> = N … }`).
const fn estate_map(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyBlockKeys(key),
        gate: Some(AGE_DIR),
        alt: &[],
    }
}

pub(crate) struct Estate;

impl Entity for Estate {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        refs: &[
            // The `estate_type:` literal is table-derived (`crate::derived`).
            RefRule {
                pattern: RefPattern::KeyValue("estate"),
                gate: None,
                alt: &[],
            },
            estate_map("max_ai_privilege_per_estate"),
            estate_map("min_ai_privilege_per_estate"),
        ],
        ..def_only(kinds::ESTATE, IconHint::Hierarchy, ESTATES_DIR)
    }];
}
