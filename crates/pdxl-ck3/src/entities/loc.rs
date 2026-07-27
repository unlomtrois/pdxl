//! Localization keys, defined outside PDXScript in
//! `localization/<lang>/**/*.yml` (extracted by `pdxl-yml`, so no
//! `DefSource`), referenced by the text-bearing fields of events and
//! decisions.

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

/// A `key = <loc>` scalar loc reference gated to scheme types.
const fn scheme_loc(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some("common/schemes/scheme_types/"),
        alt: &[],
    }
}

/// The person/tense fields of effect/trigger localization entries
/// (`first`, `third_past`, `global_neg`, …) are loc keys in both dirs,
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
        // same kind, Schema::new dedups registration).
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
                // Character-interaction and casus-belli text fields used to be
                // listed here, one gated rule per key. The *named* ones are now
                // carried by the `FieldSpec` rows of the bodies that own them
                // (`character_interaction.rs`, `casus_belli.rs`), which is where
                // the keys were already written down — see `FieldSpec::refs`.
                //
                // `desc` stays a rule in both dirs: besides the body field, it
                // appears at depths no body enumerates — the leaves of a dynamic
                // description (`first_valid`/`triggered_desc`) and the tooltip of
                // a weighted script-value line (`ai_accept`'s `modifier` blocks).
                // A key that is not a field of one modeled shape is a rule's job.
                RefRule {
                    pattern: RefPattern::KeyValue("desc"),
                    gate: Some("common/character_interactions/"),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue("desc"),
                    gate: Some("common/casus_belli_types/"),
                    alt: &[],
                },
                // Scheme-type text fields (all scalar loc keys in the corpus).
                scheme_loc("desc"),
                scheme_loc("success_desc"),
                scheme_loc("discovery_desc"),
                anywhere(RefPattern::KeyValue("custom_tooltip")),
                // Religion/faith `localization = { AnyKey = loc_key }` maps —
                // dynamic keys, loc-key values, single or listed
                // (`key = { TAG TAG }`); accessed via `[Faith.Custom('key')]`.
                // Corpus: 19 unresolved, all genuine vanilla bugs (keys like
                // CHARACTER_HERSELFHIMSELF_HIMSELF or witchgodname_paganism
                // that no localization file defines).
                RefRule {
                    pattern: RefPattern::KeyBlockValues("localization"),
                    gate: Some("common/religion/"),
                    alt: &[],
                },
            ],
            aliases: &[],
        },
    ];
}
