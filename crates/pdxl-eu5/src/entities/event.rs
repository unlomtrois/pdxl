//! EU5 events (`in_game/events/`), modeled from the directory readme.
//! Definitions are namespaced top-level blocks. Event invocations use
//! `trigger_event_silently/non_silently = { id = namespace.number }`.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Config, Effect, ScriptValue, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{
    Fallback, FieldSpec, StructSpec, block, block_scoped, scalar, scalar_or_block,
};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

pub(crate) const EVENTS_DIR: &str = "in_game/events/";

const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

static TRIGGERED_DESC: StructSpec = StructSpec {
    name: "triggered event text",
    fields: &[
        (
            "trigger",
            block(Trigger).doc("Condition for selecting this text."),
        ),
        (
            "desc",
            scalar(Setting).doc("Localization key selected when the trigger passes."),
        ),
    ],
    fallback: Fallback::Deny,
};

static TEXT_SELECTOR: StructSpec = StructSpec {
    name: "conditional event text",
    fields: &[
        (
            "first_valid",
            block(Struct(&TEXT_CHOICES)).doc("Use the first valid triggered description."),
        ),
        (
            "random_valid",
            block(Struct(&TEXT_CHOICES)).doc("Use a random valid triggered description."),
        ),
    ],
    fallback: Fallback::Deny,
};

static TEXT_CHOICES: StructSpec = StructSpec {
    name: "event text choices",
    fields: &[("triggered_desc", block(Struct(&TRIGGERED_DESC)))],
    fallback: Fallback::Deny,
};

static DYNAMIC_HISTORICAL_EVENT: StructSpec = StructSpec {
    name: "dynamic historical event window",
    fields: &[
        (
            "tag",
            scalar(Setting).doc("Country tag eligible for the historical event."),
        ),
        ("from", scalar(Setting).doc("Earliest eligible date.")),
        ("to", scalar(Setting).doc("Latest eligible date.")),
        (
            "monthly_chance",
            scalar(Setting).doc("Monthly firing chance inside the time window."),
        ),
    ],
    fallback: Fallback::Deny,
};

static OPTION: StructSpec = StructSpec {
    name: "event option",
    fields: &[
        (
            "name",
            scalar(Setting).doc("Option-label localization key."),
        ),
        (
            "historical_option",
            toggle("Highlight this as the historical option."),
        ),
        (
            "trigger",
            block(Trigger).doc("Whether this option is available."),
        ),
        ("fallback", toggle("Use when no other option is available.")),
        (
            "exclusive",
            toggle("Hide non-exclusive options while this option is available."),
        ),
        (
            "original_recipient_only",
            toggle("Only the country that triggered the event may select it."),
        ),
        ("moral_option", toggle("Marks the option as moral.")),
        ("evil_option", toggle("Marks the option as evil.")),
        ("high_risk_option", toggle("Marks the option as high risk.")),
        (
            "high_reward_option",
            toggle("Marks the option as high reward."),
        ),
        (
            "ai_will_select",
            block(ScriptValue).doc("Script-math AI selection weight; overrides `ai_chance`."),
        ),
        (
            "ai_chance",
            block(ScriptValue).doc("Legacy modifier-based AI selection weight."),
        ),
        (
            "show_as_unavailable",
            block(Trigger)
                .doc("Reserved unavailable-option trigger (not implemented by the engine)."),
        ),
    ],
    // Every other key in an option is an effect.
    fallback: Fallback::Effect,
};

static EVENT: StructSpec = StructSpec {
    name: "event",
    fields: &[
        (
            "type",
            scalar(Setting)
                .doc("Scope type in which the event runs.")
                .values(&[
                    "country_event",
                    "location_event",
                    "unit_event",
                    "exploration_event",
                    "age_event",
                ]),
        ),
        (
            "title",
            scalar_or_block(Setting, Struct(&TEXT_SELECTOR))
                .doc("Title localization key or conditional selector."),
        ),
        (
            "desc",
            scalar_or_block(Setting, Struct(&TEXT_SELECTOR))
                .doc("Description localization key or conditional selector."),
        ),
        (
            "historical_info",
            scalar_or_block(Setting, Struct(&TEXT_SELECTOR))
                .doc("Historical-background localization key or conditional selector."),
        ),
        (
            "trigger",
            block(Trigger).doc("Conditions required for the event to fire."),
        ),
        (
            "major",
            toggle("Notify other countries when the event fires."),
        ),
        (
            "major_trigger",
            block_scoped(Trigger, "country").doc(
                "Countries which can see the notification (root = country; `from` = original event scope).",
            ),
        ),
        (
            "hidden",
            toggle("Run without displaying title, description, or options."),
        ),
        (
            "immediate",
            block(Effect).doc("Effects run as soon as the event fires."),
        ),
        (
            "after",
            block(Effect).doc("Effects run after an option is selected."),
        ),
        (
            "on_trigger_fail",
            block(Effect).doc("Effects run when a directly queued event fails its trigger."),
        ),
        (
            "fire_only_once",
            toggle("Allow the event to fire only once per campaign."),
        ),
        (
            "interface_lock",
            toggle("Pause single-player while the event is displayed."),
        ),
        (
            "dynamic_historical_event",
            block(Struct(&DYNAMIC_HISTORICAL_EVENT)),
        ),
        (
            "orphan",
            toggle("Suppress warnings when no script references this event."),
        ),
        (
            "hide_portraits",
            toggle("Hide saved-target character portraits."),
        ),
        (
            "outcome",
            scalar(Setting)
                .values(&["positive", "neutral", "negative"])
                .doc("Audio direction for the event outcome."),
        ),
        (
            "category",
            scalar(Setting)
                .values(&[
                    "disaster_event",
                    "situation_event",
                    "international_organization_event",
                    "generic_event",
                ])
                .doc("Icon/category used to present the event."),
        ),
        (
            "illustration_tags",
            block(Config).doc("Tags used to select an event illustration."),
        ),
        (
            "weight_multiplier",
            block(ScriptValue).doc("Weight when selected from random event/on-action lists."),
        ),
        ("image", scalar(Setting).doc("Event illustration path.")),
        (
            "option",
            block(Struct(&OPTION)).doc("A selectable event option; may be repeated."),
        ),
    ],
    // Keep corpus-only engine additions accepted while the documented surface
    // remains explicit and useful for completion/hover.
    fallback: Fallback::Ignore,
};

pub(crate) struct Event;

impl Entity for Event {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::EVENT,
        icon: IconHint::Event,
        defs: Some(DefSource {
            dir_prefix: EVENTS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            RefRule {
                pattern: RefPattern::KeyValue("trigger_event_silently"),
                gate: None,
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyValue("trigger_event_non_silently"),
                gate: None,
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyBlockField("trigger_event_silently", "id"),
                gate: None,
                alt: &[],
            },
            RefRule {
                pattern: RefPattern::KeyBlockField("trigger_event_non_silently", "id"),
                gate: None,
                alt: &[],
            },
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(EVENTS_DIR, ClauseKind::Struct(&EVENT))];
}
