//! Canonical FileFacts dump, matching `tools/factsdump/main.go` byte-for-byte.

use pdxl_analysis::{FileFacts, Symbol};

/// Facts dump schema version. Bump on any format change.
pub const FACTS_DUMP_VERSION: u32 = 1;

/// Renders the canonical dump of one extraction run.
pub fn dump_facts(facts: &FileFacts, rel_path: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n\"version\":");
    out.push_str(&FACTS_DUMP_VERSION.to_string());
    out.push_str(",\n\"rel_path\":\"");
    push_escaped(&mut out, rel_path);
    out.push_str("\",\n\"defs\":[");
    push_symbols(&mut out, &facts.defs);
    out.push_str("],\n\"aliases\":[");
    push_symbols(&mut out, &facts.aliases);
    out.push_str("],\n\"refs\":[");
    if !facts.refs.is_empty() {
        out.push('\n');
        for (i, r) in facts.refs.iter().enumerate() {
            out.push_str("{\"kind\":\"");
            out.push_str(r.kind.name());
            out.push_str("\",\"name\":\"");
            push_escaped(&mut out, &r.name);
            out.push_str("\",\"file\":\"");
            push_escaped(&mut out, &r.file);
            out.push_str("\",\"start\":");
            out.push_str(&r.start.to_string());
            out.push_str(",\"end\":");
            out.push_str(&r.end.to_string());
            out.push('}');
            if i + 1 < facts.refs.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("]\n}\n");
    out
}

fn push_symbols(out: &mut String, symbols: &[Symbol]) {
    if symbols.is_empty() {
        return;
    }
    out.push('\n');
    for (i, s) in symbols.iter().enumerate() {
        out.push_str("{\"name\":\"");
        push_escaped(out, &s.name);
        out.push_str("\",\"kind\":\"");
        out.push_str(s.kind.name());
        out.push_str("\",\"file\":\"");
        push_escaped(out, &s.file);
        out.push_str("\",\"offset\":");
        out.push_str(&s.offset.to_string());
        out.push_str(",\"end_offset\":");
        out.push_str(&s.end_offset.to_string());
        out.push_str(",\"params\":[");
        for (j, p) in s.params.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push('"');
            push_escaped(out, p);
            out.push('"');
        }
        out.push_str("]}");
        if i + 1 < symbols.len() {
            out.push(',');
        }
        out.push('\n');
    }
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
