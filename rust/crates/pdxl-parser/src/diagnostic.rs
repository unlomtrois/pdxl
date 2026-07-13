//! Parse diagnostics and the [`Parse`] result.
//!
//! Parsing an editor buffer that is mid-edit (and therefore malformed) is normal,
//! so [`parse`](crate::parse) always returns a tree plus a list of diagnostics —
//! never a `Result` that throws the partial tree away. A non-empty diagnostics
//! list means errors were found but parsing continued as far as possible.

use std::sync::Arc;

use pdxl_ast::SyntaxTree;

/// How serious a diagnostic is. Discriminants follow the Go `Severity` iota.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error = 0,
    Warning = 1,
}

impl Severity {
    /// Stable lowercase name used in structured dumps.
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// A parse problem with a source location.
///
/// `offset` is a zero-based byte offset into the source (convert to line:column
/// only at a display boundary). `filename` is shared via `Arc<str>` so it is not
/// copied per diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub filename: Arc<str>,
    pub offset: u32,
    pub message: String,
    pub severity: Severity,
}

/// The result of [`parse`](crate::parse): a (possibly partial) tree and its
/// diagnostics.
pub struct Parse {
    pub(crate) tree: SyntaxTree,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// The parsed syntax tree (always present, even on malformed input).
    #[inline]
    pub fn tree(&self) -> &SyntaxTree {
        &self.tree
    }

    /// The accumulated diagnostics, in the order they were produced.
    #[inline]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether any diagnostics were recorded.
    #[inline]
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Decomposes into the owned tree and diagnostics.
    #[inline]
    pub fn into_parts(self) -> (SyntaxTree, Vec<Diagnostic>) {
        (self.tree, self.diagnostics)
    }
}
