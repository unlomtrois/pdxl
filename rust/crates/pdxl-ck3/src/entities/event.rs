//! Events (`events/`) — schema row (event/loc references) plus the rich
//! `_events.info` structural context (option, portraits, widgets, overrides).

use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, Effect, ScriptValue, ScriptedModifier, Trigger,
};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting, Target};
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, SymbolKind};

use super::Entity;
use super::common::{DURATION, OPAQUE, TRIGGERED_ASSET, anywhere, in_on_action};

static TRIGGERED_ANIMATION: StructSpec = StructSpec {
    name: "triggered_animation",
    fields: &[
        ("trigger", block(Trigger)),
        ("animation", scalar(Setting)),
        ("scripted_animation", scalar(Setting)),
        ("camera", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

static TRIGGERED_OUTFIT: StructSpec = StructSpec {
    name: "triggered_outfit",
    fields: &[
        ("trigger", block(Trigger)),
        (
            "outfit_tags",
            scalar_or_block(Setting, ClauseKind::Struct(&OPAQUE)),
        ),
        ("remove_default_outfit", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

/// The block form of `left_portrait = { … }` et al. Its `trigger` runs in
/// the *portrait character's* scope (layer-3 note in the design doc).
static PORTRAIT: StructSpec = StructSpec {
    name: "portrait",
    fields: &[
        ("character", scalar(Target)),
        ("trigger", block(Trigger)),
        ("animation", scalar(Setting)),
        ("scripted_animation", scalar(Setting)),
        (
            "triggered_animation",
            block(ClauseKind::Struct(&TRIGGERED_ANIMATION)),
        ),
        (
            "triggered_outfit",
            block(ClauseKind::Struct(&TRIGGERED_OUTFIT)),
        ),
        ("camera", scalar(Setting)),
        ("override_imprisonment_visuals", scalar(Setting)),
        ("animate_if_dead", scalar(Setting)),
        (
            "outfit_tags",
            scalar_or_block(Setting, ClauseKind::Struct(&OPAQUE)),
        ),
        ("remove_default_outfit", scalar(Setting)),
        ("hide_info", scalar(Setting)),
    ],
    fallback: Fallback::Deny,
};

static ARTIFACT: StructSpec = StructSpec {
    name: "artifact",
    fields: &[
        ("target", scalar(Target)),
        ("position", scalar(Setting)),
        ("trigger", block(Trigger)),
    ],
    fallback: Fallback::Deny,
};

static COURT_SCENE: StructSpec = StructSpec {
    name: "court_scene",
    fields: &[
        ("button_position_character", scalar(Target)),
        ("court_owner", scalar(Target)),
        ("court_event_force_open", scalar(Setting)),
        ("show_timeout_info", scalar(Setting)),
        ("should_pause_time", scalar(Setting)),
        // Role keys are dynamic (`scope:x = { role = … }`).
        ("roles", block(ClauseKind::Struct(&OPAQUE))),
    ],
    fallback: Fallback::Deny,
};

static WIDGET: StructSpec = StructSpec {
    name: "widget",
    fields: &[
        ("is_shown", block(Trigger)),
        ("gui", scalar(Setting)),
        ("container", scalar(Setting)),
        (
            "controller",
            scalar_or_block(Setting, ClauseKind::Struct(&OPAQUE)),
        ),
        ("setup_scope", block(Effect)),
    ],
    fallback: Fallback::Deny,
};

static WIDGETS: StructSpec = StructSpec {
    name: "widgets",
    fields: &[("widget", block(ClauseKind::Struct(&WIDGET)))],
    fallback: Fallback::Deny,
};

/// `name = { text = … trigger = { … } }` — the gated option-name candidate.
static OPTION_NAME: StructSpec = StructSpec {
    name: "option_name",
    fields: &[
        ("text", scalar_or_block(LocKey, DynamicDesc)),
        ("trigger", block(Trigger)),
    ],
    fallback: Fallback::Deny,
};

/// The event option: known structural fields, and — the key finding from
/// `_events.info` — **every unknown key is an inline effect**.
static OPTION: StructSpec = StructSpec {
    name: "option",
    fields: &[
        (
            "name",
            scalar_or_block(LocKey, ClauseKind::Struct(&OPTION_NAME))
                .doc("The option button text (a localization key). Block form picks the text by trigger."),
        ),
        (
            "trigger",
            block(Trigger).doc("Conditions required for this option to be shown at all."),
        ),
        (
            "show_as_unavailable",
            block(Trigger)
                .doc("When these conditions pass, the option is shown but disabled (greyed out)."),
        ),
        (
            "highlight_portrait",
            scalar(Target)
                .doc("A character whose portrait is highlighted while this option is hovered."),
        ),
        (
            "reason",
            scalar(Setting)
                .doc("Localization key for the reason shown when the option is unavailable."),
        ),
        (
            "skill",
            scalar(Setting).doc("A skill (diplomacy, martial, …) whose icon is shown on the option."),
        ),
        (
            "trait",
            scalar(Setting).doc("A trait whose icon is shown on the option."),
        ),
        (
            "show_unlock_reason",
            scalar(Setting).doc("Whether the unlock-reason UI is shown for this option."),
        ),
        (
            "is_cancel_option",
            scalar(Setting)
                .doc("Marks a cancel / back-out style option (used by some widgets and controllers)."),
        ),
        (
            "clicksound",
            scalar(Setting).doc("Sound effect played when the option is clicked."),
        ),
        (
            "fallback",
            scalar(Setting)
                .doc("If yes, this option is auto-selected when no other option's trigger passes."),
        ),
        (
            "exclusive",
            scalar(Setting)
                .doc("If any exclusive option is valid, non-exclusive options are ignored."),
        ),
        (
            "ai_chance",
            block(ScriptedModifier)
                .doc("Weighted-modifier block for the AI's chance to pick this option."),
        ),
        (
            "ai_will_select",
            block(ScriptValue).doc("Script value for the AI's chance to pick this option."),
        ),
        (
            "custom_tooltip",
            scalar(LocKey).doc("A localization key added as an extra tooltip line on the option."),
        ),
    ],
    fallback: Fallback::Effect,
};

static EVENT: StructSpec = StructSpec {
    name: "event",
    fields: &[
        ("type", scalar(Setting)),
        ("scope", scalar(Setting)),
        ("window", scalar(Setting)),
        ("hidden", scalar(Setting)),
        ("major", scalar(Setting)),
        ("orphan", scalar(Setting)),
        ("content_source", scalar(Setting)),
        ("theme", scalar(Setting)),
        ("title", scalar_or_block(LocKey, DynamicDesc)),
        ("desc", scalar_or_block(LocKey, DynamicDesc)),
        ("opening", scalar_or_block(LocKey, DynamicDesc)),
        ("trigger", block(Trigger)),
        ("major_trigger", block(Trigger)),
        ("immediate", block(Effect)),
        ("after", block(Effect)),
        ("on_trigger_fail", block(Effect)),
        ("cooldown", block(ClauseKind::Struct(&DURATION))),
        (
            "left_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "right_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "center_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "lower_left_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "lower_center_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "lower_right_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        (
            "sender",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)),
        ),
        ("artifact", block(ClauseKind::Struct(&ARTIFACT))),
        ("court_scene", block(ClauseKind::Struct(&COURT_SCENE))),
        ("widgets", block(ClauseKind::Struct(&WIDGETS))),
        ("widget", block(ClauseKind::Struct(&WIDGET))),
        ("option", block(ClauseKind::Struct(&OPTION))),
        (
            // Scalar shorthand `override_background = key` or the full
            // `{ trigger reference … }` asset block.
            "override_background",
            scalar_or_block(Setting, ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        (
            "override_transition",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        (
            "override_effect_2d",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        ("override_icon", block(ClauseKind::Struct(&TRIGGERED_ASSET))),
        (
            "override_header_background",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
        (
            "override_sound",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Event;

impl Entity for Event {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::Event,
        icon: IconHint::Event,
        defs: Some(DefSource {
            dir_prefix: "events/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            // Scalar form: trigger_event = ns.id.
            anywhere(RefPattern::KeyValue("trigger_event")),
            // Block form: trigger_event = { id = ns.id … }.
            anywhere(RefPattern::KeyBlockField("trigger_event", "id")),
            // on_action lists: events = { ns.id … } (ambiguous elsewhere).
            in_on_action(RefPattern::KeyList("events")),
            in_on_action(RefPattern::KeyList("first_valid")),
            // on_action weighted blocks: random_events = { 50 = ns.id … }.
            in_on_action(RefPattern::KeyWeighted("random_events")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[("events/", ClauseKind::Struct(&EVENT))];
}
