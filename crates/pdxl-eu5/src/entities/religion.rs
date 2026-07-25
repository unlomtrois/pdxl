//! Religions (`in_game/common/religions/`) — top-level definitions (293 in
//! vanilla), referenced by `religion_definition = X` (2,337 refs
//! corpus-wide, 0 unresolved, ungated). Full body model comes later.

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::scripted::def_only;

pub(crate) const RELIGIONS_DIR: &str = "in_game/common/religions/";

pub(crate) struct Religion;

impl Entity for Religion {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        refs: &[RefRule {
            pattern: RefPattern::KeyValue("religion_definition"),
            gate: None,
            alt: &[],
        }],
        ..def_only(kinds::RELIGION, IconHint::Object, RELIGIONS_DIR)
    }];
}
