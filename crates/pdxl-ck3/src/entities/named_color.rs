//! Named colors (`common/named_colors/`) — the block children of the
//! `colors = { }` container, each a color literal (`english = { 0.8 0.2 0.2 }`,
//! `red = hsv { … }`, `brown = hsv360 { … }`). Referenced by the scalar form
//! of color fields (corpus-validated):
//!
//! - `color = X` in `common/culture/` (cultures + language pillars, 168 refs)
//!   and `common/religion/` (faith colors). Ungated it would misfire on
//!   `color = good/bad` (modifier definition formats) and `color =
//!   hair/skin/eye` (genes), where the key means something else.
//! - `color1`…`color5` in `common/coat_of_arms/` (~17k refs). Slot
//!   self-references (`color1 = color2`) and the `list` selector keyword are
//!   skipped via `SCOPE_KEYWORDS` — they are relative references, not names.
//!
//! Accepted noise (2): `color = khitan` / `= tungusic` in
//! `common/culture/pillars/00_language.txt` name colors that no
//! `named_colors` file defines — an apparent vanilla oversight (the cultures
//! of those names exist, their colors do not).

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::context::{Fallback, StructSpec};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

const NAMED_COLORS_DIR: &str = "common/named_colors/";
const COA_DIR: &str = "common/coat_of_arms/";

/// A `key = X` named-color reference gated to one directory.
const fn color_in(dir: &'static str, key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(dir),
        alt: &[],
    }
}

/// The body of the top-level `colors = { … }` container: every block-valued
/// key inside is a named color definition, and its body is a color literal.
static NAMED_COLORS: StructSpec = StructSpec {
    name: "named colors",
    fields: &[],
    fallback: Fallback::Color,
};

pub(crate) struct NamedColor;

impl Entity for NamedColor {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::NAMED_COLOR,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: NAMED_COLORS_DIR,
            shape: DefShape::ChildrenOf {
                containers: &["colors"],
            },
        }),
        refs: &[
            color_in("common/culture/", "color"),
            color_in("common/religion/", "color"),
            color_in(COA_DIR, "color1"),
            color_in(COA_DIR, "color2"),
            color_in(COA_DIR, "color3"),
            color_in(COA_DIR, "color4"),
            color_in(COA_DIR, "color5"),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(
        // The root context is the body of each top-level block — here the
        // `colors` container itself.
        NAMED_COLORS_DIR,
        ClauseKind::Struct(&NAMED_COLORS),
    )];
}
