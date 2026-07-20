//! Interface-script (`.gui`) symbol kinds — templates and widget types.
//!
//! Definitions and references are harvested by the `pdxl-gui` crate from
//! `gui/**/*.gui` files (dialect parser + name-gated two-pass extraction, see
//! its crate docs), not by directory def rules — so these rows carry no
//! `DefSource` and no `RefRule`s. They exist to register the kinds with the
//! schema (count table, icons, doc-ref lookup); the engine learns which kinds
//! play the two gui roles via [`pdxl_analysis::GuiKinds`] on the schema.

use crate::kinds;
use pdxl_analysis::{IconHint, KindSpec};

use super::Entity;

pub(crate) struct Gui;

impl Entity for Gui {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::GUI_TEMPLATE,
            icon: IconHint::Function,
            defs: None,
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::GUI_TYPE,
            icon: IconHint::Object,
            defs: None,
            refs: &[],
            aliases: &[],
        },
    ];
}
