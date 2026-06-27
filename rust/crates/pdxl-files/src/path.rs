//! Path helpers ported to match Go's `path/filepath` semantics exactly.
//!
//! The Go `files` package relies on `filepath.Clean`, `filepath.Join`,
//! `filepath.ToSlash`, and `strings.ToLower` for overlay keys and stored paths.
//! Rust's `std::path` does not clean lexically (`PathBuf::join` keeps `.`/`..`),
//! so for byte-for-byte differential parity we reproduce Go's lexical algorithms
//! here rather than relying on `std::path` canonicalization.
//!
//! These functions operate on the test platform's native separator. The
//! differential harness runs on Linux (separator `/`), where `filepath.ToSlash`
//! is the identity; the slash conversion is kept for portability.

/// The native path separator as a `char` (`/` on Unix).
const SEP: char = std::path::MAIN_SEPARATOR;

/// Converts native separators to `/`, matching `filepath.ToSlash`.
pub fn to_slash(s: &str) -> String {
    if SEP == '/' {
        s.to_string()
    } else {
        s.replace(SEP, "/")
    }
}

/// Lowercases a string the way Go's `strings.ToLower` does for the overlay key.
///
/// Go applies simple per-rune lowercase mapping (not case folding). Rust's
/// `char::to_lowercase` is full Unicode lowercase, which can expand one char to
/// several (e.g. `İ` → `i` + combining dot). For the overlay key we take the
/// mapping as-is; in practice PDXScript paths are ASCII or accented Latin/Greek
/// where simple and full mapping agree. Exotic 1:many mappings are a documented
/// limitation (see the milestone report).
pub fn to_lower(s: &str) -> String {
    s.to_lowercase()
}

/// Normalizes a native relative path into an overlay key: `to_slash` then
/// `to_lower`, matching Go's `strings.ToLower(filepath.ToSlash(rel))`.
pub fn normalize_key(s: &str) -> String {
    to_lower(&to_slash(s))
}

/// Lexically cleans a slash/native path, matching Go `path/filepath.Clean`
/// (Unix). Pure string manipulation: no filesystem access, no `.`/`..`
/// resolution against the real tree.
pub fn clean(path: &str) -> String {
    let s = path.as_bytes();
    if s.is_empty() {
        return ".".to_string();
    }
    let rooted = s[0] == b'/';
    let n = s.len();
    // Output never exceeds input length.
    let mut buf = vec![0u8; n];
    let mut w = 0usize;
    let mut r = 0usize;
    let mut dotdot = 0usize;

    if rooted {
        buf[w] = b'/';
        w += 1;
        r = 1;
        dotdot = 1;
    }

    while r < n {
        if s[r] == b'/' {
            // empty path element
            r += 1;
        } else if s[r] == b'.' && (r + 1 == n || s[r + 1] == b'/') {
            // . element
            r += 1;
        } else if s[r] == b'.' && r + 1 < n && s[r + 1] == b'.' && (r + 2 == n || s[r + 2] == b'/')
        {
            // .. element: back up to the previous slash
            r += 2;
            if w > dotdot {
                w -= 1;
                while w > dotdot && buf[w] != b'/' {
                    w -= 1;
                }
            } else if !rooted {
                if w > 0 {
                    buf[w] = b'/';
                    w += 1;
                }
                buf[w] = b'.';
                w += 1;
                buf[w] = b'.';
                w += 1;
                dotdot = w;
            }
        } else {
            // real path element; add separator if needed
            if (rooted && w != 1) || (!rooted && w != 0) {
                buf[w] = b'/';
                w += 1;
            }
            while r < n && s[r] != b'/' {
                buf[w] = s[r];
                w += 1;
                r += 1;
            }
        }
    }

    if w == 0 {
        return ".".to_string();
    }
    // The buffer only ever holds bytes copied verbatim from the (UTF-8) input.
    String::from_utf8_lossy(&buf[..w]).into_owned()
}

/// Joins path elements and cleans the result, matching `filepath.Join`.
///
/// Empty elements are ignored; an all-empty join yields `""`.
pub fn join(parts: &[&str]) -> String {
    let mut acc = String::new();
    for p in parts {
        if p.is_empty() {
            continue;
        }
        if acc.is_empty() {
            acc.push_str(p);
        } else {
            acc.push('/');
            acc.push_str(p);
        }
    }
    if acc.is_empty() {
        String::new()
    } else {
        clean(&acc)
    }
}

/// Returns the directory of a path, matching `filepath.Dir` (Unix).
pub fn dir(path: &str) -> String {
    match path.rfind('/') {
        Some(idx) => {
            let head = &path[..=idx]; // include the trailing slash like Go before Clean
            clean(head)
        }
        None => ".".to_string(),
    }
}

/// Reports whether `p` looks like a Windows absolute path (e.g. `C:/...`).
///
/// Matches Go `IsWindowsAbsolute`: length ≥ 3, byte 1 is `:`, byte 2 is `/`
/// or `\`. Drive letter is not validated (any letter, any case).
///
/// Retained because [`parse_mod`](crate::parse_mod) uses it to decide whether a
/// descriptor's `path=` is absolute (kept verbatim) or relative (joined to the
/// `.mod` directory). The Go `ResolveWindowsPath` Proton/`drive_c` translation is
/// intentionally **not** ported: this project references mods by local folder
/// path, never via a Steam/Proton drive path (see the milestone report).
pub fn is_windows_absolute(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 3 && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\')
}

/// Returns the case-insensitive `.txt` extension check matching
/// `strings.EqualFold(filepath.Ext(name), ".txt")`.
pub fn has_txt_ext(name: &str) -> bool {
    // filepath.Ext: suffix from the final '.' in the final element ("" if none).
    let ext = match name.rfind('.') {
        Some(idx) => &name[idx..],
        None => "",
    };
    ext.eq_ignore_ascii_case(".txt")
}
