//! Cultures (`common/culture/cultures/`), referenced by `culture:x` and by
//! the `culture =` attribute of history characters and dynasties.

use crate::kinds;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

/// `culture = X` gated to one directory (a bare `culture =` is a scope
/// assignment elsewhere).
const fn culture_in(dir: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue("culture"),
        gate: Some(dir),
    }
}

pub(crate) struct Culture;

impl Entity for Culture {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CULTURE,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/culture/cultures/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::ScopePrefix("culture")),
            culture_in("history/characters/"),
            culture_in("common/dynasties/"),
        ],
        aliases: &[],
    }];
}
