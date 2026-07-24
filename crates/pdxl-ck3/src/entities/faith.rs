//! Faiths — the block children of `faiths = { }` inside
//! `common/religion/religion_types/`, referenced by `faith:x` and by the
//! `religion =` / `faith =` attributes of history characters and history
//! provinces (both keys take a *faith* name there; `religion` is the legacy
//! spelling).

use crate::kinds;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

/// A `key = X` faith reference gated to one history directory.
const fn in_dir(dir: &'static str, key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(dir),
        alt: &[],
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
            in_dir("history/characters/", "religion"),
            in_dir("history/characters/", "faith"),
            in_dir("history/provinces/", "religion"),
            in_dir("history/provinces/", "faith"),
        ],
        aliases: &[],
    }];
}
