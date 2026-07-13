//! The UTF-16 protocol boundary — the ONLY place in the workspace where byte
//! offsets become LSP positions and back. Ported exactly from Go
//! `internal/lsp/position.go`: 0-based lines split on `\n`, characters counted
//! in UTF-16 code units (surrogate pairs count as 2), clamping semantics
//! preserved. Everything below this file speaks byte offsets.

use lsp_types::{Position, Url};
use std::path::{Path, PathBuf};

/// Converts a byte offset in `text` to an LSP position.
pub fn offset_to_position(text: &[u8], off: u32) -> Position {
    let off = (off as usize).min(text.len());
    let (mut line, mut col) = (0u32, 0u32);
    let mut i = 0;
    while i < off {
        let (r, size) = decode(&text[i..]);
        if r == '\n' as u32 {
            line += 1;
            col = 0;
        } else {
            col += utf16_len(r);
        }
        i += size;
    }
    Position {
        line,
        character: col,
    }
}

/// Converts an LSP position to a byte offset in `text`; `text.len()` if past
/// the end, and the newline offset if the column overshoots its line.
pub fn position_to_offset(text: &[u8], pos: Position) -> u32 {
    let (mut line, mut col) = (0u32, 0u32);
    let mut i = 0;
    while i < text.len() {
        if line == pos.line && col == pos.character {
            return i as u32;
        }
        if line > pos.line {
            return i as u32;
        }
        let (r, size) = decode(&text[i..]);
        if r == '\n' as u32 {
            if line == pos.line {
                // We didn't reach the target column on this line.
                return i as u32;
            }
            line += 1;
            col = 0;
        } else {
            col += utf16_len(r);
        }
        i += size;
    }
    text.len() as u32
}

/// Number of UTF-16 code units a scalar value occupies.
fn utf16_len(r: u32) -> u32 {
    if r >= 0x10000 { 2 } else { 1 }
}

/// Decodes one scalar from UTF-8 bytes; invalid bytes yield (U+FFFD, 1),
/// matching Go's `utf8.DecodeRune` (same contract the lexer relies on).
fn decode(bytes: &[u8]) -> (u32, usize) {
    match std::str::from_utf8(&bytes[..bytes.len().min(4)]) {
        Ok(s) => match s.chars().next() {
            Some(c) => (c as u32, c.len_utf8()),
            None => (0xFFFD, 1),
        },
        Err(e) if e.valid_up_to() > 0 => {
            let c = std::str::from_utf8(&bytes[..e.valid_up_to()])
                .unwrap()
                .chars()
                .next()
                .unwrap();
            (c as u32, c.len_utf8())
        }
        Err(_) => (0xFFFD, 1),
    }
}

/// Converts a `file://` URI to a cleaned local path (Go `uriToPath`).
pub fn uri_to_path(uri: &Url) -> PathBuf {
    match uri.to_file_path() {
        Ok(p) => PathBuf::from(pdxl_path::clean(&p.to_string_lossy())),
        // Non-file URI: fall back to the percent-decoded path portion.
        Err(()) => PathBuf::from(pdxl_path::clean(uri.path())),
    }
}

/// Converts a local path to a `file://` URI (Go `pathToURI`).
pub fn path_to_uri(path: &Path) -> Url {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    Url::from_file_path(&abs)
        .unwrap_or_else(|_| Url::parse(&format!("file://{}", abs.display())).expect("valid uri"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_ascii_and_multibyte() {
        // 'é' is 2 UTF-8 bytes / 1 UTF-16 unit; '😀' is 4 bytes / 2 units.
        let text = "ab\ncé😀d\nx".as_bytes();
        for off in [0u32, 1, 2, 3, 4, 6, 10, 11, 12] {
            let pos = offset_to_position(text, off);
            assert_eq!(
                position_to_offset(text, pos),
                off,
                "offset {off} → {pos:?} must round-trip"
            );
        }
        // The emoji occupies 2 UTF-16 units: 'd' after it is character 4.
        assert_eq!(
            offset_to_position(text, 10),
            Position {
                line: 1,
                character: 4
            }
        );
    }

    #[test]
    fn clamps_like_go() {
        let text = b"ab\ncd";
        // Past the end → len(text).
        assert_eq!(
            position_to_offset(
                text,
                Position {
                    line: 9,
                    character: 0
                }
            ),
            5
        );
        // Column overshooting its line stops at the newline.
        assert_eq!(
            position_to_offset(
                text,
                Position {
                    line: 0,
                    character: 99
                }
            ),
            2
        );
        // Offset past the end clamps.
        assert_eq!(
            offset_to_position(text, 99),
            Position {
                line: 1,
                character: 2
            }
        );
    }
}
