//! CK3's schema-coverage survey roots. The survey engine is game-agnostic
//! (`pdxl_project::coverage`); the `schema-gaps` bin in `pdxl-cli` composes
//! it with these roots through the `pdxl-game` facade.

/// The definition roots worth surveying (gui is modeled by `pdxl-gui`,
/// localization by `pdxl-yml`).
pub const SURVEY_ROOTS: &[&str] = &["common", "events", "history"];
