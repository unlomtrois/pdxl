//! EU5 game concepts, one module per domain — the same schema-scaling
//! pattern as `pdxl-ck3` (see `docs/SCHEMA-SCALING.md`): each entity
//! co-locates its `KindSpec` rows and its structural-context roots, and the
//! registry below assembles them.

use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::{ImplicitLocPattern, KindSpec};

mod advance;
mod bias;
mod coat_of_arms;
pub(crate) mod common;
mod country;
mod culture;
mod custom_loc;
mod define;
mod estate;
mod game_concept;
mod government_reform;
mod international_organization;
mod named_color;
mod parliament_type;
mod religion;
mod scripted;
mod subject_type;
mod unlocks;

/// The uniform surface every game concept declares (see `pdxl-ck3`).
pub(crate) trait Entity {
    const KINDS: &'static [KindSpec] = &[];
    const ROOTS: &'static [(&'static str, ClauseKind)] = &[];
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[];
    const LOC_DATAFN_ARG_REFS: &'static [(&'static str, pdxl_analysis::KindId)] = &[];
}

macro_rules! registry {
    ($($e:ty),+ $(,)?) => {
        pub(crate) fn kinds() -> Vec<KindSpec> {
            let mut v = Vec::new();
            $( v.extend_from_slice(<$e as Entity>::KINDS); )+
            v
        }
        pub(crate) fn roots() -> Vec<(&'static str, ClauseKind)> {
            let mut v = Vec::new();
            $( v.extend_from_slice(<$e as Entity>::ROOTS); )+
            v
        }
        pub(crate) fn implicit_loc_patterns() -> Vec<ImplicitLocPattern> {
            let mut v = Vec::new();
            $( v.extend_from_slice(<$e as Entity>::IMPLICIT_LOC); )+
            v
        }
        pub(crate) fn loc_datafn_arg_refs() -> Vec<(&'static str, pdxl_analysis::KindId)> {
            let mut v = Vec::new();
            $( v.extend_from_slice(<$e as Entity>::LOC_DATAFN_ARG_REFS); )+
            v
        }
    };
}

registry!(
    scripted::Scripted,
    country::Country,
    custom_loc::CustomLoc,
    define::Define,
    advance::Advance,
    bias::Bias,
    unlocks::Unlocks,
    coat_of_arms::CoatOfArms,
    estate::Estate,
    game_concept::GameConcept,
    subject_type::SubjectType,
    government_reform::GovernmentReform,
    international_organization::InternationalOrganization,
    parliament_type::ParliamentType,
    culture::Culture,
    religion::Religion,
    named_color::NamedColor,
);
