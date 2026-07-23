//! Events (`events/`): schema row (event/loc references) plus the rich
//! `_events.info` structural context (option, portraits, widgets, overrides),
//! with documented fields and value enums for `type` / `window` / `skill` /
//! portrait `position` / widget `controller`.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, Effect, ScriptValue, ScriptedModifier, Trigger,
};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting, Target};
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{DURATION, OPAQUE, TRIGGERED_ASSET, anywhere, in_on_action};

/// A `yes`/`no` toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// The six character skills, for an option's `skill` marker.
const SKILLS: &[&str] = &[
    "diplomacy",
    "martial",
    "stewardship",
    "intrigue",
    "learning",
    "prowess",
];

static TRIGGERED_ANIMATION: StructSpec = StructSpec {
    name: "triggered_animation",
    fields: &[
        (
            "trigger",
            block(Trigger).doc("First triggered_animation whose trigger passes is used."),
        ),
        (
            "animation",
            scalar(Setting)
                .doc("Animation name (see animations.txt / the in-game portrait editor)."),
        ),
        (
            "scripted_animation",
            scalar(Setting).doc("A scripted animation key (alternative to `animation`)."),
        ),
        (
            "camera",
            scalar(Setting)
                .doc("Camera name; overrides the portrait camera when this animation is chosen."),
        ),
    ],
    fallback: Fallback::Deny,
};

static TRIGGERED_OUTFIT: StructSpec = StructSpec {
    name: "triggered_outfit",
    fields: &[
        (
            "trigger",
            block(Trigger).doc("First triggered_outfit whose trigger passes is used."),
        ),
        (
            "outfit_tags",
            scalar_or_block(Setting, ClauseKind::Struct(&OPAQUE))
                .doc("Outfit tags in ascending priority (later tags override earlier)."),
        ),
        (
            "remove_default_outfit",
            toggle("Disable portrait-modifier categories that match no event tag."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The block form of `left_portrait = { … }` et al. Its `trigger` runs in
/// the *portrait character's* scope (layer-3 note in the design doc).
static PORTRAIT: StructSpec = StructSpec {
    name: "portrait",
    fields: &[
        (
            "character",
            scalar(Target).doc("The event target whose portrait is shown."),
        ),
        (
            "trigger",
            block(Trigger).doc(
                "Controls this portrait's visibility (runs in the portrait character's scope).",
            ),
        ),
        (
            "animation",
            scalar(Setting).doc("Default animation, used if no triggered_animation passes."),
        ),
        (
            "scripted_animation",
            scalar(Setting).doc("A scripted-animation key (alternative to `animation`)."),
        ),
        (
            "triggered_animation",
            block(ClauseKind::Struct(&TRIGGERED_ANIMATION))
                .doc("Trigger-gated animation; the first whose trigger passes wins."),
        ),
        (
            "triggered_outfit",
            block(ClauseKind::Struct(&TRIGGERED_OUTFIT))
                .doc("Trigger-gated outfit; the first whose trigger passes wins."),
        ),
        (
            "camera",
            scalar(Setting).doc("Camera key for this portrait."),
        ),
        (
            "override_imprisonment_visuals",
            toggle("Ignore the character's imprisonment visuals."),
        ),
        (
            "animate_if_dead",
            toggle("Animate the portrait even if the character is dead."),
        ),
        (
            "outfit_tags",
            scalar_or_block(Setting, ClauseKind::Struct(&OPAQUE))
                .doc("Outfit tags in ascending priority for this portrait."),
        ),
        (
            "remove_default_outfit",
            toggle("Disable portrait-modifier categories that match no event tag (default `no`)."),
        ),
        (
            "hide_info",
            toggle("Show only the portrait, with no CoA, tooltips, or clicks (default `no`)."),
        ),
    ],
    fallback: Fallback::Deny,
};

static ARTIFACT: StructSpec = StructSpec {
    name: "artifact",
    fields: &[
        (
            "target",
            scalar(Target).doc("The artifact event target to display."),
        ),
        (
            "position",
            scalar(Setting)
                .doc("Where to show the artifact (cannot share a portrait's position).")
                .values(&[
                    "lower_left_portrait",
                    "lower_center_portrait",
                    "lower_right_portrait",
                ]),
        ),
        (
            "trigger",
            block(Trigger).doc("Optional visibility trigger, as for portraits."),
        ),
    ],
    fallback: Fallback::Deny,
};

static COURT_SCENE: StructSpec = StructSpec {
    name: "court_scene",
    fields: &[
        (
            "button_position_character",
            scalar(Target).doc("Character positioned at the buttons."),
        ),
        ("court_owner", scalar(Target).doc("The court's owner.")),
        (
            "court_event_force_open",
            toggle("Force the court view open for this event."),
        ),
        (
            "show_timeout_info",
            toggle("Show the event timeout information."),
        ),
        (
            "should_pause_time",
            toggle("Pause game time while the court scene is shown."),
        ),
        // Role keys are dynamic (`scope:x = { role = … }`).
        (
            "roles",
            block(ClauseKind::Struct(&OPAQUE))
                .doc("`scope:char = { role = … / group = … animation = … }` role assignments."),
        ),
    ],
    fallback: Fallback::Deny,
};

static WIDGET: StructSpec = StructSpec {
    name: "widget",
    fields: &[
        ("is_shown", block(Trigger).doc("Visibility trigger (event scope after the immediate effect; default always).")),
        ("gui", scalar(Setting).doc("Widget file at `<event_window_widgets>/<name>.gui`.")),
        ("container", scalar(Setting).doc("Parent container widget name in the event window.")),
        (
            "controller",
            scalar_or_block(Setting, ClauseKind::Struct(&OPAQUE))
                .doc("Widget controller. Simple form `controller = default`, or `{ type = text data = { … } }`.")
                .values(&[
                    "default",
                    "name_character",
                    "text",
                    "event_chain_progress",
                    "struggle_info",
                    "situation_info",
                ]),
        ),
        ("setup_scope", block(Effect).doc("Effect to set up scopes the controller expects (e.g. text_target).")),
    ],
    fallback: Fallback::Deny,
};

static WIDGETS: StructSpec = StructSpec {
    name: "widgets",
    fields: &[(
        "widget",
        block(ClauseKind::Struct(&WIDGET)).doc("One embedded custom widget."),
    )],
    fallback: Fallback::Deny,
};

/// `name = { text = … trigger = { … } }`: the gated option-name candidate.
static OPTION_NAME: StructSpec = StructSpec {
    name: "option_name",
    fields: &[
        (
            "text",
            scalar_or_block(LocKey, DynamicDesc)
                .doc("The name text: a loc key or dynamic-description block."),
        ),
        (
            "trigger",
            block(Trigger).doc("Gates whether this name candidate is available."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The event option: known structural fields, plus the key finding from
/// `_events.info`, that **every unknown key is an inline effect**.
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
                .doc("Arbitrary flag for why the option is unlocked, checked in the UI for special display."),
        ),
        (
            "skill",
            scalar(Setting)
                .doc("Marks the option skill-relevant in the unlock-reason UI (still gate the skill in `trigger`).")
                .values(SKILLS),
        ),
        (
            "trait",
            scalar(Setting).doc("Marks the option trait-relevant in the unlock-reason UI (still gate the trait in `trigger`)."),
        ),
        (
            "show_unlock_reason",
            toggle("Whether the unlock-reason UI is shown for this option."),
        ),
        (
            "is_cancel_option",
            toggle("Marks a cancel / back-out style option (used by some widgets and controllers)."),
        ),
        (
            "clicksound",
            scalar(Setting).doc("Sound effect played when the option is clicked."),
        ),
        (
            "fallback",
            toggle("If yes, this option is considered only when no regular option is valid."),
        ),
        (
            "exclusive",
            toggle("If any exclusive option is valid, non-exclusive options are ignored."),
        ),
        (
            "ai_chance",
            block(ScriptedModifier)
                .doc("Weighted-modifier block for the AI's chance to pick this option (see scripted_modifiers)."),
        ),
        (
            "ai_will_select",
            block(ScriptValue).doc("Script value for the AI's chance to pick this option (wins over ai_chance)."),
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
        (
            "type",
            scalar(Setting)
                .doc("Event type; drives the default window and available scopes (default `character_event`).")
                .values(&["character_event", "letter_event", "court_event", "activity_event"]),
        ),
        (
            "scope",
            scalar(Setting)
                .doc("Overrides the event's root scope (default `character`). `none` for no root; e.g. `artifact`, `title`.")
                .values(&[
                    "none", "character", "title", "artifact", "culture", "faith", "province",
                    "army", "activity", "struggle", "situation", "scheme", "war", "secret", "story",
                ]),
        ),
        (
            "window",
            scalar(Setting)
                .doc("Custom event-window name (gui/event_windows). Defaults from `type`.")
                .values(&[
                    "character_event",
                    "letter_event",
                    "anonymous_letter_event",
                    "big_event_window",
                    "duel_event",
                    "fullscreen_event",
                    "scheme_conclusion_window",
                    "scheme_failed_event",
                    "scheme_preparations_event",
                    "scheme_successful_event",
                    "scheme_conclusion_event_no_header",
                    "visit_settlement_window",
                ]),
        ),
        ("hidden", toggle("If yes, no event window is shown (the event only runs its effects).")),
        ("major", toggle("Marks a major event (highlighted in the UI, sent to relevant characters).")),
        ("orphan", toggle("Suppresses the \"unreferenced event\" log warning (useful for debug events).")),
        ("content_source", scalar(Setting).doc("The DLC or mod this event belongs to, shown in the event window.")),
        ("theme", scalar(Setting).doc("The event theme (see 00_event_themes.txt) driving background/icon/sound.")),
        ("title", scalar_or_block(LocKey, DynamicDesc).doc("Event title: a loc key or a dynamic-description block.")),
        ("desc", scalar_or_block(LocKey, DynamicDesc).doc("Event body text: a loc key or a dynamic-description block.")),
        ("opening", scalar_or_block(LocKey, DynamicDesc).doc("Letter-event opening line: a loc key or dynamic-description block.")),
        ("trigger", block(Trigger).doc("Conditions required for this event to fire at all (root = the event scope).")),
        ("major_trigger", block(Trigger).doc("Extra condition for the event to count as major.")),
        ("immediate", block(Effect).doc("Effect run the moment the event fires, before the window is shown (root = the event scope).")),
        ("after", block(Effect).doc("Effect run after the player/AI has picked an option and the window closes.")),
        ("on_trigger_fail", block(Effect).doc("Effect run if a queued/instant event fails its trigger checks (rarely hit for on_action-selected events).")),
        ("cooldown", block(ClauseKind::Struct(&DURATION)).doc("Per-recipient cooldown before this event can fire again; also gates legality.")),
        (
            "left_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT))
                .doc("Left portrait: an event target, or a block for animations, outfits, and triggers."),
        ),
        (
            "right_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)).doc("Right portrait (target or block)."),
        ),
        (
            "center_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)).doc("Center portrait (not used by all event types)."),
        ),
        (
            "lower_left_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)).doc("Lower-left portrait (target or block)."),
        ),
        (
            "lower_center_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)).doc("Lower-center portrait (target or block)."),
        ),
        (
            "lower_right_portrait",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)).doc("Lower-right portrait (target or block)."),
        ),
        (
            "sender",
            scalar_or_block(Target, ClauseKind::Struct(&PORTRAIT)).doc("Letter sender portrait, required for letter events."),
        ),
        ("artifact", block(ClauseKind::Struct(&ARTIFACT)).doc("An artifact shown at a portrait position.")),
        ("court_scene", block(ClauseKind::Struct(&COURT_SCENE)).doc("Court-event scene behavior (roles, owner, pausing).")),
        ("widgets", block(ClauseKind::Struct(&WIDGETS)).doc("Custom widgets embedded in the event window.")),
        ("widget", block(ClauseKind::Struct(&WIDGET)).doc("A single embedded custom widget (shorthand for `widgets = { widget = { … } }`).")),
        ("option", block(ClauseKind::Struct(&OPTION)).doc("An option the player/AI can pick. Unknown keys inside are inline effects.")),
        (
            // Scalar shorthand `override_background = key` or the full
            // `{ trigger reference … }` asset block.
            "override_background",
            scalar_or_block(Setting, ClauseKind::Struct(&TRIGGERED_ASSET))
                .doc("Overrides the theme's background: a key, or `{ trigger reference }`; first matching trigger wins."),
        ),
        (
            "override_transition",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)).doc("Overrides the theme's transition (`{ trigger reference }`)."),
        ),
        (
            "override_effect_2d",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)).doc("Overrides the theme's 2D over-background effect (`{ trigger reference }`)."),
        ),
        ("override_icon", block(ClauseKind::Struct(&TRIGGERED_ASSET)).doc("Overrides the theme's event icon (`{ trigger reference }`).")),
        (
            "override_header_background",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)).doc("Overrides the theme's header asset behind the icon (`{ trigger reference }`)."),
        ),
        (
            "override_sound",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)).doc("Overrides the theme's sound (`{ trigger reference }`)."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Event;

impl Entity for Event {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::EVENT,
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
