//! Cultures (`common/culture/cultures/`), referenced by `culture:x`.

use crate::kinds;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::anywhere;

pub(crate) struct Culture;

impl Entity for Culture {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CULTURE,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/culture/cultures/",
            shape: DefShape::TopLevel,
        }),
        refs: &[anywhere(RefPattern::ScopePrefix("culture"))],
        aliases: &[],
    }];
}
