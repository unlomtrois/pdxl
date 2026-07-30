//! Death reasons (`common/deathreasons/`, from `_death_reasons.info`) —
//! top-level `death_* = { … }` definitions. Referenced by `death_reason = X`
//! (the `death` effect and history death blocks) — unambiguous corpus-wide;
//! the only unresolved value is T4N's `death_unknown` (a genuine mod bug:
//! only the *icon* `death_unknown.dds` exists).
//!
//! The `use_equipped_artifact_in_slot` → artifact-slot reference lives with
//! its target kind in `artifact.rs`.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block_scoped, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{anywhere, toggle};

const DEATHREASONS_DIR: &str = "common/deathreasons/";

/// The body of one death-reason definition.
static DEATH_REASON: StructSpec = StructSpec {
    name: "death_reason",
    fields: &[
        (
            "public_knowledge",
            toggle("If `yes`, everybody knows the killer (default `no`)."),
        ),
        (
            "icon",
            scalar(Setting).doc(
                "The icon `.dds` (directory from the `DEATH_REASON_ICON_PATH` define; \
                 `DEFAULT_DEATH_REASON_ICON` if unspecified).",
            ),
        ),
        (
            "natural_death_trigger",
            block_scoped(Trigger, "character").doc(
                "Whether this is a valid natural death reason for the dying character \
                 (their scope).",
            ),
        ),
        (
            "priority",
            scalar(Setting).doc(
                "Highest-priority passing reason wins when picking a natural death \
                 (default 0).",
            ),
        ),
        (
            "default",
            toggle(
                "When no natural death reason passes, one of the `default` reasons is \
                 picked randomly (default `no`).",
            ),
        ),
        (
            "use_equipped_artifact_in_slot",
            scalar(Setting).doc(
                "If this slot is filled when the reason is used, that artifact is said to \
                 have been the murder weapon.",
            ),
        ),
        (
            "epidemic",
            scalar(Setting).doc(
                "The epidemic type this reason is tied to — deaths count toward an ongoing \
                 epidemic when the character carries its disease trait.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct DeathReason;

impl Entity for DeathReason {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::DEATH_REASON,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: DEATHREASONS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[anywhere(RefPattern::KeyValue("death_reason"))],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(DEATHREASONS_DIR, ClauseKind::Struct(&DEATH_REASON))];
}
