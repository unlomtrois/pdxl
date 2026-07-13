//! Source model primitives shared across the pdxl Rust workspace.
//!
//! The Go implementation uses zero-based, half-open byte ranges (`[start, end)`)
//! everywhere: lexer tokens, parser nodes, symbols, references, and diagnostics.
//! This crate provides the compact offset type that preserves that model.
//!
//! Offsets are stored as `u32`. Paradox script files are far below 4 GiB, so a
//! 32-bit offset is sufficient and keeps token/node structs small. UTF-16
//! conversion is *not* performed here; it belongs only at the LSP protocol
//! boundary.

use std::ops::Range;

/// A half-open byte range `[start, end)` into a source buffer.
///
/// `start` and `end` are zero-based byte offsets, matching the Go lexer's
/// `Token.Start` / `Token.End`. The slice a range refers to is
/// `source[start as usize..end as usize]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextRange {
    pub start: u32,
    pub end: u32,
}

impl TextRange {
    /// Creates a range from raw byte offsets.
    #[inline]
    pub const fn new(start: u32, end: u32) -> Self {
        TextRange { start, end }
    }

    /// Creates a range from `usize` offsets (e.g. from indexing a byte slice).
    #[inline]
    pub fn from_usize(start: usize, end: usize) -> Self {
        TextRange {
            start: start as u32,
            end: end as u32,
        }
    }

    /// Length of the range in bytes.
    #[inline]
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    /// Whether the range is empty (`start == end`).
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// The range as a `usize` `Range`, suitable for slicing a byte buffer.
    #[inline]
    pub fn as_range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }

    /// Returns the source bytes this range refers to.
    ///
    /// Panics if the range is out of bounds for `source`, mirroring Go's
    /// `source[Start:End]` slice semantics.
    #[inline]
    pub fn slice(self, source: &[u8]) -> &[u8] {
        &source[self.as_range()]
    }
}

/// Converts a byte offset into a 1-indexed `(line, column)` for display.
///
/// Matches Go's `Token.getPosition` exactly: lines split on `\n`, and the
/// column counts **bytes** since the last newline (not runes) — a display-only
/// derivation used for `file:line:col` strings. Offsets remain the internal
/// currency everywhere else.
pub fn line_col(source: &[u8], offset: u32) -> (u32, u32) {
    let mut line = 1;
    let mut col = 1;
    for &b in source.iter().take(offset as usize) {
        if b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_matches_go_semantics() {
        let src = b"ab\ncd\n";
        assert_eq!(line_col(src, 0), (1, 1));
        assert_eq!(line_col(src, 1), (1, 2));
        assert_eq!(line_col(src, 3), (2, 1)); // first byte after newline
        assert_eq!(line_col(src, 5), (2, 3));
        // Column counts bytes: 'é' is 2 bytes, so the byte after it is col 4.
        assert_eq!(line_col("aé b".as_bytes(), 3), (1, 4));
    }

    #[test]
    fn len_and_empty() {
        let r = TextRange::new(3, 6);
        assert_eq!(r.len(), 3);
        assert!(!r.is_empty());
        assert!(TextRange::new(5, 5).is_empty());
    }

    #[test]
    fn slices_source() {
        let src = b"key = value";
        let r = TextRange::from_usize(0, 3);
        assert_eq!(r.slice(src), b"key");
        assert_eq!(r.as_range(), 0..3);
    }
}
