//! Decisions (`common/decisions/`) — schema row plus the `_decisions.info`
//! structural context.

use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, Effect, ScriptValue, ScriptedModifier, Trigger,
};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, SymbolKind};

use super::Entity;
use super::common::{COST, OPAQUE, TRIGGERED_ASSET};

static DECISION_ITEM: StructSpec = StructSpec {
    name: "decision_widget_item",
    fields: &[
        ("value", scalar(Setting)),
        ("is_shown", block(Trigger)),
        ("is_valid", block(Trigger)),
        ("current_description", scalar_or_block(LocKey, DynamicDesc)),
        ("localization", scalar(LocKey)),
        ("is_default", scalar(Setting)),
        ("icon", scalar(Setting)),
        ("flat", scalar(Setting)),
        ("ai_chance", scalar_or_block(Setting, ScriptValue)),
    ],
    fallback: Fallback::Deny,
};

static DECISION_WIDGET: StructSpec = StructSpec {
    name: "decision_widget",
    fields: &[
        ("gui", scalar(Setting)),
        ("controller", scalar(Setting)),
        ("show_from_start", scalar(Setting)),
        ("item", block(ClauseKind::Struct(&DECISION_ITEM))),
    ],
    fallback: Fallback::Deny,
};

static DECISION: StructSpec = StructSpec {
    name: "decision",
    fields: &[
        ("title", scalar_or_block(LocKey, DynamicDesc)),
        ("desc", scalar_or_block(LocKey, DynamicDesc)),
        ("selection_tooltip", scalar_or_block(LocKey, DynamicDesc)),
        ("confirm_text", scalar_or_block(LocKey, DynamicDesc)),
        (
            "picture",
            scalar_or_block(Setting, ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        ("extra_picture", scalar(Setting)),
        ("decision_group_type", scalar(Setting)),
        ("major", scalar(Setting)),
        ("sort_order", scalar(Setting)),
        ("progress", scalar_or_block(Setting, ScriptValue)),
        ("advice", block(ClauseKind::Struct(&OPAQUE))),
        ("ai_goal", scalar(Setting)),
        ("ai_check_interval", scalar(Setting)),
        (
            "ai_check_interval_by_tier",
            block(ClauseKind::Struct(&OPAQUE)),
        ),
        ("is_shown", block(Trigger)),
        ("is_valid", block(Trigger)),
        ("is_valid_showing_failures_only", block(Trigger)),
        ("should_create_alert", block(Trigger)),
        ("cost", block(ClauseKind::Struct(&COST))),
        ("minimum_cost", block(ClauseKind::Struct(&COST))),
        ("effect", block(Effect)),
        ("ai_potential", block(Trigger)),
        ("ai_will_do", block(ScriptedModifier)),
        (
            "widget",
            scalar_or_block(Setting, ClauseKind::Struct(&DECISION_WIDGET)),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Decision;

impl Entity for Decision {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::Decision,
        icon: IconHint::Action,
        defs: Some(DefSource {
            dir_prefix: "common/decisions/",
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[("common/decisions/", ClauseKind::Struct(&DECISION))];
}
