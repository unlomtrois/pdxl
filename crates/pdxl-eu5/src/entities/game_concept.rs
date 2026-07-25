//! Game concepts (`main_menu/common/game_concepts/`) — encyclopedia concepts
//! linked from localization as `[concept|e]` and `[Concept('concept')]`.
//! Definitions are top-level blocks; `alias` values are alternate resolvable
//! names. `family` links a concept to another concept (including aliases).
//! The body is corpus-complete for EU5's vanilla concept file.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Config};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

const GAME_CONCEPTS_DIR: &str = "main_menu/common/game_concepts/";

static GAME_CONCEPT: StructSpec = StructSpec {
    name: "game concept",
    fields: &[
        (
            "alias",
            block(Config).doc("Alternate names which resolve to this concept."),
        ),
        (
            "family",
            scalar(Setting).doc("Broader concept family, which may use a concept alias."),
        ),
        (
            "texture",
            scalar(Setting).doc("Icon texture shown for the concept."),
        ),
        (
            "shown_in_loading_screen",
            scalar(Setting)
                .doc("Whether the concept may be shown on loading screens.")
                .values(&["yes", "no"]),
        ),
        (
            "tooltip_map_mode",
            scalar(Setting).doc("Map mode selected by the concept tooltip."),
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
            pattern: RefPattern::KeyValueTop("family"),
            gate: Some(GAME_CONCEPTS_DIR),
            alt: &[],
        }],
        aliases: &["alias"],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(GAME_CONCEPTS_DIR, ClauseKind::Struct(&GAME_CONCEPT))];
}
