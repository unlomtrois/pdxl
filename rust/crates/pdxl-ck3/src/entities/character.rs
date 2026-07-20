//! History characters (`history/characters/`).

use crate::kinds;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec};

use super::Entity;

pub(crate) struct Character;

impl Entity for Character {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CHARACTER,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "history/characters/",
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    }];
}
