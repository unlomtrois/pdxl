//! Scripted logic: scripted triggers/effects and script values (defined
//! symbols) plus the scripted-modifier directory (structural root only — it
//! defines no cross-referenced symbol kind).
//!
//! References to scripted effects/triggers/values are **name-gated**, not
//! fixed-keyword: an effect/trigger call's field *key* is the name
//! (`my_effect = yes`); a script value appears by name in any *value* position
//! (`add_stress = minor_stress_gain`). Neither fits a `RefPattern`, so both are
//! recognized during extraction against the project's defined-name sets and
//! stored in `FileFacts.calls` (never diagnosed — see `pdxl_analysis::extract`
//! and `CallTargets`). Hence `refs` below stays empty.

use pdxl_analysis::context::ClauseKind::{self, Effect, ScriptValue, ScriptedModifier, Trigger};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, SymbolKind};

use super::Entity;

pub(crate) struct Scripted;

impl Entity for Scripted {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: SymbolKind::ScriptedTrigger,
            icon: IconHint::Function,
            defs: Some(DefSource {
                dir_prefix: "common/scripted_triggers/",
                shape: DefShape::TopLevel,
            }),
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: SymbolKind::ScriptedEffect,
            icon: IconHint::Function,
            defs: Some(DefSource {
                dir_prefix: "common/scripted_effects/",
                shape: DefShape::TopLevel,
            }),
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: SymbolKind::ScriptValue,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: "common/script_values/",
                // Scalar (`x = 10`) and block (`x = { … }`) forms are both defs.
                shape: DefShape::TopLevelValued,
            }),
            refs: &[],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        ("common/scripted_effects/", Effect),
        ("common/scripted_triggers/", Trigger),
        ("common/script_values/", ScriptValue),
        ("common/scripted_modifiers/", ScriptedModifier),
    ];
}
