//! Coats of arms (`main_menu/common/coat_of_arms/coat_of_arms/`) — 4,654
//! top-level defs (many named after country tags, which is why `coa = X`
//! initially looked like a tag reference — it is a COA_KEY per the
//! flag-definitions doc header). Referenced by `coa = X` in
//! `main_menu/common/flag_definitions/` (262 refs, 0 unresolved; the
//! `coa = list X` selector keyword is skipped via the `list` skip word).
//! Full body model (colored_emblem etc.) is a later worklist item.

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::scripted::def_only;

pub(crate) struct CoatOfArms;

impl Entity for CoatOfArms {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        refs: &[RefRule {
            pattern: RefPattern::KeyValue("coa"),
            gate: Some("main_menu/common/flag_definitions/"),
            alt: &[],
        }],
        ..def_only(
            kinds::COAT_OF_ARMS,
            IconHint::Object,
            "main_menu/common/coat_of_arms/coat_of_arms/",
        )
    }];
}
