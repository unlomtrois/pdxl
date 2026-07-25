//! EU5's schema-coverage survey roots. The survey engine is game-agnostic
//! (`pdxl_project::coverage`); the `schema-gaps` bin in `pdxl-cli` composes
//! it with these roots through the `pdxl-game` facade.

/// The definition roots worth surveying — EU5 splits content under module
/// roots, so the prefixes carry them (gui/localization are engine-level;
/// `main_menu/common` holds the pre-game databases, `in_game/setup` the
/// scenario data).
pub const SURVEY_ROOTS: &[&str] = &[
    "in_game/common",
    "in_game/events",
    "in_game/setup",
    "main_menu/common",
];
