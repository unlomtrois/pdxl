//! Flat node-pool syntax tree for Paradox script files — the **data model only**.
//!
//! This crate owns the tree representation ported from `internal/parser/v3`:
//! a single contiguous node pool, a separate child-index array, and byte-offset
//! source ranges into an `Arc<[u8]>` source. Parsing lives one layer up in
//! `pdxl-parser`; this split exists so that consumers of *trees* (the syntax
//! cache, semantic analysis) can depend on the stable data layout without
//! pulling in the parsing algorithm.
//!
//! Layout invariants (checked by [`validate_tree`]):
//! 1. `nodes[0]` is always the [`NodeKind::File`] root.
//! 2. A node's children are `child_ids[child_start..child_end]`.
//! 3. Each `child_ids` entry indexes `nodes`.
//! 4. Node source ranges stay within `source`.
//! 5. Scalar / tagged-block text is recovered from the source range — never
//!    copied.

mod node;
mod validate;

pub use node::{Node, NodeId, NodeKind, SyntaxTree};
pub use validate::{ValidationError, validate_tree};

/// Version of the syntax data model **and** of everything upstream that shapes
/// it (lexer token rules, parser behavior, node layout).
///
/// Persistent stores (the syntax cache) embed this in every entry and treat a
/// mismatch as a miss. **Bump it whenever a change would make a previously
/// stored tree wrong** — new token kinds, changed parse rules, reordered node
/// fields. This is the fix for the Go implementation's "content-keyed caveat",
/// where entries keyed only by source content silently served stale trees
/// after a parser change.
pub const SYNTAX_VERSION: u32 = 1;

// Re-exported so tree consumers can name operator kinds and ranges without a
// direct pdxl-lexer / pdxl-src dependency.
pub use pdxl_lexer::TokenKind;
pub use pdxl_src::TextRange;
