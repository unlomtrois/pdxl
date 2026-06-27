//! Deterministic, normalized structured dump of a parse result.
//!
//! Emitted identically by the Rust parser and the Go oracle (`tools/parsedump`)
//! so the two can be compared byte-for-byte. The format is JSON with one node and
//! one diagnostic per line, so a line diff pinpoints the first divergent node.
//!
//! Normalization rules (must match the Go tool exactly):
//! - `kind` is the lowercase [`NodeKind::as_str`] name.
//! - `operator` is the token name only for [`NodeKind::Field`]; every other node
//!   reports `"invalid"` (the Go parser leaves an unset tag on non-field nodes).
//! - byte offsets are the zero-based, half-open values from the tree.
//! - filenames are deliberately omitted: a dump must not depend on checkout path.
//!
//! Exact range equality already proves node-text equality when both parsers
//! consume the same source bytes, so source text is not embedded.

use crate::diagnostic::Parse;
use crate::node::NodeKind;

/// The dump schema version. Bump on any format change.
pub const DUMP_VERSION: u32 = 1;

/// Renders the canonical structured dump string for a parse result.
pub fn dump_json(parse: &Parse) -> String {
    let tree = parse.tree();
    let nodes = tree.nodes();
    let child_ids = tree.child_index();
    let diags = parse.diagnostics();

    let mut out = String::new();
    out.push('{');
    out.push_str("\"version\":");
    out.push_str(&DUMP_VERSION.to_string());
    out.push_str(",\"source_length\":");
    out.push_str(&tree.source().len().to_string());
    out.push_str(",\"nodes\":[");
    if !nodes.is_empty() {
        out.push('\n');
        for (i, node) in nodes.iter().enumerate() {
            let operator = if node.kind == NodeKind::Field {
                node.operator.as_str()
            } else {
                "invalid"
            };
            out.push_str("{\"id\":");
            out.push_str(&i.to_string());
            out.push_str(",\"kind\":\"");
            out.push_str(node.kind.as_str());
            out.push_str("\",\"start\":");
            out.push_str(&node.range.start.to_string());
            out.push_str(",\"end\":");
            out.push_str(&node.range.end.to_string());
            out.push_str(",\"operator\":\"");
            out.push_str(operator);
            out.push_str("\",\"child_start\":");
            out.push_str(&node.child_start.to_string());
            out.push_str(",\"child_end\":");
            out.push_str(&node.child_end.to_string());
            out.push('}');
            if i + 1 < nodes.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("],\"child_ids\":[");
    for (i, id) in child_ids.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&id.raw().to_string());
    }
    out.push_str("],\"diagnostics\":[");
    if !diags.is_empty() {
        out.push('\n');
        for (i, d) in diags.iter().enumerate() {
            out.push_str("{\"offset\":");
            out.push_str(&d.offset.to_string());
            out.push_str(",\"severity\":\"");
            out.push_str(d.severity.as_str());
            out.push_str("\",\"message\":\"");
            push_json_escaped(&mut out, &d.message);
            out.push_str("\"}");
            if i + 1 < diags.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("]}\n");
    out
}

/// Appends `s` to `out` with minimal JSON string escaping (matching the Go tool).
fn push_json_escaped(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
}
