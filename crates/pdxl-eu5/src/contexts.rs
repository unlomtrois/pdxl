//! EU5 structural contexts — empty until entity bodies are modeled from the
//! game's `.info` docs (the roots table drives completion/hover/colors).

use pdxl_analysis::context::ContextSchema;

static CONTEXT_SCHEMA: ContextSchema = ContextSchema {
    roots: &[],
    effect_structs: &[],
};

/// The (currently empty) EU5 context schema.
pub fn context_schema() -> &'static ContextSchema {
    &CONTEXT_SCHEMA
}
