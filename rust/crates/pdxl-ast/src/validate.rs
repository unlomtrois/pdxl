//! Structural invariant validator for a [`SyntaxTree`].
//!
//! Used by tests against every fixture (valid and malformed) to guarantee the
//! flat-pool invariants hold regardless of recovery paths.

use crate::node::{NodeKind, SyntaxTree};

/// A violated tree invariant, with a human-readable description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError(pub String);

/// Verifies the structural invariants of `tree`.
///
/// Checks: non-empty pool; root id 0 is a `File`; every node range is well-formed
/// and within source; every child range indexes the child-index array; every
/// child id indexes the node pool; fields have exactly two children with a scalar
/// key; scalars have no children.
pub fn validate_tree(tree: &SyntaxTree) -> Result<(), ValidationError> {
    let nodes = tree.nodes();
    let child_index = tree.child_index();
    let source_len = tree.source().len() as u64;

    if nodes.is_empty() {
        return Err(ValidationError("node pool is empty".to_string()));
    }
    if tree.root().raw() != 0 {
        return Err(ValidationError("root id is not 0".to_string()));
    }
    if nodes[0].kind != NodeKind::File {
        return Err(ValidationError(format!(
            "root kind is {:?}, expected File",
            nodes[0].kind
        )));
    }

    for (i, node) in nodes.iter().enumerate() {
        if node.range.start > node.range.end {
            return Err(ValidationError(format!(
                "node {i}: start {} > end {}",
                node.range.start, node.range.end
            )));
        }
        if node.range.end as u64 > source_len {
            return Err(ValidationError(format!(
                "node {i}: end {} exceeds source length {source_len}",
                node.range.end
            )));
        }
        if node.child_start > node.child_end {
            return Err(ValidationError(format!(
                "node {i}: child_start {} > child_end {}",
                node.child_start, node.child_end
            )));
        }
        if node.child_end as usize > child_index.len() {
            return Err(ValidationError(format!(
                "node {i}: child_end {} exceeds child_index length {}",
                node.child_end,
                child_index.len()
            )));
        }

        match node.kind {
            NodeKind::Field => {
                let n = node.child_end - node.child_start;
                if n != 2 {
                    return Err(ValidationError(format!(
                        "field node {i}: expected 2 children, got {n}"
                    )));
                }
                let key_id = child_index[node.child_start as usize];
                if tree.node(key_id).kind != NodeKind::Scalar {
                    return Err(ValidationError(format!(
                        "field node {i}: child 0 is {:?}, expected Scalar",
                        tree.node(key_id).kind
                    )));
                }
            }
            NodeKind::Scalar if node.child_start != node.child_end => {
                return Err(ValidationError(format!(
                    "scalar node {i}: must have no children, has {}",
                    node.child_end - node.child_start
                )));
            }
            // File / Block / TaggedBlock may have zero or more children.
            _ => {}
        }
    }

    for (i, id) in child_index.iter().enumerate() {
        if id.index() >= nodes.len() {
            return Err(ValidationError(format!(
                "child_index[{i}] = {} out of range (nodes len {})",
                id.raw(),
                nodes.len()
            )));
        }
    }

    Ok(())
}
