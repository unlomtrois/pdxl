//! Structural fragments shared across EU5 entities.

use pdxl_analysis::context::ClauseKind::{self, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar_or_block};

/// An opaque block whose contents are not modeled.
pub(crate) static OPAQUE: StructSpec = StructSpec {
    name: "opaque",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// A scaled, triggered modifier block — the recurring EU5 shape
/// (`country_modifier`, advance `modifier_while_progressing`, …):
/// `potential_trigger` gates it, `scale` scales it, every other key is a
/// modifier tag.
pub(crate) static SCALED_MODIFIER: StructSpec = StructSpec {
    name: "scaled modifier",
    fields: &[
        (
            "potential_trigger",
            block(Trigger).doc("Conditions for the modifier to apply."),
        ),
        (
            "scale",
            scalar_or_block(Setting, ClauseKind::ScriptValue)
                .doc("Scale factor for the modifiers (a maths/script value)."),
        ),
    ],
    fallback: Fallback::Modifier,
};
