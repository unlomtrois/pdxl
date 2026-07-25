//! Engine defines (`loading_screen/common/defines/`). A top-level namespace
//! contains scalar or block constants, referenced as
//! `define:NAMESPACE|CONSTANT` (for example
//! `define:NMapColors|INTERNATIONAL_ORGANIZATION_LEADER_COLOR`).
//!
//! The qualified name is the symbol identity; navigation lands on the constant
//! key inside its namespace. Top-level `@` helper constants are deliberately
//! excluded because they use the ordinary file-local script-constant model.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::{Fallback, StructSpec};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

pub(crate) const DEFINES_DIR: &str = "loading_screen/common/defines/";

static DEFINE_NAMESPACE: StructSpec = StructSpec {
    name: "define namespace",
    fields: &[],
    fallback: Fallback::Ignore,
};

pub(crate) struct Define;

impl Entity for Define {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::DEFINE,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: DEFINES_DIR,
            shape: DefShape::QualifiedFields { separator: "|" },
        }),
        refs: &[RefRule {
            pattern: RefPattern::ScopePrefix("define"),
            gate: None,
            alt: &[],
        }],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(DEFINES_DIR, Struct(&DEFINE_NAMESPACE))];
}
