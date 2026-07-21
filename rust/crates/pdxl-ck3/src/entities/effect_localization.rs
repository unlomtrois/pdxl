//! Effect and trigger localization (`common/effect_localization/` +
//! `common/trigger_localization/`, from `_effect_localization.info` and its
//! sibling) — per-effect/trigger text entries keyed by grammatical person
//! and tense: `{first,third,global,none} × {∅,_past,_not,_neg,_past_neg}`.
//! `_past` = past tense, `_not` = negated trigger text, `_neg` = negative
//! value output ("lose X" vs "gain X"); `none` = no-scope form. Entries are
//! looked up by effect/trigger name by the engine (an entry is optional —
//! the default is `<name>_first`-style loc keys), and by
//! `custom_description = { text = X }` in script.
//!
//! The person/tense field values are loc-key references (resolved in
//! `loc.rs`, the LOC_KEY entity — ~6k refs, ~96% resolve; the ~190 misses
//! are genuine dead loc references, almost all vanilla's own — the keys
//! exist in no language). `custom_description`'s `text = X` is
//! **not** a strict reference: 14% of corpus values are plain loc keys the
//! engine falls back to, so a single-kind rule would flag valid script.

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::context::ScalarKind::LocKey;
use pdxl_analysis::context::{Fallback, StructSpec, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec};

use super::Entity;

pub(crate) const EFFECT_LOC_DIR: &str = "common/effect_localization/";
pub(crate) const TRIGGER_LOC_DIR: &str = "common/trigger_localization/";

/// The body of one effect/trigger localization entry.
static ENTRY: StructSpec = StructSpec {
    name: "effect/trigger localization",
    fields: &[
        (
            "first",
            scalar(LocKey).doc("First person: \"I gain 123 gold\"."),
        ),
        (
            "third",
            scalar(LocKey).doc("Third person: \"King John gains 123 gold\"."),
        ),
        (
            "global",
            scalar(LocKey).doc("Global pronoun: King John: \"Gains 123 gold\"."),
        ),
        ("none", scalar(LocKey).doc("No-scope form.")),
        (
            "first_past",
            scalar(LocKey).doc("First person, past tense: \"I gained 123 gold\"."),
        ),
        (
            "third_past",
            scalar(LocKey).doc("Third person, past tense."),
        ),
        (
            "global_past",
            scalar(LocKey).doc("Global pronoun, past tense."),
        ),
        (
            "first_not",
            scalar(LocKey).doc("First person, negated trigger text."),
        ),
        (
            "third_not",
            scalar(LocKey).doc("Third person, negated trigger text."),
        ),
        (
            "global_not",
            scalar(LocKey).doc("Global pronoun, negated trigger text."),
        ),
        (
            "none_not",
            scalar(LocKey).doc("No-scope form, negated trigger text."),
        ),
        (
            "first_neg",
            scalar(LocKey).doc(
                "First person, negative value (\"lose X\"); the value shown is always positive.",
            ),
        ),
        (
            "third_neg",
            scalar(LocKey).doc("Third person, negative value."),
        ),
        (
            "global_neg",
            scalar(LocKey).doc("Global pronoun, negative value."),
        ),
        (
            "first_past_neg",
            scalar(LocKey).doc("First person, past tense, negative value."),
        ),
        (
            "third_past_neg",
            scalar(LocKey).doc("Third person, past tense, negative value."),
        ),
        (
            "global_past_neg",
            scalar(LocKey).doc("Global pronoun, past tense, negative value."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct EffectLocalization;

impl Entity for EffectLocalization {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::EFFECT_LOC,
            icon: IconHint::Text,
            defs: Some(DefSource {
                dir_prefix: EFFECT_LOC_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::TRIGGER_LOC,
            icon: IconHint::Text,
            defs: Some(DefSource {
                dir_prefix: TRIGGER_LOC_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (EFFECT_LOC_DIR, ClauseKind::Struct(&ENTRY)),
        (TRIGGER_LOC_DIR, ClauseKind::Struct(&ENTRY)),
    ];
}
