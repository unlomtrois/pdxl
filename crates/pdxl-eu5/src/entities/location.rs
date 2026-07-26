//! EU5 map geography. Named locations are scalar definitions in
//! `map_data/named_locations/`; `map_data/definitions.txt` then groups those
//! locations into a fixed five-level hierarchy.

use crate::kinds;
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern, RefRule,
};

use super::Entity;

pub(crate) const NAMED_LOCATIONS_DIR: &str = "in_game/map_data/named_locations/";
pub(crate) const MAP_DEFINITIONS: &str = "in_game/map_data/definitions.txt";

pub(crate) struct Location;

impl Entity for Location {
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::LOCATION,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::PROVINCE,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::AREA,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::REGION,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::SUB_CONTINENT,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::CONTINENT,
            suffix: "",
        },
    ];

    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::LOCATION,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: NAMED_LOCATIONS_DIR,
                shape: DefShape::TopLevelValued,
            }),
            refs: &[
                RefRule {
                    pattern: RefPattern::AllScalarValues,
                    gate: Some(MAP_DEFINITIONS),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue("birth_place"),
                    gate: Some(super::setup_manager::START_SETUP_DIR),
                    alt: &[],
                },
            ],
            aliases: &[],
        },
        hierarchy(kinds::CONTINENT, 0),
        hierarchy(kinds::SUB_CONTINENT, 1),
        hierarchy(kinds::REGION, 2),
        hierarchy(kinds::AREA, 3),
        hierarchy(kinds::PROVINCE, 4),
    ];
}

const fn hierarchy(kind: pdxl_analysis::KindId, depth: u8) -> KindSpec {
    KindSpec {
        kind,
        icon: IconHint::Hierarchy,
        defs: Some(DefSource {
            dir_prefix: MAP_DEFINITIONS,
            shape: DefShape::BlocksAtDepth { depth },
        }),
        refs: &[],
        aliases: &[],
    }
}
