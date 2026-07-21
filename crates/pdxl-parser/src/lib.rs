//! Recursive-descent parser for Paradox script files, producing [`pdxl_ast`]
//! trees.
//!
//! A direct port of `internal/parser/v3`, preserving the Go parser's observable
//! behavior *and* its internal layout: post-order node allocation into the flat
//! pool, byte-offset source ranges, and error-tolerant recovery that always
//! yields a partial tree. The tree data model itself lives in [`pdxl_ast`]; this
//! crate owns only the algorithm (plus the golden-format renderer used by
//! tests).
//!
//! The Go implementation was the oracle; node allocation order and child-index
//! ordering were differential-parity targets, not just "logically equivalent
//! trees" — now pinned by the golden snapshots in `tests/golden.rs`.
//!
//! ```
//! let parse = pdxl_parser::parse("example.txt", &b"key = value"[..]);
//! assert!(parse.diagnostics().is_empty());
//! let tree = parse.tree();
//! let field = tree.children(tree.root()).next().unwrap();
//! assert_eq!(tree.node(field).kind, pdxl_parser::NodeKind::Field);
//! ```

mod diagnostic;
mod parser;
mod render;

pub use diagnostic::{Diagnostic, Parse, Severity};
pub use parser::{parse, parse_gui};
pub use render::render_tree;

// Re-export the tree model so parser consumers get the full vocabulary from one
// import; `pdxl_ast` remains the home of these types.
pub use pdxl_ast::{
    Node, NodeId, NodeKind, SyntaxTree, TextRange, TokenKind, ValidationError, validate_tree,
};

#[cfg(test)]
mod tests;
