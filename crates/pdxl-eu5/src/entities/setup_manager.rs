//! Start-scenario setup managers (`main_menu/setup/start/`). Managers are
//! top-level engine-owned blocks. This first slice models
//! `institution_manager` from `02_core.txt`; the shared root deliberately
//! accepts fields belonging to other managers until they are added.

use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};

use super::Entity;

pub(crate) const START_SETUP_DIR: &str = "main_menu/setup/start/";

static INSTITUTION_SETUP: StructSpec = StructSpec {
    name: "starting institution",
    fields: &[
        (
            "active",
            scalar(Setting)
                .doc("Whether the institution is active at scenario start.")
                .values(&["yes", "no"]),
        ),
        (
            "birth_place",
            scalar(Setting).doc("Location where the institution originated."),
        ),
    ],
    fallback: Fallback::Deny,
};

static INSTITUTIONS: StructSpec = StructSpec {
    name: "starting institutions",
    fields: &[],
    fallback: Fallback::Struct(&INSTITUTION_SETUP),
};

static RELATIONS: StructSpec = StructSpec {
    name: "religious school relations",
    fields: &[],
    // Dynamic religious-school keys with `kindred`/`enemy` values.
    fallback: Fallback::Ignore,
};

static RELIGIOUS_SCHOOL_SETUP: StructSpec = StructSpec {
    name: "starting religious school",
    fields: &[(
        "relation",
        block(Struct(&RELATIONS)).doc(
            "Relations keyed by another religious school; values include `kindred` and `enemy`.",
        ),
    )],
    // The engine also accepts direct `other_school = kindred/enemy` entries.
    fallback: Fallback::Ignore,
};

static SETUP_MANAGER: StructSpec = StructSpec {
    name: "setup manager",
    fields: &[(
        "institutions",
        block(Struct(&INSTITUTIONS)).doc(
            "Institution initial state keyed by institution id. Keys resolve to institution definitions.",
        ),
    )],
    // `religion_manager` is itself keyed by religious-school id. Because all
    // managers share this directory root, unknown manager-body keys open its
    // school-entry structure; explicit fields above take precedence.
    fallback: Fallback::Struct(&RELIGIOUS_SCHOOL_SETUP),
};

pub(crate) struct SetupManager;

impl Entity for SetupManager {
    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(START_SETUP_DIR, ClauseKind::Struct(&SETUP_MANAGER))];
}
