//! Scripted logic: scripted triggers/effects (defined symbols) plus the
//! script-value and scripted-modifier directories (structural roots only —
//! they define no cross-referenced symbol kind).

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
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        ("common/scripted_effects/", Effect),
        ("common/scripted_triggers/", Trigger),
        ("common/script_values/", ScriptValue),
        ("common/scripted_modifiers/", ScriptedModifier),
    ];
}
