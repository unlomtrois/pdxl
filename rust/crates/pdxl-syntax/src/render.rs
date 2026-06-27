//! Golden-tree renderer — test/debug infrastructure, not a primary syntax API.
//!
//! Reproduces the Go `renderTree`/`renderNode` output byte-for-byte so the Rust
//! parser can be checked against the shared `testdata/*.golden` files. Output is
//! built as raw bytes (scalar/tag text is copied verbatim from source) so it
//! stays exact even if a fixture contained non-UTF-8 bytes.

use crate::node::{NodeId, NodeKind, SyntaxTree};

/// Renders a tree to the golden text format (top-level items, recursively).
pub fn render_tree(tree: &SyntaxTree) -> Vec<u8> {
    let mut out = Vec::new();
    for child in tree.children(tree.root()) {
        render_node(&mut out, tree, child, 0);
    }
    out
}

fn indent(out: &mut Vec<u8>, depth: usize) {
    for _ in 0..depth {
        out.extend_from_slice(b"  ");
    }
}

fn render_node(out: &mut Vec<u8>, tree: &SyntaxTree, id: NodeId, depth: usize) {
    let node = tree.node(id);
    match node.kind {
        NodeKind::Field => {
            let children = tree.child_ids(id);
            let key = tree.node_text(children[0]);
            let op = node.op_string();
            let val = children[1];
            let val_node = tree.node(val);
            match val_node.kind {
                NodeKind::Scalar => {
                    indent(out, depth);
                    out.extend_from_slice(key);
                    out.push(b' ');
                    out.extend_from_slice(op.as_bytes());
                    out.push(b' ');
                    out.extend_from_slice(tree.node_text(val));
                    out.push(b'\n');
                }
                NodeKind::TaggedBlock => {
                    indent(out, depth);
                    out.extend_from_slice(key);
                    out.push(b' ');
                    out.extend_from_slice(op.as_bytes());
                    out.push(b' ');
                    out.extend_from_slice(tree.node_text(val));
                    out.extend_from_slice(b" {\n");
                    for item in tree.children(val) {
                        render_node(out, tree, item, depth + 1);
                    }
                    indent(out, depth);
                    out.extend_from_slice(b"}\n");
                }
                NodeKind::Block => {
                    indent(out, depth);
                    out.extend_from_slice(key);
                    out.push(b' ');
                    out.extend_from_slice(op.as_bytes());
                    out.extend_from_slice(b" {\n");
                    for item in tree.children(val) {
                        render_node(out, tree, item, depth + 1);
                    }
                    indent(out, depth);
                    out.extend_from_slice(b"}\n");
                }
                // File / Field as a value can't occur; mirror Go's silent skip.
                _ => {}
            }
        }
        NodeKind::Scalar => {
            indent(out, depth);
            out.extend_from_slice(tree.node_text(id));
            out.push(b'\n');
        }
        NodeKind::Block => {
            // bare block at top level (unusual)
            indent(out, depth);
            out.extend_from_slice(b"{\n");
            for item in tree.children(id) {
                render_node(out, tree, item, depth + 1);
            }
            indent(out, depth);
            out.extend_from_slice(b"}\n");
        }
        // File / TaggedBlock at item level: Go's renderNode has no case, so it
        // emits nothing. Preserve that.
        _ => {}
    }
}
