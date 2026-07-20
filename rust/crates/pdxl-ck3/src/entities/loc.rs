//! Localization keys — defined outside PDXScript in
//! `localization/<lang>/**/*.yml` (extracted by `pdxl-loc`, so no
//! `DefSource`), referenced by the text-bearing fields of events and
//! decisions.

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

/// A `key = <loc>` scalar loc reference gated to character interactions.
const fn interaction_loc(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some("common/character_interactions/"),
    }
}

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
            // Character-interaction text fields (from `_character_interactions.info`).
            // `desc` also catches nested dynamic-description descs. All corpus-
            // validated at ~0 unresolved.
            interaction_loc("desc"),
            interaction_loc("notification_text"),
            interaction_loc("intermediary_notification_text"),
            interaction_loc("prompt"),
            interaction_loc("send_name"),
            interaction_loc("options_heading"),
            interaction_loc("highlighted_reason"),
            interaction_loc("reply_item_key"),
            anywhere(RefPattern::KeyValue("custom_tooltip")),
        ],
        aliases: &[],
    }];
}
