//! The universal Jomini script kinds: scripted triggers/effects (defs +
//! project-level call-by-name refs), script values, and namespaced events.

use crate::kinds;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec};

use super::Entity;

/// One definition-only kind row.
pub(crate) const fn def_only(
    kind: pdxl_analysis::KindId,
    icon: IconHint,
    dir: &'static str,
) -> KindSpec {
    KindSpec {
        kind,
        icon,
        defs: Some(DefSource {
            dir_prefix: dir,
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    }
}

pub(crate) struct Scripted;

impl Entity for Scripted {
    const KINDS: &'static [KindSpec] = &[
        def_only(
            kinds::SCRIPTED_TRIGGER,
            IconHint::Function,
            "in_game/common/scripted_triggers/",
        ),
        def_only(
            kinds::SCRIPTED_EFFECT,
            IconHint::Function,
            "in_game/common/scripted_effects/",
        ),
        KindSpec {
            kind: kinds::SCRIPT_VALUE,
            icon: IconHint::Function,
            defs: Some(DefSource {
                dir_prefix: "in_game/common/script_values/",
                shape: DefShape::TopLevelValued,
            }),
            refs: &[],
            aliases: &[],
        },
        def_only(kinds::EVENT, IconHint::Event, "in_game/events/"),
        KindSpec {
            kind: kinds::NAMESPACE,
            icon: IconHint::Tag,
            defs: None,
            refs: &[],
            aliases: &[],
        },
    ];
}
