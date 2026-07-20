//! Character traits (`common/traits/`).

use crate::kinds;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

pub(crate) struct Traits;

impl Entity for Traits {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::TRAIT,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: "common/traits/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("add_trait")),
            anywhere(RefPattern::KeyValue("remove_trait")),
            anywhere(RefPattern::KeyValue("has_trait")),
            // XP effects/triggers name the trait in a block: `{ trait = X … }`.
            anywhere(RefPattern::KeyBlockField("add_trait_xp", "trait")),
            anywhere(RefPattern::KeyBlockField("has_trait_xp", "trait")),
            // History characters list starting/dated traits as `trait = X`
            // (both the body and the dated blocks; corpus 0 unresolved).
            RefRule {
                pattern: RefPattern::KeyValue("trait"),
                gate: Some("history/characters/"),
            },
        ],
        // CK3 traits expose group / group_equivalence names as valid refs.
        aliases: &["group", "group_equivalence"],
    }];
}
