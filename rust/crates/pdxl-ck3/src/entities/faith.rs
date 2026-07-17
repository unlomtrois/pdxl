//! Faiths — the block children of `faiths = { }` inside
//! `common/religion/religion_types/`, referenced by `faith:x`.

use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, SymbolKind};

use super::Entity;
use super::common::anywhere;

pub(crate) struct Faith;

impl Entity for Faith {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::Faith,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/religion/religion_types/",
            shape: DefShape::ChildrenOf {
                containers: &["faiths"],
            },
        }),
        refs: &[anywhere(RefPattern::ScopePrefix("faith"))],
        aliases: &[],
    }];
}
