//! Experimental schema-aware field ordering.

use pdxl_analysis::context::{ClauseKind, ContextSchema, context_at};
use pdxl_ast::{NodeId, NodeKind, SyntaxTree};

#[derive(Debug)]
struct Operation {
    start: usize,
    end: usize,
    chunks: Vec<(usize, usize, usize)>, // schema rank, source start, source end
}

/// Reorders fields in recognized structural blocks according to their
/// `StructSpec.fields` order. Input must already be layout-formatted: one field
/// per line and expanded structural blocks.
pub(crate) fn reorder(
    src: &str,
    tree: &SyntaxTree,
    rel_path: &str,
    schema: &ContextSchema,
) -> String {
    let line_starts = line_starts(src);
    let lines: Vec<&str> = src.split_inclusive('\n').collect();
    let brace_pairs = brace_pairs(src);
    let mut operations = Vec::new();

    for raw in 0..tree.len() {
        let id = NodeId::new(raw as u32);
        if tree.node(id).kind != NodeKind::Block {
            continue;
        }
        let ClauseKind::Struct(spec) = context_at(tree, id, rel_path, schema) else {
            continue;
        };
        let fields: Vec<NodeId> = tree
            .children(id)
            .filter(|child| tree.node(*child).kind == NodeKind::Field)
            .collect();
        if fields.len() < 2 || fields.len() != tree.child_ids(id).len() {
            continue;
        }

        let first = tree.node(fields[0]).range.start as usize;
        let last = tree.node(*fields.last().unwrap()).range.end as usize;
        let Some(&(open, close)) = brace_pairs
            .iter()
            .filter(|(open, close)| *open < first && *close >= last)
            .min_by_key(|(open, close)| close - open)
        else {
            continue;
        };
        let open_line = line_of(&line_starts, open);
        let close_line = line_of(&line_starts, close);
        if close_line <= open_line + 1 {
            continue;
        }

        let mut starts = Vec::with_capacity(fields.len());
        starts.push(open_line + 1);
        for (previous, field) in fields.iter().zip(fields.iter().skip(1)) {
            let previous_end = line_of(
                &line_starts,
                tree.node(*previous).range.end.saturating_sub(1) as usize,
            ) + 1;
            let mut start = line_of(&line_starts, tree.node(*field).range.start as usize);
            while start > previous_end {
                let prior = lines[start - 1].trim();
                if prior.is_empty() || prior.starts_with('#') {
                    start -= 1;
                } else {
                    break;
                }
            }
            starts.push(start);
        }

        let mut chunks = Vec::with_capacity(fields.len());
        for (index, field) in fields.iter().enumerate() {
            let start_line = starts[index];
            let end_line = starts.get(index + 1).copied().unwrap_or(close_line);
            let key = tree
                .child_ids(*field)
                .first()
                .map(|key| tree.node_text(*key));
            let rank = key
                .and_then(|key| std::str::from_utf8(key).ok())
                .and_then(|key| spec.fields.iter().position(|(name, _)| *name == key))
                .unwrap_or(usize::MAX);
            chunks.push((rank, line_starts[start_line], line_starts[end_line]));
        }
        let already_sorted = chunks.windows(2).all(|pair| pair[0].0 <= pair[1].0);
        if !already_sorted {
            chunks.sort_by_key(|(rank, _, _)| *rank); // stable: repeated/unknown fields retain order
            operations.push(Operation {
                start: line_starts[open_line + 1],
                end: line_starts[close_line],
                chunks,
            });
        }
    }

    render_range(src, 0, src.len(), &operations)
}

/// Renders a range while recursively applying its direct child operations.
/// This avoids overlapping replacement bugs when both a struct and one of its
/// nested structs need ordering.
fn render_range(src: &str, start: usize, end: usize, operations: &[Operation]) -> String {
    let mut direct: Vec<_> = operations
        .iter()
        .filter(|op| {
            op.start >= start
                && op.end <= end
                && !operations.iter().any(|parent| {
                    parent.start >= start
                        && parent.end <= end
                        && parent.start <= op.start
                        && parent.end >= op.end
                        && (parent.start != op.start || parent.end != op.end)
                })
        })
        .collect();
    direct.sort_by_key(|op| op.start);
    let mut out = String::with_capacity(end - start);
    let mut cursor = start;
    for op in direct {
        out.push_str(&src[cursor..op.start]);
        for &(_, chunk_start, chunk_end) in &op.chunks {
            out.push_str(&render_range(src, chunk_start, chunk_end, operations));
        }
        cursor = op.end;
    }
    out.push_str(&src[cursor..end]);
    out
}

fn line_starts(src: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(src.match_indices('\n').map(|(at, _)| at + 1));
    starts
}

fn line_of(starts: &[usize], offset: usize) -> usize {
    starts
        .partition_point(|start| *start <= offset)
        .saturating_sub(1)
}

fn brace_pairs(src: &str) -> Vec<(usize, usize)> {
    let mut stack = Vec::new();
    let mut pairs = Vec::new();
    for token in pdxl_lexer::tokenize(src.as_bytes()) {
        match token.kind {
            pdxl_lexer::TokenKind::LBrace => stack.push(token.range.start as usize),
            pdxl_lexer::TokenKind::RBrace => {
                if let Some(open) = stack.pop() {
                    pairs.push((open, token.range.start as usize));
                }
            }
            _ => {}
        }
    }
    pairs
}
