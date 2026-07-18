//! Event backgrounds (`common/event_backgrounds/`) — schema row plus the
//! `_event_backgrounds.info` structural context.
//!
//! A file holds `<id> = { background = { … } … }` definitions; the `<id>` is
//! the background key. Script selects one with a `reference` inside a
//! `background` / `override_background` block (an event theme's background, or
//! an event's `override_background`).

use pdxl_analysis::context::ClauseKind::{self, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule, SymbolKind};

use super::Entity;
use super::common::anywhere;

/// One `background = { … }` candidate: the first whose `trigger` passes is
/// shown (`_event_backgrounds.info`).
static BACKGROUND: StructSpec = StructSpec {
    name: "background",
    fields: &[
        (
            "trigger",
            block(Trigger).doc("Receives the event scope; checked to see if this candidate fits."),
        ),
        (
            "reference",
            scalar(Setting).doc("Path to the texture, or the key of another event background."),
        ),
        ("video", scalar(Setting).doc("Is the reference a video?")),
        (
            "environment",
            scalar(Setting)
                .doc("Reference key to a database object in gfx/portraits/environments/."),
        ),
        (
            "ambience",
            scalar(Setting).doc("Ambience sound-effect reference (as defined in GUIDs.txt)."),
        ),
        (
            "video_mask",
            scalar(Setting).doc("Video mask used to alpha-multiply the fade video or image."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one `<id> = { … }` definition: one or more `background` blocks.
static EVENT_BACKGROUND: StructSpec = StructSpec {
    name: "event_background",
    fields: &[(
        "background",
        block(ClauseKind::Struct(&BACKGROUND)).doc(
            "A background shown when the event pops up. With multiple, the first whose trigger \
             fits is selected.",
        ),
    )],
    fallback: Fallback::Deny,
};

pub(crate) struct EventBackground;

impl Entity for EventBackground {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::EventBackground,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/event_backgrounds/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            // A theme's `background = { reference = X }` names a background.
            // Gated to event_themes: inside the def files themselves the same
            // `background = { reference = "gfx/…dds" }` shape holds a *texture
            // path*, not a cross-reference.
            RefRule {
                pattern: RefPattern::KeyBlockField("background", "reference"),
                gate: Some("common/event_themes/"),
            },
            // An event's `override_background = { reference = X }` (events only;
            // always a bare background key).
            anywhere(RefPattern::KeyBlockField(
                "override_background",
                "reference",
            )),
            // The scalar shorthand `override_background = X`. Path values
            // (`"gfx/…dds"`) are skipped by skip_ref_value's `/` rule.
            anywhere(RefPattern::KeyValue("override_background")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(
        "common/event_backgrounds/",
        ClauseKind::Struct(&EVENT_BACKGROUND),
    )];
}
