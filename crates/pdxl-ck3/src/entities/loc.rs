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
        alt: &[],
    }
}

/// A `key = <loc>` scalar loc reference gated to scheme types.
const fn scheme_loc(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some("common/schemes/scheme_types/"),
        alt: &[],
    }
}

/// The person/tense fields of effect/trigger localization entries
/// (`first`, `third_past`, `global_neg`, …) are loc keys in both dirs —
/// one gated rule per key per directory (98.8% corpus resolution; misses
/// are genuine missing-loc bugs).
macro_rules! person_tense_rules {
    ($($key:literal),* $(,)?) => {
        [$(
            RefRule {
                pattern: RefPattern::KeyValue($key),
                gate: Some(super::effect_localization::EFFECT_LOC_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue($key),
                gate: Some(super::effect_localization::TRIGGER_LOC_DIR),
                alt: &[],
            },
        )*]
    };
}

/// The generated effect/trigger-localization loc rules (17 keys × 2 dirs).
static PERSON_TENSE_RULES: [RefRule; 34] = person_tense_rules!(
    "first",
    "third",
    "global",
    "none",
    "first_past",
    "third_past",
    "global_past",
    "first_not",
    "third_not",
    "global_not",
    "none_not",
    "first_neg",
    "third_neg",
    "global_neg",
    "first_past_neg",
    "third_past_neg",
    "global_past_neg",
);

pub(crate) struct Loc;

impl Entity for Loc {
    const KINDS: &'static [KindSpec] = &[
        // Effect/trigger-localization person/tense fields (second row for the
        // same kind — Schema::new dedups registration).
        KindSpec {
            kind: kinds::LOC_KEY,
            icon: IconHint::Text,
            defs: None,
            refs: &PERSON_TENSE_RULES,
            aliases: &[],
        },
        KindSpec {
            kind: kinds::LOC_KEY,
            icon: IconHint::Text,
            defs: None,
            refs: &[
                RefRule {
                    pattern: RefPattern::KeyValue("title"),
                    gate: Some("events/"),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue("desc"),
                    gate: Some("events/"),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue("opening"),
                    gate: Some("events/"),
                    alt: &[],
                },
                // `name` is a loc key only directly inside an option (elsewhere it
                // names variable lists etc.); `text` only inside a gated name block
                // or a custom_tooltip block.
                RefRule {
                    pattern: RefPattern::KeyValueUnder("option", "name"),
                    gate: Some("events/"),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValueUnder("name", "text"),
                    gate: Some("events/"),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValueUnder("custom_tooltip", "text"),
                    gate: None,
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue("title"),
                    gate: Some("common/decisions/"),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue("desc"),
                    gate: Some("common/decisions/"),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue("selection_tooltip"),
                    gate: Some("common/decisions/"),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue("confirm_text"),
                    gate: Some("common/decisions/"),
                    alt: &[],
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
                // Scheme-type text fields (all scalar loc keys in the corpus).
                scheme_loc("desc"),
                scheme_loc("success_desc"),
                scheme_loc("discovery_desc"),
                anywhere(RefPattern::KeyValue("custom_tooltip")),
            ],
            aliases: &[],
        },
    ];
}
