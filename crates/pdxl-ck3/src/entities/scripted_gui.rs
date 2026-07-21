//! Scripted GUIs (`common/scripted_guis/`) — script logic invoked from the
//! interface: a named `is_shown`/`is_valid` check plus an `effect`, run via
//! the `GetScriptedGui('name')` datafunction family
//! (`.IsShown`/`.IsValid`/`.Execute`). No `_*.info` exists; the body below is
//! distilled from the vanilla corpus (all 27 vanilla entries surveyed).
//!
//! References come from `.gui` files as datafunction *arguments*
//! (`GetScriptedGui('x')`, 150 corpus refs, 0 unresolved) — extracted by the
//! gui layer's argument-reference map ([`GuiKinds::arg_refs`]), not by a
//! schema `RefRule` (script never names scripted GUIs directly).
//!
//! [`GuiKinds::arg_refs`]: pdxl_analysis::GuiKinds

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Effect, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec};

use super::Entity;
use super::common::OPAQUE;

const SCRIPTED_GUIS_DIR: &str = "common/scripted_guis/";

/// The body of one scripted GUI.
static SCRIPTED_GUI: StructSpec = StructSpec {
    name: "scripted_gui",
    fields: &[
        (
            "scope",
            scalar(Setting).doc(
                "The scope type this scripted GUI is called on — must match the \
                 datacontext at the `GetScriptedGui` call site (vanilla uses `character` \
                 and `innovation_type`).",
            ),
        ),
        (
            "saved_scopes",
            block(Struct(&OPAQUE)).doc(
                "Scope names the call site provides via `AddScope` — accessible as \
                 `scope:<name>` in the triggers and effect.",
            ),
        ),
        (
            "is_shown",
            block(Trigger).doc("Whether the gui element is shown (`.IsShown` datafunction)."),
        ),
        (
            "is_valid",
            block(Trigger).doc("Whether the action is currently valid (`.IsValid` datafunction)."),
        ),
        (
            "effect",
            block(Effect).doc("Run when the gui invokes `.Execute` (e.g. a button click)."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct ScriptedGui;

impl Entity for ScriptedGui {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::SCRIPTED_GUI,
        icon: IconHint::Function,
        defs: Some(DefSource {
            dir_prefix: SCRIPTED_GUIS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(SCRIPTED_GUIS_DIR, ClauseKind::Struct(&SCRIPTED_GUI))];
}
