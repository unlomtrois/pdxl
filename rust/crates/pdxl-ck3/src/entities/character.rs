//! History characters (`history/characters/`).

use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, SymbolKind};

use super::Entity;

pub(crate) struct Character;

impl Entity for Character {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::Character,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "history/characters/",
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    }];
}
