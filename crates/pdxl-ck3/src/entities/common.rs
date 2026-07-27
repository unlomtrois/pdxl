//! Fragments shared across CK3 entities: reference-rule helpers, the
//! on_action gate prefix, and structural blocks reused by several concepts
//! (cost, duration, opaque payloads, triggered assets).

use pdxl_analysis::context::{
    ClauseKind, Fallback, ScalarKind, StructSpec, block, scalar, scalar_or_block,
};
use pdxl_analysis::{RefPattern, RefRule};

use ClauseKind::{ScriptValue, Trigger};
use ScalarKind::Setting;

/// The file prefix that gates the on_action list/weighted reference rules —
/// those shapes are ambiguous elsewhere (Go: `OnActionDir`).
pub(crate) const ON_ACTION_DIR: &str = "common/on_action/";

/// An ungated reference rule (applies in every file).
pub(crate) const fn anywhere(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: None,
        alt: &[],
    }
}

/// A reference rule gated to on_action files.
pub(crate) const fn in_on_action(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(ON_ACTION_DIR),
        alt: &[],
    }
}

/// A block whose contents we don't model (controller payloads, role maps).
pub(crate) static OPAQUE: StructSpec = StructSpec {
    name: "opaque",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// `trigger` + `reference` blocks (`picture`, every event `override_*`).
pub(crate) static TRIGGERED_ASSET: StructSpec = StructSpec {
    name: "triggered_asset",
    fields: &[
        ("trigger", block(Trigger)),
        ("reference", scalar(Setting)),
        ("soundeffect", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// `days/weeks/months/years = <script value>` (cooldowns, delays).
pub(crate) static DURATION: StructSpec = StructSpec {
    name: "duration",
    fields: &[
        ("days", scalar_or_block(Setting, ScriptValue)),
        ("weeks", scalar_or_block(Setting, ScriptValue)),
        ("months", scalar_or_block(Setting, ScriptValue)),
        ("years", scalar_or_block(Setting, ScriptValue)),
    ],
    fallback: Fallback::Deny,
};

/// A scripted cost — decisions, laws, casus belli, interactions, great
/// projects. Deducted from the actor when the action is taken, and the action
/// is disabled if they cannot pay.
///
/// The currency list is corpus-wide, not per-consumer: an earlier version held
/// only gold/piety/prestige, so every other currency fell through
/// `Fallback::Deny` into an unknown context and stopped hovering. Counts across
/// the modeled directories: piety 131, treasury_or_gold 123, prestige 122,
/// gold 98, treasury 46, influence 8, herd 1.
pub(crate) static COST: StructSpec = StructSpec {
    name: "cost",
    fields: &[
        ("gold", scalar_or_block(Setting, ScriptValue)),
        ("piety", scalar_or_block(Setting, ScriptValue)),
        ("prestige", scalar_or_block(Setting, ScriptValue)),
        (
            "renown",
            scalar_or_block(Setting, ScriptValue)
                .doc("Dynasty renown — only the dynast may spend it."),
        ),
        (
            "influence",
            scalar_or_block(Setting, ScriptValue).doc("Administrative influence."),
        ),
        (
            "treasury",
            scalar_or_block(Setting, ScriptValue).doc("Domicile treasury."),
        ),
        (
            "treasury_or_gold",
            scalar_or_block(Setting, ScriptValue)
                .doc("Pay from the treasury, falling back to personal gold."),
        ),
        (
            "herd",
            scalar_or_block(Setting, ScriptValue).doc("Herd, for nomadic rulers."),
        ),
    ],
    fallback: Fallback::Deny,
};
