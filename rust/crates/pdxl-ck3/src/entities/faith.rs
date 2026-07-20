//! Faiths — the block children of `faiths = { }` inside
//! `common/religion/religion_types/`, referenced by `faith:x` and by the
//! `religion =` / `faith =` attributes of history characters (both keys take
//! a *faith* name there; `religion` is the legacy spelling).

use crate::kinds;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

/// A `key = X` faith reference gated to history characters.
const fn in_history(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some("history/characters/"),
    }
}

pub(crate) struct Faith;

impl Entity for Faith {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::FAITH,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/religion/religion_types/",
            shape: DefShape::ChildrenOf {
                containers: &["faiths"],
            },
        }),
        refs: &[
            anywhere(RefPattern::ScopePrefix("faith")),
            in_history("religion"),
            in_history("faith"),
        ],
        aliases: &[],
    }];
}
