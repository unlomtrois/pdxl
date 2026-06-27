//! A direct port of Go's `unicode/utf8.DecodeRune`.
//!
//! The lexer's byte offsets depend on the exact rune *size* the decoder reports,
//! including on malformed input (where Go yields `RuneError` with size 1). To
//! guarantee byte-for-byte offset parity with the Go lexer, this reproduces the
//! standard library's `first[256]` accept table and `acceptRanges` rather than
//! relying on Rust's `str::from_utf8`, whose error handling differs.

/// The Unicode replacement character `U+FFFD`, returned for invalid encodings.
pub const RUNE_ERROR: u32 = 0xFFFD;

const LOCB: u8 = 0b1000_0000; // 0x80
const HICB: u8 = 0b1011_1111; // 0xBF

// Values in the `FIRST` table. `XX` = invalid (size 1), `AS` = ASCII (size 1),
// otherwise the low 3 bits are the encoded length and the high nibble selects an
// accept range.
const XX: u8 = 0xF1;
const AS: u8 = 0xF0;
const S1: u8 = 0x02;
const S2: u8 = 0x13;
const S3: u8 = 0x03;
const S4: u8 = 0x23;
const S5: u8 = 0x34;
const S6: u8 = 0x04;
const S7: u8 = 0x44;

/// First-byte classification table, identical to Go's `utf8.first`.
#[rustfmt::skip]
static FIRST: [u8; 256] = [
    //   0     1     2     3     4     5     6     7     8     9     A     B     C     D     E     F
    AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x00-0x0F
    AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x10-0x1F
    AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x20-0x2F
    AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x30-0x3F
    AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x40-0x4F
    AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x50-0x5F
    AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x60-0x6F
    AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, AS, // 0x70-0x7F
    //   0     1     2     3     4     5     6     7     8     9     A     B     C     D     E     F
    XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0x80-0x8F
    XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0x90-0x9F
    XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0xA0-0xAF
    XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0xB0-0xBF
    XX, XX, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, // 0xC0-0xCF
    S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, S1, // 0xD0-0xDF
    S2, S3, S3, S3, S3, S3, S3, S3, S3, S3, S3, S3, S3, S4, S3, S3, // 0xE0-0xEF
    S5, S6, S6, S6, S7, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0xF0-0xFF
];

/// `(lo, hi)` continuation-byte ranges selected by the high nibble of a `FIRST`
/// table entry; identical to Go's `utf8.acceptRanges`.
static ACCEPT_RANGES: [(u8, u8); 16] = [
    (LOCB, HICB),
    (0xA0, HICB),
    (LOCB, 0x9F),
    (0x90, HICB),
    (LOCB, 0x8F),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
];

const MASKX: u8 = 0b0011_1111;
const MASK2: u8 = 0b0001_1111;
const MASK3: u8 = 0b0000_1111;
const MASK4: u8 = 0b0000_0111;

/// Decodes the first rune in `p`, returning the rune and its byte size.
///
/// Matches `utf8.DecodeRune`:
/// - empty input → `(RUNE_ERROR, 0)`;
/// - invalid encoding → `(RUNE_ERROR, 1)`;
/// - otherwise the decoded rune and its 1–4 byte length.
#[inline]
pub fn decode_rune(p: &[u8]) -> (u32, usize) {
    let n = p.len();
    if n < 1 {
        return (RUNE_ERROR, 0);
    }
    let p0 = p[0];
    let x = FIRST[p0 as usize];
    if x >= AS {
        // x == AS (ASCII) → return the byte; x == XX (invalid) → RUNE_ERROR.
        // (Go uses a branchless mask; an explicit branch is clearer here.)
        if x == AS {
            return (p0 as u32, 1);
        }
        return (RUNE_ERROR, 1);
    }
    let sz = (x & 7) as usize;
    let accept = ACCEPT_RANGES[(x >> 4) as usize];
    if n < sz {
        return (RUNE_ERROR, 1);
    }
    let b1 = p[1];
    if b1 < accept.0 || accept.1 < b1 {
        return (RUNE_ERROR, 1);
    }
    if sz <= 2 {
        return ((((p0 & MASK2) as u32) << 6) | ((b1 & MASKX) as u32), 2);
    }
    let b2 = p[2];
    if !(LOCB..=HICB).contains(&b2) {
        return (RUNE_ERROR, 1);
    }
    if sz <= 3 {
        return (
            (((p0 & MASK3) as u32) << 12) | (((b1 & MASKX) as u32) << 6) | ((b2 & MASKX) as u32),
            3,
        );
    }
    let b3 = p[3];
    if !(LOCB..=HICB).contains(&b3) {
        return (RUNE_ERROR, 1);
    }
    (
        (((p0 & MASK4) as u32) << 18)
            | (((b1 & MASKX) as u32) << 12)
            | (((b2 & MASKX) as u32) << 6)
            | ((b3 & MASKX) as u32),
        4,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii() {
        assert_eq!(decode_rune(b"k"), ('k' as u32, 1));
        assert_eq!(decode_rune(b"="), ('=' as u32, 1));
    }

    #[test]
    fn empty() {
        assert_eq!(decode_rune(b""), (RUNE_ERROR, 0));
    }

    #[test]
    fn two_byte() {
        // 'é' = U+00E9 = 0xC3 0xA9
        assert_eq!(decode_rune("é".as_bytes()), (0x00E9, 2));
    }

    #[test]
    fn three_byte() {
        // '€' = U+20AC = 0xE2 0x82 0xAC
        assert_eq!(decode_rune("€".as_bytes()), (0x20AC, 3));
    }

    #[test]
    fn four_byte() {
        // '😀' = U+1F600 = 0xF0 0x9F 0x98 0x80
        assert_eq!(decode_rune("😀".as_bytes()), (0x1F600, 4));
    }

    #[test]
    fn invalid_continuation_is_size_one() {
        // 0x80 is a lone continuation byte → RuneError, size 1.
        assert_eq!(decode_rune(&[0x80]), (RUNE_ERROR, 1));
        // 0xC3 with a bad continuation → RuneError, size 1.
        assert_eq!(decode_rune(&[0xC3, 0x28]), (RUNE_ERROR, 1));
        // Truncated 2-byte lead → RuneError, size 1.
        assert_eq!(decode_rune(&[0xC3]), (RUNE_ERROR, 1));
        // 0xFF is never a valid lead byte.
        assert_eq!(decode_rune(&[0xFF]), (RUNE_ERROR, 1));
    }
}
