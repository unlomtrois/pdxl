//! Flat node-pool syntax tree and parser for Paradox script files.
//!
//! A direct port of `internal/parser/v3`, preserving the Go parser's observable
//! behavior *and* its internal layout: a single contiguous node pool with a
//! separate child-index array, post-order node allocation, byte-offset source
//! ranges, and error-tolerant recovery that always yields a partial tree.
//!
//! The Go implementation is the oracle for this milestone; node allocation order
//! and child-index ordering are differential-parity targets, not just "logically
//! equivalent trees".
//!
//! ```
//! let parse = pdxl_syntax::parse("example.txt", &b"key = value"[..]);
//! assert!(parse.diagnostics().is_empty());
//! let tree = parse.tree();
//! let field = tree.children(tree.root()).next().unwrap();
//! assert_eq!(tree.node(field).kind, pdxl_syntax::NodeKind::Field);
//! ```

mod diagnostic;
mod dump;
mod node;
mod parser;
mod render;
mod validate;

pub use diagnostic::{Diagnostic, Parse, Severity};
pub use dump::{DUMP_VERSION, dump_json};
pub use node::{Node, NodeId, NodeKind, SyntaxTree};
pub use parser::parse;
pub use render::render_tree;
pub use validate::{ValidationError, validate_tree};

// Re-exported so downstream crates and tools need not depend on pdxl-lexer/source
// directly just to name a node's operator or range.
pub use pdxl_lexer::TokenKind;
pub use pdxl_source::TextRange;

#[cfg(test)]
mod tests;
