//! Byte-offset tokenizer for Paradox script files.
//!
//! This is a faithful port of the Go `internal/lexer` package. It preserves the
//! exact token kinds and byte ranges of the reference implementation, which is
//! the oracle until parity is established.
//!
//! Design invariants carried over from Go:
//!
//! - **No string copies.** A [`Token`] stores only a [`TokenKind`] and a
//!   [`TextRange`]; the literal text is `range.slice(source)`.
//! - **Byte offsets are zero-based and half-open** (`[start, end)`).
//! - **Runes are decoded with Go's `utf8.DecodeRune` semantics.** Invalid UTF-8
//!   yields `U+FFFD` with size 1 and is treated as an identifier character — the
//!   lexer never rejects bytes. See [`decode_rune`]. This makes offsets match the
//!   Go lexer byte-for-byte even on malformed input.
//! - **The UTF-8 BOM is skipped** by [`Lexer::init`].

mod rune;

pub use pdxl_src::TextRange;
use rune::decode_rune;

/// The UTF-8 BOM byte sequence skipped at the start of a source buffer.
pub const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// The kind of a lexical token.
///
/// Variants and their [`TokenKind::as_str`] names match the Go `Tag` constants
/// and `Tag.String()` exactly, so token dumps are directly comparable. Like
/// Go's `Tag` (a `uint8`), the kind is byte-sized (`repr(u8)`, discriminants
/// in declaration order) so it can round-trip through persistent storage via
/// `as u8` / [`TokenKind::from_u8`].
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Identifier
    Identifier, // id

    // Literals
    LiteralNumber,  // 123, 0.1, 1.2.3
    LiteralString,  // "string"
    LiteralBoolean, // yes, no
    LiteralDate,    // 1099.1.1

    // Delimiters
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]

    // Arithmetic operators
    Plus,     // `+`
    Minus,    // `-`
    Multiply, // `*`
    Divide,   // `/`

    // Comparison operators
    GreaterThan,  // `>`
    GreaterEqual, // `>=`
    LessThan,     // `<`
    LessEqual,    // `<=`

    // Assignment
    Equal, // `=`

    // Equality
    EqualEqual,    // `==`
    NotEqual,      // `!=`
    QuestionEqual, // `?=`

    // Scope resolution operators / PDXScript specials
    Dot,         // `.`
    Colon,       // `:`
    At,          // `@`
    Pipe,        // `|`
    Dollar,      // `$`
    MacroParam,  // `$IDENT$`
    ScriptValue, // `@name`
    ScriptMath,  // `@[ ... ]`

    Percent, // `%`

    Comment,    // `#...`
    DocComment, // `#!...` — smart documentation
    Invalid,    // invalid token
    Eof,        // end of file
}

impl TokenKind {
    /// Every kind, in discriminant order (`ALL[i] as u8 == i`).
    pub const ALL: [TokenKind; 34] = {
        use TokenKind::*;
        [
            Identifier,
            LiteralNumber,
            LiteralString,
            LiteralBoolean,
            LiteralDate,
            LBrace,
            RBrace,
            LBracket,
            RBracket,
            Plus,
            Minus,
            Multiply,
            Divide,
            GreaterThan,
            GreaterEqual,
            LessThan,
            LessEqual,
            Equal,
            EqualEqual,
            NotEqual,
            QuestionEqual,
            Dot,
            Colon,
            At,
            Pipe,
            Dollar,
            MacroParam,
            ScriptValue,
            ScriptMath,
            Percent,
            Comment,
            DocComment,
            Invalid,
            Eof,
        ]
    };

    /// The kind whose discriminant is `v`, or `None` for an out-of-range byte.
    /// Inverse of `kind as u8`; used by persistent stores (the syntax cache).
    #[inline]
    pub fn from_u8(v: u8) -> Option<TokenKind> {
        Self::ALL.get(v as usize).copied()
    }

    /// The display name of this kind, identical to Go's `Tag.String()`.
    pub const fn as_str(self) -> &'static str {
        use TokenKind::*;
        match self {
            Identifier => "identifier",
            LiteralNumber => "literal_number",
            LiteralString => "literal_string",
            LiteralBoolean => "literal_boolean",
            LiteralDate => "literal_date",
            LBrace => "l_brace",
            RBrace => "r_brace",
            LBracket => "l_bracket",
            RBracket => "r_bracket",
            Plus => "plus",
            Minus => "minus",
            Multiply => "multiply",
            Divide => "divide",
            GreaterThan => "greater_than",
            GreaterEqual => "greater_equal",
            LessThan => "less_than",
            LessEqual => "less_equal",
            Equal => "equal",
            EqualEqual => "equal_equal",
            NotEqual => "not_equal",
            QuestionEqual => "question_equal",
            Dot => "dot",
            Colon => "colon",
            At => "at",
            Pipe => "pipe",
            Dollar => "dollar",
            MacroParam => "macro_param",
            ScriptValue => "script_value",
            ScriptMath => "script_math",
            Percent => "percent",
            Comment => "comment",
            DocComment => "doc_comment",
            Invalid => "invalid",
            Eof => "eof",
        }
    }

    /// Whether this kind is a line comment — plain [`Comment`] or a
    /// [`DocComment`]. Consumers that care about comments as *trivia* (the
    /// formatter) want both; consumers that want smart-documentation
    /// references match `DocComment` alone.
    ///
    /// [`Comment`]: TokenKind::Comment
    /// [`DocComment`]: TokenKind::DocComment
    #[inline]
    pub const fn is_comment(self) -> bool {
        matches!(self, TokenKind::Comment | TokenKind::DocComment)
    }
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A lexical token: a kind plus a byte range into the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: TextRange,
}

impl Token {
    /// Returns the literal bytes of this token from `source`.
    #[inline]
    pub fn value(self, source: &[u8]) -> &[u8] {
        self.range.slice(source)
    }

    /// Whether this is an [`TokenKind::Invalid`] token.
    #[inline]
    pub fn is_invalid(self) -> bool {
        self.kind == TokenKind::Invalid
    }
}

/// A lexical analyzer over a borrowed source buffer.
///
/// `pos` is a byte offset, not a rune index, matching the Go `Lexer`.
pub struct Lexer<'src> {
    source: &'src [u8],
    pos: usize,
}

impl<'src> Lexer<'src> {
    /// Creates a lexer for `source`, skipping a leading UTF-8 BOM if present.
    pub fn init(source: &'src [u8]) -> Self {
        let pos = if source.starts_with(UTF8_BOM) {
            UTF8_BOM.len()
        } else {
            0
        };
        Lexer { source, pos }
    }

    /// Returns the next token, or `None` at end of input.
    ///
    /// Unlike [`tokenize`], this yields *every* token including
    /// [`TokenKind::Comment`] and [`TokenKind::Invalid`] ones (it never
    /// produces `Eof`). Comments are real tokens here so that comment-aware
    /// consumers (smart-doc references, the formatter) can read them from the
    /// lexer instead of reconstructing them from the gaps between tokens.
    pub fn next_token(&mut self) -> Option<Token> {
        self.skip_whitespace();
        if self.is_at_end() {
            return None;
        }

        let start_pos = self.pos;
        // Checked before `advance` so the comment body is consumed wholesale;
        // a `#` inside a string is unreachable here (`lex_string` swallows it).
        if self.peek() == CH_HASH {
            // `#!` opens a smart-documentation comment, plain `#` ordinary
            // prose. The split happens here, in the one place that sees every
            // comment exactly once: doc comments are ~0.004% of the corpus
            // (65 of ~1.7M in CK3 vanilla + T4N), so any consumer wanting
            // only those can discard the rest on a byte compare rather than
            // carrying every comment downstream.
            let kind = if self.source.get(start_pos + 1) == Some(&b'!') {
                TokenKind::DocComment
            } else {
                TokenKind::Comment
            };
            self.skip_to_line_end();
            return Some(Token {
                kind,
                range: TextRange::from_usize(start_pos, self.pos),
            });
        }
        let (c, size) = self.advance();

        let kind = if is_ascii_digit_rune(c) {
            self.lex_after_digit(size)
        } else if is_identifier_start(c) {
            let k = self.lex_identifier();
            match &self.source[start_pos..self.pos] {
                b"yes" | b"no" => TokenKind::LiteralBoolean,
                _ => k,
            }
        } else {
            self.lex_symbol(c, start_pos)
        };

        Some(Token {
            kind,
            range: TextRange::from_usize(start_pos, self.pos),
        })
    }

    /// Handles a token that began with an ASCII digit.
    ///
    /// `size` is the byte size of the digit already consumed by `advance`.
    fn lex_after_digit(&mut self, size: usize) -> TokenKind {
        if !self.is_at_end() && (is_alpha(trunc(self.peek())) || self.peek() == '_' as u32) {
            // digit followed immediately by letter/underscore — whole thing is an
            // identifier (e.g. "8_something").
            self.pos -= size;
            return self.lex_identifier();
        }
        let mut kind = self.lex_number();
        // trailing identifier chars after a number (e.g. "1abc") — identifier.
        if is_identifier_char(self.peek()) {
            while is_identifier_char(self.peek()) {
                self.advance();
            }
            kind = TokenKind::Identifier;
        }
        kind
    }

    /// Handles a token that began with a non-identifier, non-digit byte.
    fn lex_symbol(&mut self, c: u32, start_pos: usize) -> TokenKind {
        match c {
            CH_QUOTE => self.lex_string(),

            // scope operators
            CH_DOT => self.lex_dot(start_pos),
            CH_COLON => TokenKind::Colon,
            CH_AT => self.lex_at(),
            CH_PIPE => TokenKind::Pipe,
            CH_DOLLAR => self.lex_dollar(),

            // special
            CH_PERCENT => TokenKind::Percent,

            // comparison / assignment / equality operators
            CH_EQUAL => {
                if self.matches(b'=') {
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Equal
                }
            }
            CH_GREATER => {
                if self.matches(b'=') {
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::GreaterThan
                }
            }
            CH_LESS => {
                if self.matches(b'=') {
                    TokenKind::LessEqual
                } else {
                    TokenKind::LessThan
                }
            }
            CH_BANG => {
                if self.matches(b'=') {
                    TokenKind::NotEqual
                } else {
                    TokenKind::Invalid
                }
            }
            CH_QUESTION => {
                if self.matches(b'=') {
                    TokenKind::QuestionEqual
                } else {
                    TokenKind::Invalid
                }
            }

            // arithmetic operators
            CH_PLUS => TokenKind::Plus,
            CH_MINUS => self.lex_minus(),
            CH_STAR => TokenKind::Multiply,
            CH_SLASH => TokenKind::Divide,

            CH_LBRACE => TokenKind::LBrace,
            CH_RBRACE => TokenKind::RBrace,
            CH_LBRACKET => TokenKind::LBracket,
            CH_RBRACKET => TokenKind::RBracket,

            _ => TokenKind::Invalid,
        }
    }

    /// `.` dispatch: a leading-dot float (`.7`) at a value start, else a `dot`.
    fn lex_dot(&mut self, start_pos: usize) -> TokenKind {
        // A leading-dot float, but only at value start. A '.' following an
        // identifier char (incl. digits) stays a separator, so dates ("1099.1.1")
        // and event IDs / scope chains ("accolade.0001") are not mis-lexed.
        let prev_is_ident = start_pos > 0 && is_identifier_char(self.source[start_pos - 1] as u32);
        if is_digit(trunc(self.peek())) && (start_pos == 0 || !prev_is_ident) {
            while is_digit(trunc(self.peek())) {
                self.advance();
            }
            TokenKind::LiteralNumber
        } else {
            TokenKind::Dot
        }
    }

    /// `@` dispatch: `@name` (script value), `@[ … ]` (inline math), or bare `@`.
    fn lex_at(&mut self) -> TokenKind {
        if is_identifier_char(self.peek()) {
            while is_identifier_char(self.peek()) {
                self.advance();
            }
            TokenKind::ScriptValue
        } else if self.peek() == '[' as u32 {
            self.advance(); // consume '['
            while !self.is_at_end() && self.peek() != ']' as u32 {
                self.advance();
            }
            if !self.is_at_end() {
                self.advance(); // consume ']'
            }
            TokenKind::ScriptMath
        } else {
            TokenKind::At
        }
    }

    /// `$` dispatch: `$IDENT$` macro param, or a bare `$` (with backtrack).
    fn lex_dollar(&mut self) -> TokenKind {
        let name_start = self.pos;
        while !self.is_at_end() && is_identifier_char(self.peek()) {
            self.advance();
        }
        if self.pos > name_start && !self.is_at_end() && self.peek() == '$' as u32 {
            self.advance(); // consume closing $
            TokenKind::MacroParam
        } else {
            self.pos = name_start; // backtrack — bare dollar
            TokenKind::Dollar
        }
    }

    /// `-` dispatch: a leading `-` on a date is part of the BC date literal;
    /// otherwise it stays a minus.
    fn lex_minus(&mut self) -> TokenKind {
        if is_digit(trunc(self.peek())) {
            let save = self.pos;
            if self.lex_number() == TokenKind::LiteralDate {
                TokenKind::LiteralDate
            } else {
                self.pos = save; // not a date — re-lex the number after the minus
                TokenKind::Minus
            }
        } else {
            TokenKind::Minus
        }
    }

    /// Scans an identifier, including hyphens flanked by identifier chars.
    fn lex_identifier(&mut self) -> TokenKind {
        while !self.is_at_end() {
            if is_identifier_char(self.peek()) {
                self.advance();
                continue;
            }
            // A hyphen is part of the identifier only when flanked by identifier
            // chars (e.g. title keys like c_anti-atlas).
            if self.peek() == '-' as u32 && is_identifier_char(self.peek_next()) {
                self.advance(); // consume '-'
                continue;
            }
            break;
        }
        TokenKind::Identifier
    }

    /// Scans a number literal; returns [`TokenKind::LiteralDate`] for `Y.M.D`.
    fn lex_number(&mut self) -> TokenKind {
        while is_digit(trunc(self.peek())) {
            self.advance();
        }

        // Consume one or more ".digits" groups. One group is a decimal (1099.1);
        // two or more make a date literal (1099.1.1).
        let mut dots = 0;
        while self.peek() == '.' as u32 && is_digit(trunc(self.peek_next())) {
            self.advance(); // consume '.'
            while is_digit(trunc(self.peek())) {
                self.advance();
            }
            dots += 1;
        }
        if dots >= 2 {
            return TokenKind::LiteralDate;
        }

        if dots == 0 && self.peek() == '.' as u32 && !is_identifier_start(self.peek_next()) {
            // trailing-dot float, e.g. "1." — but keep "N.identifier" as a scope
            // chain (number, dot, identifier).
            self.advance(); // consume '.'
        }

        TokenKind::LiteralNumber
    }

    /// Scans a quoted string; unterminated strings are [`TokenKind::Invalid`].
    fn lex_string(&mut self) -> TokenKind {
        while !self.is_at_end() && self.peek() != '"' as u32 {
            self.advance();
        }
        if self.is_at_end() {
            return TokenKind::Invalid; // unterminated string
        }
        self.advance(); // consume closing quote
        TokenKind::LiteralString
    }

    /// Skips whitespace. `#` line comments are *not* skipped — [`next_token`]
    /// emits them as [`TokenKind::Comment`].
    ///
    /// [`next_token`]: Lexer::next_token
    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            match self.peek() {
                c if c == ' ' as u32
                    || c == '\t' as u32
                    || c == '\r' as u32
                    || c == '\n' as u32 =>
                {
                    self.advance();
                }
                _ => return,
            }
        }
    }

    /// Consumes up to (not including) the next newline — the body of a `#`
    /// line comment. The terminating `\n` stays for `skip_whitespace`, so a
    /// comment token never spans the line break.
    fn skip_to_line_end(&mut self) {
        while !self.is_at_end() && self.peek() != '\n' as u32 {
            self.advance();
        }
    }

    #[inline]
    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    /// Consumes and returns the current rune and its byte size.
    ///
    /// Invalid UTF-8 produces `U+FFFD` with size 1 (see [`decode_rune`]); since
    /// `RUNE_ERROR > 127` it passes [`is_identifier_char`] — no validation.
    #[inline]
    fn advance(&mut self) -> (u32, usize) {
        if self.is_at_end() {
            return (0, 0);
        }
        let (r, size) = decode_rune(&self.source[self.pos..]);
        self.pos += size;
        (r, size)
    }

    /// Returns the current rune without consuming it (`0` at end).
    #[inline]
    fn peek(&self) -> u32 {
        if self.is_at_end() {
            return 0;
        }
        decode_rune(&self.source[self.pos..]).0
    }

    /// Returns the rune after the current one without consuming either (`0` at end).
    #[inline]
    fn peek_next(&self) -> u32 {
        if self.is_at_end() {
            return 0;
        }
        let (_, size) = decode_rune(&self.source[self.pos..]);
        let next = self.pos + size;
        if next >= self.source.len() {
            return 0;
        }
        decode_rune(&self.source[next..]).0
    }

    /// Consumes the current byte if it equals `expected` (ASCII).
    #[inline]
    fn matches(&mut self, expected: u8) -> bool {
        if self.is_at_end() {
            return false;
        }
        if self.source[self.pos] != expected {
            return false;
        }
        self.pos += 1;
        true
    }
}

/// Returns all valid tokens from `src`, skipping comments (plain and doc),
/// `Invalid` and `Eof`.
///
/// Mirrors the Go `lexer.Tokenize` helper — this is what the parser consumes.
/// The comment-free contract is load-bearing: the parser has no comment
/// handling, so comments must not reach it. Consumers that *want* comments
/// drive [`Lexer::next_token`] directly.
pub fn tokenize(src: &[u8]) -> Vec<Token> {
    tokenize_inner(src, None)
}

/// Like [`tokenize`], but also returns the range of every `#!` doc comment,
/// in source order — the parser's single-pass path.
///
/// Plain comments are still dropped on the spot. Only doc comments are
/// retained, which is why this costs nothing: they are ~0.004% of comments in
/// practice, so the returned vector is a handful of entries even for large
/// files.
pub fn tokenize_with_docs(src: &[u8]) -> (Vec<Token>, Vec<TextRange>) {
    let mut docs = Vec::new();
    let tokens = tokenize_inner(src, Some(&mut docs));
    (tokens, docs)
}

fn tokenize_inner(src: &[u8], mut docs: Option<&mut Vec<TextRange>>) -> Vec<Token> {
    let mut lexer = Lexer::init(src);
    let mut out = Vec::with_capacity(src.len() / 8);
    while let Some(tok) = lexer.next_token() {
        if tok.kind == TokenKind::DocComment
            && let Some(docs) = docs.as_deref_mut()
        {
            docs.push(tok.range);
        }
        if tok.kind.is_comment() || matches!(tok.kind, TokenKind::Invalid | TokenKind::Eof) {
            continue;
        }
        out.push(tok);
    }
    out
}

// --- character classification (ports of the Go helpers) ---------------------

// ASCII byte constants for the symbol dispatch. Using named constants keeps the
// match arms readable while matching against the `u32` rune values.
const CH_QUOTE: u32 = b'"' as u32;
const CH_HASH: u32 = b'#' as u32;
const CH_DOT: u32 = b'.' as u32;
const CH_COLON: u32 = b':' as u32;
const CH_AT: u32 = b'@' as u32;
const CH_PIPE: u32 = b'|' as u32;
const CH_DOLLAR: u32 = b'$' as u32;
const CH_PERCENT: u32 = b'%' as u32;
const CH_EQUAL: u32 = b'=' as u32;
const CH_GREATER: u32 = b'>' as u32;
const CH_LESS: u32 = b'<' as u32;
const CH_BANG: u32 = b'!' as u32;
const CH_QUESTION: u32 = b'?' as u32;
const CH_PLUS: u32 = b'+' as u32;
const CH_MINUS: u32 = b'-' as u32;
const CH_STAR: u32 = b'*' as u32;
const CH_SLASH: u32 = b'/' as u32;
const CH_LBRACE: u32 = b'{' as u32;
const CH_RBRACE: u32 = b'}' as u32;
const CH_LBRACKET: u32 = b'[' as u32;
const CH_RBRACKET: u32 = b']' as u32;

/// Truncates a rune to a single byte, matching Go's `byte(rune)` conversion.
#[inline]
const fn trunc(r: u32) -> u8 {
    r as u8
}

/// Whether `r` is an ASCII digit `0`–`9` (as a full rune, no truncation).
#[inline]
const fn is_ascii_digit_rune(r: u32) -> bool {
    r >= '0' as u32 && r <= '9' as u32
}

/// Whether a rune can begin an identifier.
///
/// `/` starts unquoted path atoms; any non-ASCII codepoint (`> 127`) is allowed.
#[inline]
const fn is_identifier_start(r: u32) -> bool {
    r > 127
        || (r >= 'a' as u32 && r <= 'z' as u32)
        || (r >= 'A' as u32 && r <= 'Z' as u32)
        || r == '_' as u32
        || r == '/' as u32
}

/// Whether a rune can be part of an identifier.
#[inline]
const fn is_identifier_char(r: u32) -> bool {
    if r > 127 {
        return true; // any non-ASCII Unicode codepoint is valid in an identifier
    }
    if is_alpha_numeric(trunc(r)) {
        return true;
    }
    matches!(
        r,
        // '_', '&', '\'', '%', '/'
        0x5F | 0x26 | 0x27 | 0x25 | 0x2F
    )
}

#[inline]
const fn is_alpha(c: u8) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_uppercase()
}

#[inline]
const fn is_numeric(c: u8) -> bool {
    c.is_ascii_digit()
}

#[inline]
const fn is_alpha_numeric(c: u8) -> bool {
    is_alpha(c) || is_numeric(c)
}

#[inline]
const fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

#[cfg(test)]
mod tests;
