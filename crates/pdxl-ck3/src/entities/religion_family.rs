//! Religion families (`common/religion/religion_family_types/`) — top-level
//! definitions (4 in vanilla: abrahamic, eastern, pagan, zoroastrian_group),
//! referenced by the `family = X` attribute of religions (49 refs,
//! corpus-validated at 0 unresolved, gated to `common/religion/`). The
//! `hostility_doctrine` reference lives in [`super::doctrine`].

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

const FAMILIES_DIR: &str = "common/religion/religion_family_types/";

/// The body of one religion family (corpus-derived).
static RELIGION_FAMILY: StructSpec = StructSpec {
    name: "religion family",
    fields: &[
        (
            "graphical_faith",
            scalar(Setting).doc("Default 3D-model set for the family's religions."),
        ),
        (
            "piety_icon_group",
            scalar(Setting).doc("Default piety icon set for the family's religions."),
        ),
        (
            "doctrine_background_icon",
            scalar(Setting).doc("Default doctrine background icon."),
        ),
        (
            "hostility_doctrine",
            scalar(Setting).doc("The hostility doctrine governing the family's faith relations."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct ReligionFamily;

impl Entity for ReligionFamily {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::RELIGION_FAMILY,
        icon: IconHint::Hierarchy,
        defs: Some(DefSource {
            dir_prefix: FAMILIES_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[RefRule {
            pattern: RefPattern::KeyValue("family"),
            gate: Some("common/religion/"),
            alt: &[],
        }],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(FAMILIES_DIR, ClauseKind::Struct(&RELIGION_FAMILY))];
}
