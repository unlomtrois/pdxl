//! Canonical whole-project analysis dump, matching `tools/projectdump/main.go`
//! byte-for-byte: symbol counts by kind (stable order), duplicates in merge
//! order, unresolved-reference diagnostics in walk order.

use pdxl_analysis::{KindId, RefDiag, SymbolTable};

/// Project dump schema version. Bump on any format change.
pub const PROJECT_DUMP_VERSION: u32 = 1;

/// Renders the canonical dump of one whole-project analysis. `kinds` is the
/// schema's kind order (stable counts ordering).
pub fn dump_project(table: &SymbolTable, diags: &[RefDiag], kinds: &[KindId]) -> String {
    let mut out = String::new();
    out.push_str("{\n\"version\":");
    out.push_str(&PROJECT_DUMP_VERSION.to_string());
    out.push_str(",\n\"counts\":{");
    for (i, kind) in kinds.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(kind.name());
        out.push_str("\":");
        out.push_str(&table.count(*kind).to_string());
    }
    out.push_str(",\"total\":");
    out.push_str(&table.total().to_string());
    out.push_str("},\n\"duplicates\":[");
    if !table.duplicates.is_empty() {
        out.push('\n');
        for (i, d) in table.duplicates.iter().enumerate() {
            out.push_str("{\"kind\":\"");
            out.push_str(d.kind.name());
            out.push_str("\",\"name\":\"");
            push_escaped(&mut out, &d.name);
            out.push_str("\",\"first_file\":\"");
            push_escaped(&mut out, &d.first.file);
            out.push_str("\",\"file\":\"");
            push_escaped(&mut out, &d.file);
            out.push_str("\"}");
            if i + 1 < table.duplicates.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("],\n\"unresolved\":[");
    if !diags.is_empty() {
        out.push('\n');
        for (i, d) in diags.iter().enumerate() {
            out.push_str("{\"file\":\"");
            push_escaped(&mut out, &d.file);
            out.push_str("\",\"start\":");
            out.push_str(&d.start.to_string());
            out.push_str(",\"end\":");
            out.push_str(&d.end.to_string());
            out.push_str(",\"msg\":\"");
            push_escaped(&mut out, &d.msg);
            out.push_str("\"}");
            if i + 1 < diags.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("]\n}\n");
    out
}

fn push_escaped(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
}
