//! Event namespaces (`namespace = X` at the top of an events file). Harvested
//! as a keyed-value definition (see `Schema::new`'s `keyed_value_defs`), not a
//! directory rule — the symbol is the declaration's value, so hovering it shows
//! the file's `#!` doc. This row only supplies the presentation icon; it has no
//! `defs` (directory) or `refs`.

use pdxl_analysis::{IconHint, KindSpec, SymbolKind};

use super::Entity;

pub(crate) struct Namespace;

impl Entity for Namespace {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::Namespace,
        icon: IconHint::Object,
        defs: None,
        refs: &[],
        aliases: &[],
    }];
}
