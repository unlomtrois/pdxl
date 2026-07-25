//! Cultures (`in_game/common/cultures/`) — top-level definitions (2,083 in
//! vanilla), referenced by `culture_definition = X` (countries and other
//! setup data; 2,337 refs corpus-wide, 0 unresolved, ungated). The full
//! culture body model is the next worklist item; def-only for now.

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::scripted::def_only;

pub(crate) const CULTURES_DIR: &str = "in_game/common/cultures/";

pub(crate) struct Culture;

impl Entity for Culture {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        refs: &[RefRule {
            pattern: RefPattern::KeyValue("culture_definition"),
            gate: None,
            alt: &[],
        }],
        ..def_only(kinds::CULTURE, IconHint::Object, CULTURES_DIR)
    }];
}
