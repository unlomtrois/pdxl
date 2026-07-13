//! Flat node-pool syntax tree.
//!
//! Ported from `internal/parser/v3`. All nodes live in a single contiguous pool
//! ([`SyntaxTree::nodes`]); parent→child relationships are expressed through a
//! separate child-index array ([`SyntaxTree::child_ids`]) rather than pointers or
//! per-node child vectors. This is the layout that gives the Go parser ~2× fewer
//! allocations than a pointer tree, and it is preserved exactly here.
//!
//! Invariants (checked by `tests`):
//! 1. `nodes[0]` is always the [`NodeKind::File`] root.
//! 2. A node's children are `child_ids[child_start..child_end]`.
//! 3. Each `child_ids` entry indexes `nodes`.
//! 4. Node source ranges stay within `source`.
//! 5. Scalar / tagged-block text is recovered from the source range — never copied.

use std::sync::Arc;

use pdxl_lexer::TokenKind;
use pdxl_src::TextRange;

/// A typed, stable identifier for a node within one [`SyntaxTree`].
///
/// This is an index into [`SyntaxTree::nodes`]. It is `repr(transparent)` over a
/// `u32` so it stays compact, but it is deliberately *not* a bare `usize`: node
/// identity must not be confused with arbitrary indexing.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    /// The file root, always present and always id `0`.
    pub const ROOT: NodeId = NodeId(0);

    /// Creates a node id from a raw index.
    #[inline]
    pub const fn new(raw: u32) -> Self {
        NodeId(raw)
    }

    /// The raw `u32` index.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// The index as a `usize`, for slicing the node pool.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// The kind of a syntax node.
///
/// Discriminants follow the Go `NodeKind` iota order exactly
/// (`File=0 … Scalar=4`) so structured dumps line up.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// Root; children are the top-level items.
    File = 0,
    /// `key OP value`; child 0 is the key [`NodeKind::Scalar`], child 1 the value.
    Field = 1,
    /// `{ … }`; children are the block items.
    Block = 2,
    /// `tag = { … }` style; `range` covers the tag text, children are block items.
    TaggedBlock = 3,
    /// A leaf; `range` is its literal source text.
    Scalar = 4,
}

impl NodeKind {
    /// The kind whose discriminant is `v`, or `None` for an out-of-range byte.
    /// Inverse of `kind as u8`; used by persistent stores (the syntax cache).
    #[inline]
    pub const fn from_u8(v: u8) -> Option<NodeKind> {
        match v {
            0 => Some(NodeKind::File),
            1 => Some(NodeKind::Field),
            2 => Some(NodeKind::Block),
            3 => Some(NodeKind::TaggedBlock),
            4 => Some(NodeKind::Scalar),
            _ => None,
        }
    }

    /// Stable lowercase name used in structured dumps.
    pub const fn as_str(self) -> &'static str {
        match self {
            NodeKind::File => "file",
            NodeKind::Field => "field",
            NodeKind::Block => "block",
            NodeKind::TaggedBlock => "tagged_block",
            NodeKind::Scalar => "scalar",
        }
    }
}

/// A pointer-free syntax node.
///
/// Mirrors the Go `Node`: a kind, a source byte range, a [`Field`](NodeKind::Field)
/// operator, and a half-open range `[child_start, child_end)` into the tree's
/// child-index array. Non-field nodes carry [`TokenKind::Invalid`] as their
/// operator (the Go parser leaves the zero tag there; dumps normalize both to
/// `invalid`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    pub range: TextRange,
    pub operator: TokenKind,
    pub child_start: u32,
    pub child_end: u32,
}

impl Node {
    /// Operator rendered as its source symbol (`=`, `?=`, `>=`, …) for fields.
    ///
    /// Mirrors Go's `Node.OpString`. For non-comparison operators it falls back
    /// to the token kind name.
    pub fn op_string(&self) -> &'static str {
        match self.operator {
            TokenKind::Equal => "=",
            TokenKind::EqualEqual => "==",
            TokenKind::NotEqual => "!=",
            TokenKind::QuestionEqual => "?=",
            TokenKind::GreaterThan => ">",
            TokenKind::GreaterEqual => ">=",
            TokenKind::LessThan => "<",
            TokenKind::LessEqual => "<=",
            other => other.as_str(),
        }
    }
}

/// The result of parsing: a flat node pool, a child-index array, and the source.
///
/// `nodes[0]` is always the [`NodeKind::File`] root. The tree shares its source
/// via `Arc<[u8]>` and never exposes a self-referential lifetime.
pub struct SyntaxTree {
    source: Arc<[u8]>,
    nodes: Box<[Node]>,
    child_ids: Box<[NodeId]>,
}

impl SyntaxTree {
    /// Assembles a tree from its raw parts.
    ///
    /// This is the construction path for tree *producers* — the parser in
    /// `pdxl-parser`, and (later) the syntax cache's deserializer. The caller is
    /// responsible for upholding the layout invariants documented on the crate;
    /// [`validate_tree`](crate::validate_tree) can check them after the fact.
    pub fn from_parts(source: Arc<[u8]>, nodes: Box<[Node]>, child_ids: Box<[NodeId]>) -> Self {
        SyntaxTree {
            source,
            nodes,
            child_ids,
        }
    }

    /// The source bytes this tree was parsed from.
    #[inline]
    pub fn source(&self) -> &[u8] {
        &self.source
    }

    /// A cheap clone of the shared source handle.
    #[inline]
    pub fn source_arc(&self) -> Arc<[u8]> {
        Arc::clone(&self.source)
    }

    /// The file root node id (always [`NodeId::ROOT`]).
    #[inline]
    pub fn root(&self) -> NodeId {
        NodeId::ROOT
    }

    /// The total number of nodes in the pool.
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the pool is empty. Never true for a parsed tree (the root exists).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Borrows a node by id.
    #[inline]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    /// All nodes in pool (allocation) order. Primarily for dumps and validation.
    #[inline]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The complete child-index array. Primarily for dumps and validation.
    #[inline]
    pub fn child_index(&self) -> &[NodeId] {
        &self.child_ids
    }

    /// The source text of a node (`source[range]`), without copying.
    #[inline]
    pub fn node_text(&self, id: NodeId) -> &[u8] {
        self.node(id).range.slice(&self.source)
    }

    /// The immediate child ids of a node, as a borrowed slice. Allocation-free.
    #[inline]
    pub fn child_ids(&self, id: NodeId) -> &[NodeId] {
        let n = self.node(id);
        &self.child_ids[n.child_start as usize..n.child_end as usize]
    }

    /// Iterates a node's immediate children by id. Allocation-free.
    #[inline]
    pub fn children(&self, id: NodeId) -> impl ExactSizeIterator<Item = NodeId> + '_ {
        self.child_ids(id).iter().copied()
    }
}
