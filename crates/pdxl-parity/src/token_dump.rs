//! Canonical token dump: `<kind>\t<start>\t<end>`, one token per line.
//!
//! Matches `tools/lexdump/main.go` byte-for-byte. Every token from
//! `Lexer::next_token` is emitted, **including invalid ones** (unlike
//! `tokenize`, which skips them), so invalid/partial-input behavior is covered
//! by the comparison.

use pdxl_lexer::Lexer;

/// Renders the canonical token dump for `src`.
pub fn dump_tokens(src: &[u8]) -> String {
    let mut lexer = Lexer::init(src);
    let mut out = String::new();
    while let Some(tok) = lexer.next_token() {
        out.push_str(tok.kind.as_str());
        out.push('\t');
        out.push_str(&tok.range.start.to_string());
        out.push('\t');
        out.push_str(&tok.range.end.to_string());
        out.push('\n');
    }
    out
}
