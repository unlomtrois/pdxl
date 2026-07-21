//! Game concepts (`common/game_concepts/`, no `.info` file — modeled from the
//! vanilla corpus). A concept is a documentation entry the engine links from
//! localization text (`[Concept('vassal', …)]`, `#L_… #!` markup) and the
//! encyclopedia. Definitions are top-level `NAME = { … }` blocks; 1045 in
//! vanilla, each optionally naming synonyms via `alias = { … }` (1314 more
//! resolvable names).
//!
//! References (corpus-validated at 0 unresolved): `parent = X` names another
//! concept — 683 uses, 12 of which resolve only through an alias
//! (`parent = accolades` → the `accolade` concept), which is exactly why the
//! alias list must be harvested as extra def names. The rule is gated to the
//! game_concepts directory and matched at depth 1 ([`RefPattern::KeyValueTop`]),
//! since `parent` is a common key elsewhere (gui, culture eras).
//!
//! Not modeled as refs: loc-layer `Concept('X')` / `#L_X` markup (61 in
//! vanilla) — that belongs to the localization datafunction scanner, still
//! deferred (see BACKLOG), the same machinery `Custom('X')` needs.
//!
//! The body is a closed, fully-enumerated struct (only these seven keys occur
//! corpus-wide), so the fallback denies unknown keys.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Config};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

const GAME_CONCEPTS_DIR: &str = "common/game_concepts/";

/// The body of one game-concept definition.
static GAME_CONCEPT: StructSpec = StructSpec {
    name: "game_concept",
    fields: &[
        (
            "alias",
            block(Config).doc(
                "Alternate names the concept can be linked by (each resolves to \
                 this concept). E.g. `alias = { vassals vassalize vassalage }`.",
            ),
        ),
        (
            "parent",
            scalar(Setting).doc(
                "A broader concept this one specializes; falls back to the \
                 parent's texture/encyclopedia entry when unset.",
            ),
        ),
        (
            "texture",
            scalar(Setting)
                .doc("`.dds` icon shown beside the concept link (e.g. `gfx/interface/icons/icon_vassal.dds`)."),
        ),
        (
            "framesize",
            block(Config)
                .doc("Frame dimensions `{ W H }` when the texture is a sprite sheet."),
        ),
        (
            "frame",
            scalar(Setting).doc("1-based frame index into the sprite sheet (requires `framesize`)."),
        ),
        (
            "requires_dlc_flag",
            scalar(Setting)
                .doc("Concept is only defined when this DLC flag is active (e.g. `royal_court`, `all_under_heaven`)."),
        ),
        (
            "shown_in_encyclopedia",
            scalar(Setting)
                .doc("Set `no` to hide the concept from the in-game encyclopedia (default `yes`).")
                .values(&["yes", "no"]),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct GameConcept;

impl Entity for GameConcept {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::GAME_CONCEPT,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: GAME_CONCEPTS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[RefRule {
            pattern: RefPattern::KeyValueTop("parent"),
            gate: Some(GAME_CONCEPTS_DIR),
            alt: &[],
        }],
        // Each name in `alias = { … }` resolves to the concept.
        aliases: &["alias"],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(GAME_CONCEPTS_DIR, ClauseKind::Struct(&GAME_CONCEPT))];
}
