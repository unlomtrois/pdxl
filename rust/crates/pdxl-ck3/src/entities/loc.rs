//! Localization keys — defined outside PDXScript in
//! `localization/<lang>/**/*.yml` (extracted by `pdxl-loc`, so no
//! `DefSource`), referenced by the text-bearing fields of events and
//! decisions.

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

pub(crate) struct Loc;

impl Entity for Loc {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::LOC_KEY,
        icon: IconHint::Text,
        defs: None,
        refs: &[
            RefRule {
                pattern: RefPattern::KeyValue("title"),
                gate: Some("events/"),
            },
            RefRule {
                pattern: RefPattern::KeyValue("desc"),
                gate: Some("events/"),
            },
            RefRule {
                pattern: RefPattern::KeyValue("opening"),
                gate: Some("events/"),
            },
            // `name` is a loc key only directly inside an option (elsewhere it
            // names variable lists etc.); `text` only inside a gated name block
            // or a custom_tooltip block.
            RefRule {
                pattern: RefPattern::KeyValueUnder("option", "name"),
                gate: Some("events/"),
            },
            RefRule {
                pattern: RefPattern::KeyValueUnder("name", "text"),
                gate: Some("events/"),
            },
            RefRule {
                pattern: RefPattern::KeyValueUnder("custom_tooltip", "text"),
                gate: None,
            },
            RefRule {
                pattern: RefPattern::KeyValue("title"),
                gate: Some("common/decisions/"),
            },
            RefRule {
                pattern: RefPattern::KeyValue("desc"),
                gate: Some("common/decisions/"),
            },
            RefRule {
                pattern: RefPattern::KeyValue("selection_tooltip"),
                gate: Some("common/decisions/"),
            },
            RefRule {
                pattern: RefPattern::KeyValue("confirm_text"),
                gate: Some("common/decisions/"),
            },
            anywhere(RefPattern::KeyValue("custom_tooltip")),
        ],
        aliases: &[],
    }];
}
