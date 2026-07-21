//! Deterministic structured dumps for FileSet / descriptor differential testing.
//!
//! Emitted identically by the Rust crate and the Go oracle (`tools/filesetdump`)
//! so the two can be compared byte-for-byte. JSON with one entry / resolution per
//! line. Entries are in exact `iter` (winner) order — never re-sorted.

use pdxl_fileset::FileSet;
use pdxl_moddesc::ModDescriptor;
use pdxl_path::is_windows_absolute;

/// Dump schema version. Bump on any format change.
pub const FILESET_DUMP_VERSION: u32 = 1;

/// Renders the canonical scan dump: entries (winner order), stats, resolutions.
pub fn dump_scan(fs: &FileSet, queries: &[String]) -> String {
    let mut out = String::new();
    out.push_str("{\n\"version\":");
    out.push_str(&FILESET_DUMP_VERSION.to_string());
    out.push_str(",\n\"entries\":[");

    let entries: Vec<_> = fs.iter().collect();
    if !entries.is_empty() {
        out.push('\n');
        for (i, e) in entries.iter().enumerate() {
            out.push_str("{\"rel_path\":\"");
            push_escaped(&mut out, &e.rel_path);
            out.push_str("\",\"full_path\":\"");
            push_escaped(&mut out, &e.full_path.to_string_lossy());
            out.push_str("\",\"kind\":\"");
            out.push_str(e.kind.as_str());
            out.push_str("\"}");
            if i + 1 < entries.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("],\n");

    let st = fs.stats();
    out.push_str("\"stats\":{\"vanilla\":");
    out.push_str(&st.vanilla.to_string());
    out.push_str(",\"mod\":");
    out.push_str(&st.mod_files.to_string());
    out.push_str(",\"total\":");
    out.push_str(&st.total.to_string());
    out.push_str(",\"shadowed\":");
    out.push_str(&st.shadowed.to_string());
    out.push_str(",\"replaced\":");
    out.push_str(&st.replaced.to_string());
    out.push_str("},\n");

    out.push_str("\"resolutions\":[");
    if !queries.is_empty() {
        out.push('\n');
        for (i, q) in queries.iter().enumerate() {
            out.push_str("{\"query\":\"");
            push_escaped(&mut out, q);
            match fs.resolve(q) {
                Some(e) => {
                    out.push_str("\",\"found\":true,\"rel_path\":\"");
                    push_escaped(&mut out, &e.rel_path);
                    out.push_str("\",\"kind\":\"");
                    out.push_str(e.kind.as_str());
                    out.push_str("\"}");
                }
                None => {
                    out.push_str("\",\"found\":false,\"rel_path\":null,\"kind\":null}");
                }
            }
            if i + 1 < queries.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("]\n}\n");
    out
}

/// Renders the canonical descriptor dump.
pub fn dump_descriptor(m: &ModDescriptor) -> String {
    let path_str = m.path.to_string_lossy();
    let mut out = String::new();
    out.push_str("{\n\"version\":");
    out.push_str(&FILESET_DUMP_VERSION.to_string());
    out.push_str(",\n\"name\":\"");
    push_escaped(&mut out, &m.name);
    out.push_str("\",\n\"path\":\"");
    push_escaped(&mut out, &path_str);
    out.push_str("\",\n\"replace_paths\":[");
    for (i, rp) in m.replace_paths.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        push_escaped(&mut out, rp);
        out.push('"');
    }
    out.push_str("],\n\"is_windows_absolute\":");
    out.push_str(if is_windows_absolute(&path_str) {
        "true"
    } else {
        "false"
    });
    out.push_str("\n}\n");
    out
}

/// Appends `s` with minimal JSON string escaping (matching the Go tool).
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
