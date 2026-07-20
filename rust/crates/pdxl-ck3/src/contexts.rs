//! Structural-context schema assembly.
//!
//! The specs themselves live with their owning concept in `entities/` (see
//! `rust/docs/STRUCTURAL-CONTEXTS.md` for the model). This module only
//! assembles their roots into the [`ContextSchema`] the analysis engine
//! consumes, built once and shared.

use std::sync::OnceLock;

use pdxl_analysis::context::{ContextSchema, StructSpec};

/// Built-in effects whose block is a documented structure, so completion and
/// hover work inside them (`create_character = { … }`).
const EFFECT_STRUCTS: &[(&str, &StructSpec)] = &[(
    "create_character",
    &crate::entities::create_character::CREATE_CHARACTER,
)];

/// The CK3 structural-context schema. Assembled once from every entity's
/// declared roots (see [`crate::entities`]); cheap to call thereafter.
pub fn context_schema() -> &'static ContextSchema {
    static CTX: OnceLock<ContextSchema> = OnceLock::new();
    CTX.get_or_init(|| ContextSchema {
        // Leak once: the assembled roots live for the whole process, and the
        // engine wants a `&'static` slice.
        roots: Box::leak(crate::entities::roots().into_boxed_slice()),
        effect_structs: EFFECT_STRUCTS,
    })
}
