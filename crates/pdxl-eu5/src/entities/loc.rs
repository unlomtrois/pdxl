//! Localization-key references whose target kind is the universal `LOC_KEY`.
//! Localization definitions themselves are extracted from PDX-YML files.
//!
//! International-organization `desc` values are localization keys throughout
//! nested variable/change-factor structures. Variable `format` and
//! `change_format` values are localization keys as documented by the IO
//! directory readme. Vanilla `desc` has 343 uses / 138 distinct values: 132
//! resolve in English localization; bracketed inline localization expressions
//! are skipped, while two missing keys are genuine vanilla dangling refs.

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

pub(crate) struct Loc;

impl Entity for Loc {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::LOC_KEY,
        icon: IconHint::Text,
        defs: None,
        refs: &[
            RefRule {
                pattern: RefPattern::KeyValue("desc"),
                gate: Some(super::international_organization::IO_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("format"),
                gate: Some(super::international_organization::IO_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("change_format"),
                gate: Some(super::international_organization::IO_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("title"),
                gate: Some(super::event::EVENTS_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("desc"),
                gate: Some(super::event::EVENTS_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("historical_info"),
                gate: Some(super::event::EVENTS_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("name"),
                gate: Some(super::event::EVENTS_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("hint_tag"),
                gate: Some(super::situation::SITUATIONS_DIR),
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("desc"),
                gate: Some(super::situation::SITUATIONS_DIR),
                alt: &[],
            },
        ],
        aliases: &[],
    }];
}
