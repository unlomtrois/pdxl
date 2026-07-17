//! Cultures (`common/culture/cultures/`), referenced by `culture:x`.

use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, SymbolKind};

use super::Entity;
use super::common::anywhere;

pub(crate) struct Culture;

impl Entity for Culture {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::Culture,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/culture/cultures/",
            shape: DefShape::TopLevel,
        }),
        refs: &[anywhere(RefPattern::ScopePrefix("culture"))],
        aliases: &[],
    }];
}
