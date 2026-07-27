//! Player-facing messages (`common/messages/`, from `_messages.info`) — the
//! feed entries and toasts raised by `send_interface_message` — plus the two
//! kinds that organize them in the message-settings window:
//! `common/message_filter_types/` and `common/message_group_types/`.
//!
//! The three form a chain, and modeling only the first would leave most of it
//! dangling: a message names a filter type (665 uses), and a filter type names
//! a group (174). Corpus: every one resolves.
//!
//! Cross-references:
//! - `send_interface_message = { type = X }` (and the `_as_toast` variant)
//!   names a message. `type` is far too overloaded to catch ungated, so the
//!   rule keys on the enclosing effect via [`RefPattern::KeyValueUnder`].
//! - `message_filter_type = X` is ungated: besides a message body it appears in
//!   `send_interface_message`, which the info says overrides the message's own.
//! - `group = X` names a group type, gated to `common/message_filter_types/` —
//!   `group` means something different in eight other directories.
//!
//! Where the info and the corpus disagree, the corpus wins: the Structure
//! section documents `text` for the message title, but no file uses it and 536
//! use `title` — which is also what the info's own EXAMPLES section writes. The
//! same section omits `display = popup`, used twice.
//!
//! Implicit localization, all three documented by the info and corpus-verified:
//! a filter type is `message_filter_<key>` (180/180), a group type is
//! `message_group_type_<key>` (29/31), and a message falls back to its own key
//! when it declares no `title` — 146 of 146 such messages have exactly that.

use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, scalar};
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern, RefRule,
};

use crate::kinds;

use super::Entity;
use super::common::anywhere;

const MESSAGES_DIR: &str = "common/messages/";
const FILTER_DIR: &str = "common/message_filter_types/";
const GROUP_DIR: &str = "common/message_group_types/";

/// The body of one message definition.
static MESSAGE: StructSpec = StructSpec {
    name: "message",
    fields: &[
        (
            "title",
            scalar(LocKey).doc(
                "The message's headline. Defaults to the message's own key \
                 *(the info calls this field `text`; no file does)*.",
            ),
        ),
        (
            "desc",
            scalar(LocKey).doc("Longer text explaining what happened."),
        ),
        (
            "tooltip",
            scalar(LocKey).doc("Hover text for the message item. Default: none."),
        ),
        (
            "style",
            scalar(Setting)
                .doc("How the item reads. Default `neutral`.")
                .values(&["good", "bad", "neutral"]),
        ),
        (
            "display",
            scalar(Setting)
                .doc("Where the message appears. Default `feed`.")
                .values(&["feed", "toast", "popup"]),
        ),
        (
            "message_filter_type",
            scalar(Setting).doc(
                "The filter group this message sits under in message settings. \
                 Overridden by the same key on `send_interface_message`. \
                 Default: empty, which hides it from settings entirely.",
            ),
        ),
        (
            "icon",
            scalar(Setting).doc("Texture under `gfx/interface/message_icons`."),
        ),
        (
            "soundeffect",
            scalar(Setting).doc("Sound played when shown. Default: chosen from display + style."),
        ),
        (
            "flag",
            scalar(Setting).doc(
                "Repeatable customization key. A flagged message needs matching \
                 handling in `hud_notification_templates.gui` — the default toast \
                 only renders for messages carrying no flags.",
            ),
        ),
        (
            "combine_into_one",
            scalar(Setting)
                .doc(
                    "Merge into an existing message of this type rather than \
                     animating a new one — for high-frequency messages.",
                )
                .values(&["yes", "no"]),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one message filter type.
static FILTER_TYPE: StructSpec = StructSpec {
    name: "message_filter_type",
    fields: &[
        (
            "display",
            scalar(Setting)
                .doc("Where messages of this filter appear. Default `feed`.")
                .values(&["feed", "toast", "hidden"]),
        ),
        (
            "group",
            scalar(Setting).doc("The foldable group in message settings. Default `misc`."),
        ),
        (
            "always_show",
            scalar(Setting)
                .doc("Stop the player hiding messages of this filter. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "auto_pause",
            scalar(Setting)
                .doc("Pause the game when one of these appears. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "sort_order",
            scalar(Setting).doc(
                "Position in message settings, higher first; ties break on \
                 definition order. Default `0`.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one message group type.
static GROUP_TYPE: StructSpec = StructSpec {
    name: "message_group_type",
    fields: &[(
        "sort_order",
        scalar(Setting).doc(
            "Position of the group in message settings, higher first; ties \
             break on definition order. Default `0`.",
        ),
    )],
    fallback: Fallback::Deny,
};

/// A `type = X` message reference, keyed on the effect that encloses it.
const fn message_type(effect: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValueUnder(effect, "type"),
        gate: None,
        alt: &[],
    }
}

pub(crate) struct Message;

impl Entity for Message {
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::MESSAGE,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::MESSAGE_FILTER_TYPE,
            suffix: "message_filter_{}",
        },
        ImplicitLocPattern {
            kind: kinds::MESSAGE_GROUP_TYPE,
            suffix: "message_group_type_{}",
        },
    ];

    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::MESSAGE,
            icon: IconHint::Text,
            defs: Some(DefSource {
                dir_prefix: MESSAGES_DIR,
                shape: DefShape::TopLevel,
            }),
            // The three engine effects that raise a message, by corpus
            // frequency. `send_interface_toast` is by far the common one
            // (11156 uses to `send_interface_message`'s 2001) despite the info
            // naming only the latter two. The sibling spellings are *scripted*
            // effects wrapping these — `send_interface_message_good`/`_bad`/
            // `_as_popup` each have a top-level definition, so their bodies
            // carry the real reference and they need no rule of their own.
            refs: &[
                message_type("send_interface_toast"),
                message_type("send_interface_message"),
                message_type("send_interface_message_as_toast"),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::MESSAGE_FILTER_TYPE,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: FILTER_DIR,
                shape: DefShape::TopLevel,
            }),
            // Ungated: a message body names one, and so does
            // `send_interface_message`, which overrides it.
            refs: &[anywhere(RefPattern::KeyValue("message_filter_type"))],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::MESSAGE_GROUP_TYPE,
            icon: IconHint::Hierarchy,
            defs: Some(DefSource {
                dir_prefix: GROUP_DIR,
                shape: DefShape::TopLevel,
            }),
            // `group` names eight other things elsewhere, so this is gated to
            // the one directory where it means a message group.
            refs: &[RefRule {
                pattern: RefPattern::KeyValue("group"),
                gate: Some(FILTER_DIR),
                alt: &[],
            }],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (MESSAGES_DIR, Struct(&MESSAGE)),
        (FILTER_DIR, Struct(&FILTER_TYPE)),
        (GROUP_DIR, Struct(&GROUP_TYPE)),
    ];
}
