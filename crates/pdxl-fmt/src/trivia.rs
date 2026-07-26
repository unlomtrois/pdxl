//! The item stream: tokens plus trivia recovered from inter-token gaps.
//!
//! The lexer never emits comments (`#…` is consumed inside its whitespace
//! skipping) and carries no whitespace information — both exist only as the
//! byte gaps between consecutive token ranges. This module rescans those
//! gaps, producing a flat stream where every item knows how many newlines
//! preceded it and whether it was glued (zero-gap) to its predecessor.

use pdxl_lexer::{Lexer, TokenKind, UTF8_BOM};

/// One element of the formatting stream.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Item<'a> {
    pub(crate) kind: ItemKind,
    /// Verbatim source bytes of the token or comment (`#` included,
    /// trailing `\r` excluded). Never rewritten by the formatter.
    pub(crate) text: &'a [u8],
    /// Newlines between this item and the previous one (CRLF counts once).
    pub(crate) nl_before: u32,
    /// The source gap to the previous item was empty (`scope:x`, `a=5`).
    pub(crate) glued: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ItemKind {
    Token(TokenKind),
    Comment,
}

impl Item<'_> {
    pub(crate) fn is_comment(&self) -> bool {
        self.kind == ItemKind::Comment
    }

    pub(crate) fn token(&self) -> Option<TokenKind> {
        match self.kind {
            ItemKind::Token(k) => Some(k),
            ItemKind::Comment => None,
        }
    }
}

/// Scans `src` into the item stream. Returns `None` if the lexer produced an
/// `Invalid` token (the caller treats the file as unformattable).
pub(crate) fn scan(src: &[u8]) -> Option<Vec<Item<'_>>> {
    let mut items: Vec<Item<'_>> = Vec::new();
    let mut cursor = if src.starts_with(UTF8_BOM) {
        UTF8_BOM.len()
    } else {
        0
    };
    let mut nl_pending: u32 = 0;

    let mut lexer = Lexer::init(src);
    loop {
        let tok = lexer.next_token();
        let (tok_start, done) = match &tok {
            Some(t) if t.kind == TokenKind::Invalid => return None,
            Some(t) if t.kind == TokenKind::Eof => (src.len(), true),
            Some(t) => (t.range.start as usize, false),
            None => (src.len(), true),
        };

        // Walk the gap before this token (or the tail after the last one).
        // Gaps hold nothing but whitespace — comments are real tokens — so
        // this only counts the blank lines feeding `nl_before`.
        nl_pending += src[cursor..tok_start]
            .iter()
            .filter(|&&b| b == b'\n')
            .count() as u32;

        if done {
            break;
        }
        let tok = tok.unwrap();
        if tok.kind == TokenKind::Comment {
            // Strip every trailing CR: corpus files carry stray double-\r
            // line endings (found in vanilla scripted triggers), and any
            // retained \r would fail the re-lex verification against the
            // LF-only output.
            let mut text = tok.range.slice(src);
            while text.last() == Some(&b'\r') {
                text = &text[..text.len() - 1];
            }
            items.push(Item {
                kind: ItemKind::Comment,
                text,
                nl_before: nl_pending,
                glued: false,
            });
            nl_pending = 0;
            cursor = tok.range.end as usize;
            continue;
        }
        items.push(Item {
            kind: ItemKind::Token(tok.kind),
            text: tok.range.slice(src),
            nl_before: nl_pending,
            glued: tok.range.start as usize == cursor && !items.is_empty() && nl_pending == 0,
        });
        nl_pending = 0;
        cursor = tok.range.end as usize;
    }
    Some(items)
}
