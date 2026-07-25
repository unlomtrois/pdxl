//! EU5 game concepts, one module per domain — the same schema-scaling
//! pattern as `pdxl-ck3` (see `docs/SCHEMA-SCALING.md`): each entity
//! co-locates its `KindSpec` rows and its structural-context roots, and the
//! registry below assembles them.

use pdxl_analysis::KindSpec;
use pdxl_analysis::context::ClauseKind;

mod country;
mod culture;
mod named_color;
mod religion;
mod scripted;

/// The uniform surface every game concept declares (see `pdxl-ck3`).
pub(crate) trait Entity {
    const KINDS: &'static [KindSpec] = &[];
    const ROOTS: &'static [(&'static str, ClauseKind)] = &[];
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
    };
}

registry!(
    scripted::Scripted,
    country::Country,
    culture::Culture,
    religion::Religion,
    named_color::NamedColor,
);
