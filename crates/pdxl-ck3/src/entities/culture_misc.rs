//! Def-only culture-domain concepts: **aesthetics bundles**
//! (`common/culture/aesthetics_bundles/`), **creation names**
//! (`common/culture/creation_names/`) and **name equivalencies**
//! (`common/culture/name_equivalency/`).
//!
//! All three are corpus-validated def-only: nothing in script names them by
//! key. Aesthetics bundles are offered in the divergence UI, creation names
//! are matched by trigger in definition order, and equivalency keys are
//! consumed by the engine's name matching. (A bundle's `name_list = X` field
//! *is* a reference — to a name list; that rule lives in
//! [`super::name_list`].)

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, DynamicDesc, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec};

use super::Entity;
use super::common::{OPAQUE, toggle};

const AESTHETICS_BUNDLES_DIR: &str = "common/culture/aesthetics_bundles/";
const CREATION_NAMES_DIR: &str = "common/culture/creation_names/";
const NAME_EQUIVALENCY_DIR: &str = "common/culture/name_equivalency/";

/// Top-level `NAME = { … }` definitions in one culture subdirectory.
const fn defs(dir: &'static str) -> Option<DefSource> {
    Some(DefSource {
        dir_prefix: dir,
        shape: DefShape::TopLevel,
    })
}

/// The body of one aesthetics bundle (`_aesthetics_bundles.info`) — lets the
/// player change aesthetics when diverging their culture. The loc key
/// `<key>_name` must exist.
static AESTHETICS_BUNDLE: StructSpec = StructSpec {
    name: "aesthetics_bundle",
    fields: &[
        (
            "name_list",
            scalar(Setting).doc("The name list this bundle switches the culture to."),
        ),
        (
            "building_gfx",
            block(Struct(&OPAQUE)).doc("Building GFX set keys, as in a culture body."),
        ),
        (
            "clothing_gfx",
            block(Struct(&OPAQUE)).doc("Clothing GFX set keys, as in a culture body."),
        ),
        (
            "unit_gfx",
            block(Struct(&OPAQUE)).doc("Unit GFX set keys, as in a culture body."),
        ),
        (
            "coa_gfx",
            block(Struct(&OPAQUE)).doc("Coat-of-arms GFX set keys, as in a culture body."),
        ),
        (
            "is_shown",
            block(Trigger).doc(
                "Whether the bundle is shown when diverging. `root` is the diverging \
                 character's culture, `scope:character` the character, `scope:trait` a \
                 list of all selected culture traits.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one creation name (`_creation_names.info`) — candidate names
/// for new hybrid/divergent cultures, tried in definition order; when none
/// matches, the `HYBRID_NAME_FORMAT_<n>` / `DIVERGE_NAME_FORMAT_<n>` loc
/// formats take over. Only `name` is checked for uniqueness.
static CULTURE_CREATION_NAME: StructSpec = StructSpec {
    name: "culture_creation_name",
    fields: &[
        (
            "name",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Optional dynamic description; defaults to `<key>_name`."),
        ),
        (
            "collective_noun",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Optional dynamic description; defaults to `<key>_collective_noun`."),
        ),
        (
            "prefix",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("Optional dynamic description; defaults to `<key>_trigger`."),
        ),
        (
            "trigger",
            block(Trigger).doc(
                "`root` = the character creating the culture, `scope:culture` = their \
                 culture, `scope:other_culture` = the other culture (hybridization only).",
            ),
        ),
        (
            "hybrid",
            toggle("Is this name for hybridization? Defaults to `no`."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// A name equivalency (`name_equivalency/_info.info`) is a loose list of
/// equivalent names (`henrik_male = { "Henrik" "Heinrich" }`) — no fields.
/// Keys are arbitrary except a `_male`/`_female` suffix (no suffix = male).
static NAME_EQUIVALENCY: StructSpec = StructSpec {
    name: "name_equivalency",
    fields: &[],
    fallback: Fallback::Ignore,
};

pub(crate) struct CultureMisc;

impl Entity for CultureMisc {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::AESTHETICS_BUNDLE,
            icon: IconHint::Object,
            defs: defs(AESTHETICS_BUNDLES_DIR),
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::CULTURE_CREATION_NAME,
            icon: IconHint::Text,
            defs: defs(CREATION_NAMES_DIR),
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::NAME_EQUIVALENCY,
            icon: IconHint::Text,
            defs: defs(NAME_EQUIVALENCY_DIR),
            refs: &[],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (
            AESTHETICS_BUNDLES_DIR,
            ClauseKind::Struct(&AESTHETICS_BUNDLE),
        ),
        (
            CREATION_NAMES_DIR,
            ClauseKind::Struct(&CULTURE_CREATION_NAME),
        ),
        (NAME_EQUIVALENCY_DIR, ClauseKind::Struct(&NAME_EQUIVALENCY)),
    ];
}
