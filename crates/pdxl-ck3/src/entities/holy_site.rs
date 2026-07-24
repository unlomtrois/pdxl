//! Holy sites (`common/religion/holy_site_types/`, from
//! `_holy_site_types.info`) — top-level definitions (322 in vanilla),
//! referenced by the `holy_site = X` entries of faith bodies (716 refs,
//! corpus-validated at 0 unresolved; ungated — the key has no other
//! meaning). The `county` / `barony` title references live in
//! [`super::title`].

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, StaticModifier, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{OPAQUE, anywhere};

pub(crate) const HOLY_SITES_DIR: &str = "common/religion/holy_site_types/";

/// The body of one holy site (`_holy_site_types.info`).
static HOLY_SITE: StructSpec = StructSpec {
    name: "holy site",
    fields: &[
        (
            "county",
            scalar(Setting).doc("The county the holy site is in."),
        ),
        (
            "barony",
            scalar(Setting).doc("Optional barony override within the county."),
        ),
        (
            "character_modifier",
            block(StaticModifier).doc(
                "Modifier applied to characters whose faith controls the site (an optional \
                 `name = <effect loc>` labels it).",
            ),
        ),
        (
            "parameters",
            block(Struct(&OPAQUE)).doc(
                "Arbitrary parameters checked by script (`has_holy_site_parameter` / \
                 `controls_holy_site_with_parameter`); localized as \
                 `holy_site_parameter_<name>`.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct HolySite;

impl Entity for HolySite {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::HOLY_SITE,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: HOLY_SITES_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[anywhere(RefPattern::KeyValue("holy_site"))],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(HOLY_SITES_DIR, ClauseKind::Struct(&HOLY_SITE))];
}
