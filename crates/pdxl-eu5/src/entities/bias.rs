//! Bias modifiers (`in_game/common/biases/`): opinion, trust, and antagonism
//! values. Vanilla defines 1,140 flat top-level entries. Their bodies use only
//! the value, duration/decay, and clamp fields below.
//!
//! References are intentionally limited to `modifier` fields directly under
//! the engine clauses which consume biases. A script-wide `modifier = X` rule
//! would collide with static modifiers and many unrelated modifier concepts.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::{Fallback, FieldSpec, ScalarKind, StructSpec};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

pub(crate) const BIASES_DIR: &str = "in_game/common/biases/";

const fn setting(doc: &'static str) -> FieldSpec {
    FieldSpec {
        scalar: Some(ScalarKind::Setting),
        block: None,
        scope: None,
        doc: Some(doc),
        values: None,
        ref_kind: None,
        ref_alt: &[],
    }
}

static BIAS: StructSpec = StructSpec {
    name: "bias",
    fields: &[
        ("value", setting("Base value of the bias.")),
        (
            "yearly_decay",
            setting("Amount removed from the bias each year."),
        ),
        (
            "yearly_gain",
            setting("Amount added to the bias each year."),
        ),
        ("min", setting("Minimum cumulative value.")),
        ("max", setting("Maximum cumulative value.")),
        ("months", setting("Duration in months before removal.")),
        ("years", setting("Duration in years before removal.")),
    ],
    fallback: Fallback::Deny,
};

const fn modifier_under(parent: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValueUnder(parent, "modifier"),
        gate: None,
        alt: &[],
    }
}

pub(crate) struct Bias;

impl Entity for Bias {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::BIAS,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: BIASES_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            modifier_under("add_opinion"),
            modifier_under("remove_opinion"),
            modifier_under("reverse_add_opinion"),
            modifier_under("add_antagonism"),
            modifier_under("remove_antagonism"),
            modifier_under("reverse_add_antagonism"),
            modifier_under("add_trust_equilibrium"),
            modifier_under("remove_trust_equilibrium"),
            modifier_under("reverse_add_trust_equilibrium"),
            modifier_under("drop_antagonism_bomb"),
            modifier_under("bias_value"),
            modifier_under("has_opinion"),
            modifier_under("has_trust"),
            modifier_under("has_antagonism"),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(BIASES_DIR, Struct(&BIAS))];
}
