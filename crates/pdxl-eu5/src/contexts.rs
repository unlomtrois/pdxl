//! Structural-context schema assembly — the specs live with their owning
//! concept in `entities/`; this module assembles their roots into the
//! [`ContextSchema`] the analysis engine consumes, built once and shared.

use std::sync::OnceLock;

use pdxl_analysis::context::ContextSchema;

/// The EU5 structural-context schema.
pub fn context_schema() -> &'static ContextSchema {
    static CTX: OnceLock<ContextSchema> = OnceLock::new();
    CTX.get_or_init(|| ContextSchema {
        // Leak once: the assembled roots live for the whole process.
        roots: Box::leak(crate::entities::roots().into_boxed_slice()),
        effect_structs: &[],
    })
}
