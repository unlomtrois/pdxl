//! Dynasties (`common/dynasties/`) and dynasty houses
//! (`common/dynasty_houses/`) — numeric-ID and `house_*` top-level
//! definitions. Referenced by history characters (`dynasty = 101046`,
//! `dynasty_house = house_chiny`) and by houses naming their parent dynasty
//! (`dynasty = 25061`). All corpus-validated at 0 unresolved.

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

const DYNASTIES_DIR: &str = "common/dynasties/";
const HOUSES_DIR: &str = "common/dynasty_houses/";
const CHARACTERS_DIR: &str = "history/characters/";

/// A `key = X` reference gated to one directory.
const fn in_dir(dir: &'static str, key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(dir),
    }
}

/// The body of one dynasty definition.
static DYNASTY: StructSpec = StructSpec {
    name: "dynasty",
    fields: &[
        (
            "name",
            scalar(LocKey).doc("The dynasty name (a `dynn_*` loc key)."),
        ),
        (
            "prefix",
            scalar(LocKey).doc("An optional name prefix (a `dynnp_*` loc key, e.g. `de`)."),
        ),
        ("culture", scalar(Setting).doc("The dynasty's culture.")),
        ("motto", scalar(LocKey).doc("An optional dynasty motto.")),
        (
            "forced_coa_religiongroup",
            scalar(Setting)
                .doc("Force coat-of-arms generation to use this religion group's style."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one dynasty-house definition.
static DYNASTY_HOUSE: StructSpec = StructSpec {
    name: "dynasty_house",
    fields: &[
        (
            "name",
            scalar(LocKey).doc("The house name (a `dynn_*` loc key)."),
        ),
        (
            "prefix",
            scalar(LocKey).doc("An optional name prefix (a `dynnp_*` loc key)."),
        ),
        (
            "dynasty",
            scalar(Setting).doc("The parent dynasty this house cadets from."),
        ),
        ("motto", scalar(LocKey).doc("An optional house motto.")),
        (
            "forced_coa_religiongroup",
            scalar(Setting)
                .doc("Force coat-of-arms generation to use this religion group's style."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Dynasty;

impl Entity for Dynasty {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::DYNASTY,
            icon: IconHint::Hierarchy,
            defs: Some(DefSource {
                dir_prefix: DYNASTIES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                in_dir(CHARACTERS_DIR, "dynasty"),
                in_dir(HOUSES_DIR, "dynasty"),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::DYNASTY_HOUSE,
            icon: IconHint::Hierarchy,
            defs: Some(DefSource {
                dir_prefix: HOUSES_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[in_dir(CHARACTERS_DIR, "dynasty_house")],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (DYNASTIES_DIR, ClauseKind::Struct(&DYNASTY)),
        (HOUSES_DIR, ClauseKind::Struct(&DYNASTY_HOUSE)),
    ];
}
