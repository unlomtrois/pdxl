//! Mod descriptor parsing — both Paradox generations:
//!
//! - **`.mod` files** (CK3-era), via `pdxl-syntax` (no second ad-hoc parser).
//!   Faithful port of Go `ParseMod`: tolerant (parse diagnostics are
//!   ignored), recognizes only `name` / `path` / `replace_path` direct-child
//!   fields, trims surrounding quotes, preserves `replace_path` order and
//!   duplicates, and lets a later `name`/`path` overwrite an earlier one. The
//!   mod directory `path` is resolved relative to the `.mod` file's directory
//!   unless it is a Windows absolute path (kept verbatim).
//! - **`.metadata/metadata.json`** (VIC3/EU5-era): the descriptor lives
//!   *inside* the mod directory; `replace_paths` come from
//!   `game_custom_data.replace_paths` when present. Launcher-written files
//!   start with a UTF-8 BOM, which is stripped before JSON parsing.
//!
//! [`resolve_mod`] dispatches between the two by what the argument points at.

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
                // Absolute paths are kept verbatim: Windows-shaped (C:/...) for
                // Proton-managed descriptors, and native absolute paths — the
                // Linux launcher writes absolute Unix paths, which used to be
                // wrongly joined onto the .mod directory (fixed in Go in
                // lockstep; see the M7 report).
                if is_windows_absolute(raw) || raw.starts_with('/') {
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

/// Reads and parses a VIC3/EU5-style `.metadata/metadata.json` descriptor.
///
/// `arg` may be the mod directory (containing `.metadata/metadata.json`), the
/// `.metadata` directory, or the `metadata.json` file itself;
/// `ModDescriptor::path` is always the mod directory. Only `name` and
/// `game_custom_data.replace_paths` are read; everything else in the file is
/// launcher metadata.
pub fn parse_metadata_json(arg: impl AsRef<Path>) -> io::Result<ModDescriptor> {
    let arg = arg.as_ref();
    let (json_file, mod_dir) = if arg.is_dir() {
        if arg.file_name().is_some_and(|n| n == ".metadata") {
            (arg.join("metadata.json"), parent_of(arg)?)
        } else {
            (
                arg.join(".metadata").join("metadata.json"),
                arg.to_path_buf(),
            )
        }
    } else {
        // <mod>/.metadata/metadata.json → the mod dir is two levels up.
        (arg.to_path_buf(), parent_of(&parent_of(arg)?)?)
    };

    let bytes = std::fs::read(&json_file)?;
    // Launcher-written metadata.json starts with a UTF-8 BOM.
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {e}", json_file.display()),
        )
    })?;

    let mut m = ModDescriptor {
        path: mod_dir,
        ..ModDescriptor::default()
    };
    if let Some(name) = value.get("name").and_then(|v| v.as_str()) {
        m.name = name.to_string();
    }
    if let Some(paths) = value
        .get("game_custom_data")
        .and_then(|d| d.get("replace_paths"))
        .and_then(|v| v.as_array())
    {
        m.replace_paths = paths
            .iter()
            .filter_map(|p| p.as_str())
            .map(str::to_string)
            .collect();
    }
    Ok(m)
}

/// Resolves a mod argument of any supported form into a [`ModDescriptor`]:
/// a `.mod` file (CK3-era), a directory containing `.metadata/metadata.json`
/// (VIC3/EU5-era, including the `.metadata` dir or the json itself), or a
/// plain content directory (no descriptor — path only).
pub fn resolve_mod(arg: impl AsRef<Path>) -> io::Result<ModDescriptor> {
    let arg = arg.as_ref();
    let meta = std::fs::metadata(arg)?;
    if meta.is_file() {
        if arg
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("mod"))
        {
            return parse_mod(arg);
        }
        if arg.file_name().is_some_and(|n| n == "metadata.json") {
            return parse_metadata_json(arg);
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a mod descriptor: {}", arg.display()),
        ));
    }
    if arg.join(".metadata").join("metadata.json").is_file()
        || arg.file_name().is_some_and(|n| n == ".metadata")
    {
        return parse_metadata_json(arg);
    }
    // A plain content directory: no descriptor, no replace paths.
    Ok(ModDescriptor {
        path: arg.to_path_buf(),
        ..ModDescriptor::default()
    })
}

fn parent_of(p: &Path) -> io::Result<PathBuf> {
    p.parent().map(Path::to_path_buf).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} has no parent directory", p.display()),
        )
    })
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
