//! `.mod` descriptor parsing, via `pdxl-syntax` (no second ad-hoc parser).
//!
//! Faithful port of Go `ParseMod`: tolerant (parse diagnostics are ignored),
//! recognizes only `name` / `path` / `replace_path` direct-child fields, trims
//! surrounding quotes, preserves `replace_path` order and duplicates, and lets a
//! later `name`/`path` overwrite an earlier one. The mod directory `path` is
//! resolved relative to the `.mod` file's directory unless it is a Windows
//! absolute path (kept verbatim).

use std::io;
use std::path::{Path, PathBuf};

use pdxl_parser::{NodeKind, parse};

use pdxl_path::{dir, is_windows_absolute, join};

/// Metadata parsed from a `.mod` file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModDescriptor {
    pub name: String,
    /// Resolved mod directory path. Not canonicalized.
    pub path: PathBuf,
    /// Relative paths that fully replace vanilla, in file order (duplicates kept).
    pub replace_paths: Vec<String>,
}

/// Reads and parses a CK3 `.mod` descriptor.
///
/// `mod_file` is the path to the `.mod` file itself; `ModDescriptor::path` is
/// resolved relative to that file's directory. Parse diagnostics are ignored,
/// matching Go's tolerant behavior — a malformed-but-readable descriptor still
/// yields whatever facts its partial syntax tree provides.
pub fn parse_mod(mod_file: impl AsRef<Path>) -> io::Result<ModDescriptor> {
    let mod_file = mod_file.as_ref();
    let src = std::fs::read(mod_file)?;
    let parsed = parse(mod_file.to_string_lossy().into_owned(), src);
    let tree = parsed.tree();

    let mut m = ModDescriptor::default();
    let mod_dir = dir(&mod_file.to_string_lossy());

    for child in tree.children(tree.root()) {
        let node = tree.node(child);
        if node.kind != NodeKind::Field {
            continue;
        }
        let field_children = tree.child_ids(child);
        if field_children.len() < 2 {
            continue;
        }
        let key = tree.node_text(field_children[0]);
        let val = tree.node_text(field_children[1]);
        match key {
            b"name" => m.name = trim_quotes(val).to_string(),
            b"path" => {
                let raw = trim_quotes(val);
                if is_windows_absolute(raw) {
                    m.path = PathBuf::from(raw);
                } else {
                    // Join(dir(modFile), FromSlash(raw)) — FromSlash is identity
                    // on Unix; join() cleans like filepath.Join.
                    m.path = PathBuf::from(join(&[&mod_dir, raw]));
                }
            }
            b"replace_path" => m.replace_paths.push(trim_quotes(val).to_string()),
            _ => {}
        }
    }
    Ok(m)
}

/// Trims a single layer of surrounding ASCII double quotes, matching Go's
/// `strings.Trim(val, "\"")` (which strips any run of leading/trailing quotes).
///
/// Returns a `&str`; the recognized scalar values are valid UTF-8 in practice.
/// On the rare chance of non-UTF-8 bytes, falls back to a lossy view's trimmed
/// form would change bytes, so we instead trim on the raw bytes and decode.
fn trim_quotes(val: &[u8]) -> &str {
    let trimmed = trim_quote_bytes(val);
    // Recognized .mod scalar values are UTF-8; lossy decode only on bad input.
    std::str::from_utf8(trimmed).unwrap_or("")
}

/// Strips all leading and trailing `"` bytes (Go `strings.Trim` semantics).
fn trim_quote_bytes(mut b: &[u8]) -> &[u8] {
    while b.first() == Some(&b'"') {
        b = &b[1..];
    }
    while b.last() == Some(&b'"') {
        b = &b[..b.len() - 1];
    }
    b
}
