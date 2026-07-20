//! Event themes (`common/event_themes/`) — schema row (the event `theme = X`
//! reference) plus the `_event_themes.info` structural context.
//!
//! A theme bundles the visual/audio channels an event uses; each channel is a
//! `{ trigger reference }` group (the shared [`TRIGGERED_ASSET`] shape). The
//! `background` channel's `reference` names an event background — that
//! reference is owned by the event-background entity (gated to this directory).

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::context::{Fallback, StructSpec, block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{TRIGGERED_ASSET, anywhere};

/// A theme body: one triggered-asset group per channel. `background`, `icon`
/// and `sound` are required; `transition` and `effect_2d` are optional.
static THEME: StructSpec = StructSpec {
    name: "event_theme",
    fields: &[
        (
            "background",
            block(ClauseKind::Struct(&TRIGGERED_ASSET)).doc(
                "Background shown when the event pops up; reference is an event-background key.",
            ),
        ),
        (
            "icon",
            block(ClauseKind::Struct(&TRIGGERED_ASSET))
                .doc("Icon shown when the event pops up; reference is a texture path."),
        ),
        (
            "sound",
            block(ClauseKind::Struct(&TRIGGERED_ASSET))
                .doc("Sound played when the event pops up; reference is a GUIDs.txt sound key."),
        ),
        (
            "transition",
            block(ClauseKind::Struct(&TRIGGERED_ASSET))
                .doc("Optional transition overlay; reference is an event-transition key."),
        ),
        (
            "effect_2d",
            block(ClauseKind::Struct(&TRIGGERED_ASSET))
                .doc("Optional 2D effect over the background; reference is an event-effect key."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct EventTheme;

impl Entity for EventTheme {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::EVENT_THEME,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/event_themes/",
            shape: DefShape::TopLevel,
        }),
        // The event `theme = martial` keyword names a theme. Values are always
        // bare keys (no scope/quoted/path forms in the corpus).
        refs: &[anywhere(RefPattern::KeyValue("theme"))],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[("common/event_themes/", ClauseKind::Struct(&THEME))];
}
