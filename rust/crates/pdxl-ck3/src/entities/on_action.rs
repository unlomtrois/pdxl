//! On-actions (`common/on_action/`) — schema row (fire-list references) plus
//! the `_on_actions.info` structural context.

use pdxl_analysis::context::ClauseKind::{self, Effect, ScriptValue, ScriptedModifier, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, SymbolKind};

use super::Entity;
use super::common::{DURATION, anywhere, in_on_action};

/// `events = { id delay = { … } id }` — loose items are event/on_action ids.
static FIRE_LIST: StructSpec = StructSpec {
    name: "fire_list",
    fields: &[("delay", block(ClauseKind::Struct(&DURATION)))],
    fallback: Fallback::Ignore,
};

/// `random_events = { chance_to_happen = 25  100 = id }` — weight keys are
/// dynamic numbers.
static WEIGHTED_FIRE_LIST: StructSpec = StructSpec {
    name: "weighted_fire_list",
    fields: &[
        ("chance_to_happen", scalar(Setting)),
        ("chance_of_no_event", scalar_or_block(Setting, ScriptValue)),
        ("delay", block(ClauseKind::Struct(&DURATION))),
    ],
    fallback: Fallback::Ignore,
};

static ON_ACTION: StructSpec = StructSpec {
    name: "on_action",
    fields: &[
        ("trigger", block(Trigger)),
        ("weight_multiplier", block(ScriptedModifier)),
        ("events", block(ClauseKind::Struct(&FIRE_LIST))),
        ("first_valid", block(ClauseKind::Struct(&FIRE_LIST))),
        ("on_actions", block(ClauseKind::Struct(&FIRE_LIST))),
        (
            "first_valid_on_action",
            block(ClauseKind::Struct(&FIRE_LIST)),
        ),
        (
            "random_events",
            block(ClauseKind::Struct(&WEIGHTED_FIRE_LIST)),
        ),
        (
            "random_on_actions",
            block(ClauseKind::Struct(&WEIGHTED_FIRE_LIST)),
        ),
        ("effect", block(Effect)),
        ("fallback", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct OnAction;

impl Entity for OnAction {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::OnAction,
        icon: IconHint::Event,
        defs: Some(DefSource {
            dir_prefix: "common/on_action/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            // Fire lists inside on_action files (`_on_actions.info`).
            in_on_action(RefPattern::KeyList("on_actions")),
            in_on_action(RefPattern::KeyList("first_valid_on_action")),
            in_on_action(RefPattern::KeyWeighted("random_on_actions")),
            // `fallback = another_on_action` — runs if nothing else fired.
            in_on_action(RefPattern::KeyValue("fallback")),
            // Script can fire an on_action from anywhere:
            // trigger_event = { on_action = X }.
            anywhere(RefPattern::KeyBlockField("trigger_event", "on_action")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[("common/on_action/", ClauseKind::Struct(&ON_ACTION))];
}
