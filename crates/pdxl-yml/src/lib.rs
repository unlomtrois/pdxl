//! Parser for Paradox localization `.yml` files.
//!
//! Despite the extension these are not YAML — the format is a fixed shape
//! the games share:
//!
//! ```text
//! \u{FEFF}l_english:
//!  some.loc.key: "The text"
//!  other.key:0 "Versioned text with \"inner quotes\""
//!  # a comment
//! ```
//!
//! One `l_<language>:` header per file; every entry line is
//! `key[:version] "text"` where the text runs from the first `"` to the
//! **last** `"` on the line (inner quotes are legal and common). Keys carry
//! their byte offsets so editor features (go-to-definition into the yml)
//! land exactly on the key.

/// One parsed localization file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocFile {
    /// The `l_<language>` header value, e.g. `english`.
    pub language: String,
    pub entries: Vec<LocEntry>,
}

/// One `key[:version] "text"` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocEntry {
    pub key: String,
    /// Byte offset of the key's first byte (for definition targets).
    pub key_start: u32,
    /// Byte offset just past the key's last byte.
    pub key_end: u32,
    /// The text between the outermost quotes, verbatim.
    pub text: String,
    /// Byte offset of the text's first byte (just past the opening quote), so
    /// scanners over `text` can map local indices back to file offsets.
    pub text_start: u32,
}

/// A semantic span in the PDX localization dialect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub start: u32,
    pub end: u32,
    pub kind: TokenKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Header,
    Key,
    Comment,
    Text,
    LocReference,
    FunctionCandidate,
    Format,
    Icon,
}

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Whether a path names a localization file for `language`
/// (`localization/<language>/**/*_l_<language>.yml`, per Paradox layout —
/// matching on the directory is sufficient and survives typos in suffixes).
pub fn is_language_file(rel_path: &str, language: &str) -> bool {
    let Some(rest) = rel_path.strip_prefix("localization/") else {
        return false;
    };
    rest.strip_prefix(language)
        .is_some_and(|r| r.starts_with('/'))
        && rest.ends_with(".yml")
}

/// Parses a localization file. Returns `None` when no `l_<lang>:` header is
/// found before the first entry (not a localization file).
pub fn parse(src: &[u8]) -> Option<LocFile> {
    let body = src.strip_prefix(UTF8_BOM).unwrap_or(src);
    let bom_len = (src.len() - body.len()) as u32;
    let text = String::from_utf8_lossy(body);

    let mut language: Option<String> = None;
    let mut entries = Vec::new();
    let mut offset = bom_len; // byte offset of the current line start

    for line in text.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len() as u32;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if language.is_none() {
            let header = trimmed.strip_prefix("l_")?.strip_suffix(':')?;
            language = Some(header.to_string());
            continue;
        }

        // `key[:version] "text"` — key runs to the first ':'.
        let indent = (line.len() - line.trim_start().len()) as u32;
        let Some(colon) = trimmed.find(':') else {
            continue;
        };
        let key = trimmed[..colon].trim_end();
        if key.is_empty() {
            continue;
        }
        let Some(open) = trimmed.find('"') else {
            continue; // header-like or malformed line; no text
        };
        let close = trimmed.rfind('"').unwrap_or(open);
        let value = if close > open {
            &trimmed[open + 1..close]
        } else {
            ""
        };

        let key_start = line_start + indent;
        // `open` indexes into `trimmed`, whose left edge sits `indent` bytes
        // into the line; the first text byte is one past the opening quote.
        let text_start = line_start + indent + open as u32 + 1;
        entries.push(LocEntry {
            key: key.to_string(),
            key_start,
            key_end: key_start + key.len() as u32,
            text: value.to_string(),
            text_start,
        });
    }

    language.map(|language| LocFile { language, entries })
}

/// Tokenizes the fixed file shape and the inline PDX markup embedded in values.
/// Function candidates are intentionally lexical; callers can validate their
/// names against a game's dumped datafunction registry.
pub fn tokens(src: &[u8]) -> Vec<Token> {
    let mut out = Vec::new();
    let Some(file) = parse(src) else { return out };
    let body = src.strip_prefix(UTF8_BOM).unwrap_or(src);
    let bom = (src.len() - body.len()) as u32;
    if let Some(end) = body.iter().position(|&b| b == b'\n') {
        out.push(Token {
            start: bom,
            end: bom + end as u32,
            kind: TokenKind::Header,
        });
    }
    for line in byte_lines(src) {
        let trimmed = &src[line.0 as usize..line.1 as usize];
        let ws = trimmed
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(trimmed.len());
        if trimmed.get(ws) == Some(&b'#') {
            out.push(Token {
                start: line.0 + ws as u32,
                end: line.1,
                kind: TokenKind::Comment,
            });
        }
    }
    for entry in file.entries {
        out.push(Token {
            start: entry.key_start,
            end: entry.key_end,
            kind: TokenKind::Key,
        });
        scan_inline(entry.text.as_bytes(), entry.text_start, &mut out);
    }
    out.sort_by_key(|t| (t.start, t.end));
    out
}

fn byte_lines(src: &[u8]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let mut start = 0usize;
    std::iter::from_fn(move || {
        if start >= src.len() {
            return None;
        }
        let end = src[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(src.len(), |n| start + n);
        let span = (start as u32, end as u32);
        start = end.saturating_add(1);
        Some(span)
    })
}

fn scan_inline(text: &[u8], base: u32, out: &mut Vec<Token>) {
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut i = 0usize;
    let mut plain = 0usize;
    while i < text.len() {
        let (kind, lo, hi) = if text[i] == b'$' {
            let Some(n) = text[i + 1..].iter().position(|&b| b == b'$') else {
                i += 1;
                continue;
            };
            (TokenKind::LocReference, i + 1, i + 1 + n)
        } else if text[i] == b'#' {
            let mut end = i + 1;
            while end < text.len() && ident(text[end]) {
                end += 1;
            }
            if end == i + 1 && text.get(end) == Some(&b'!') {
                end += 1;
            }
            (TokenKind::Format, i, end)
        } else if text[i] == b'@' {
            let mut end = i + 1;
            while end < text.len() && (ident(text[end]) || text[end] == b'/') {
                end += 1;
            }
            if text.get(end) == Some(&b'!') {
                end += 1;
            }
            (TokenKind::Icon, i, end)
        } else if text[i] == b'[' || text[i] == b'.' {
            let start = i + 1;
            // A dot starts a datafunction member only when the member name is
            // adjacent. Sentence punctuation (`. Unlike`, `. A`, `. The`)
            // must remain ordinary localization text.
            if start >= text.len() || !ident(text[start]) {
                i += 1;
                continue;
            }
            let mut end = start;
            while end < text.len() && ident(text[end]) {
                end += 1;
            }
            if start == end {
                i += 1;
                continue;
            }
            (TokenKind::FunctionCandidate, start, end)
        } else {
            i += 1;
            continue;
        };
        if plain < lo {
            out.push(Token {
                start: base + plain as u32,
                end: base + lo as u32,
                kind: TokenKind::Text,
            });
        }
        if lo < hi {
            out.push(Token {
                start: base + lo as u32,
                end: base + hi as u32,
                kind,
            });
        }
        // Loc refs exclude delimiters; consume the closing `$` too.
        i = if kind == TokenKind::LocReference {
            hi + 1
        } else {
            hi
        };
        plain = i;
    }
    if plain < text.len() {
        out.push(Token {
            start: base + plain as u32,
            end: base + text.len() as u32,
            kind: TokenKind::Text,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header_keys_versions_and_text() {
        let src = "\u{FEFF}l_english:\n \
                   a.key: \"Plain\"\n \
                   b.key:0 \"Versioned\"\n \
                   c.key:12 \"He said \"stop\" twice\"\n";
        let f = parse(src.as_bytes()).expect("loc file");
        assert_eq!(f.language, "english");
        assert_eq!(f.entries.len(), 3);
        assert_eq!(f.entries[0].key, "a.key");
        assert_eq!(f.entries[0].text, "Plain");
        assert_eq!(f.entries[1].text, "Versioned");
        // Inner quotes: first `"` to last `"`.
        assert_eq!(f.entries[2].text, "He said \"stop\" twice");
    }

    #[test]
    fn key_offsets_point_into_the_source() {
        let src = "l_english:\n my.key: \"x\"\n";
        let f = parse(src.as_bytes()).unwrap();
        let e = &f.entries[0];
        assert_eq!(
            &src.as_bytes()[e.key_start as usize..e.key_end as usize],
            b"my.key"
        );
        // …and BOM shifts offsets accordingly.
        let bom = format!("\u{FEFF}{src}");
        let f = parse(bom.as_bytes()).unwrap();
        let e = &f.entries[0];
        assert_eq!(
            &bom.as_bytes()[e.key_start as usize..e.key_end as usize],
            b"my.key"
        );
    }

    #[test]
    fn text_start_points_past_the_opening_quote() {
        let src = "l_english:\n my.key: \"[ruler|E]\"\n";
        let f = parse(src.as_bytes()).unwrap();
        let e = &f.entries[0];
        assert_eq!(
            &src.as_bytes()[e.text_start as usize..e.text_start as usize + e.text.len()],
            e.text.as_bytes()
        );
        assert_eq!(&src.as_bytes()[e.text_start as usize..][..1], b"[");
    }

    #[test]
    fn skips_comments_blanks_and_textless_lines() {
        let src = "l_english:\n # comment\n\n broken_line_no_quote:0\n ok: \"yes\"\n";
        let f = parse(src.as_bytes()).unwrap();
        assert_eq!(f.entries.len(), 1);
        assert_eq!(f.entries[0].key, "ok");
    }

    #[test]
    fn tokenizes_inline_dialect() {
        let src = "l_english:\n key: \"#bold $other$ [GetPlayer.GetName] @gold!#!\"\n";
        let ts = tokens(src.as_bytes());
        let pieces: Vec<_> = ts
            .iter()
            .filter(|t| !matches!(t.kind, TokenKind::Text))
            .map(|t| (t.kind, &src[t.start as usize..t.end as usize]))
            .collect();
        assert!(pieces.contains(&(TokenKind::Key, "key")));
        assert!(pieces.contains(&(TokenKind::Format, "#bold")));
        assert!(pieces.contains(&(TokenKind::LocReference, "other")));
        assert!(pieces.contains(&(TokenKind::FunctionCandidate, "GetPlayer")));
        assert!(pieces.contains(&(TokenKind::FunctionCandidate, "GetName")));
        assert!(pieces.contains(&(TokenKind::Icon, "@gold!")));
        assert!(pieces.contains(&(TokenKind::Format, "#!")));

        let sentence =
            "l_english:\n key: \"[GetPlayer.GetName]. Unlike that. A thing. The end.\"\n";
        let candidates: Vec<_> = tokens(sentence.as_bytes())
            .into_iter()
            .filter(|t| t.kind == TokenKind::FunctionCandidate)
            .map(|t| &sentence[t.start as usize..t.end as usize])
            .collect();
        assert_eq!(candidates, ["GetPlayer", "GetName"]);
    }

    #[test]
    fn non_loc_files_return_none() {
        assert!(parse(b"just some text\n").is_none());
        assert!(parse(b"").is_none());
    }

    #[test]
    fn language_file_matching() {
        assert!(is_language_file(
            "localization/english/T4N_events_l_english.yml",
            "english"
        ));
        assert!(is_language_file(
            "localization/english/nested/dir/x.yml",
            "english"
        ));
        assert!(!is_language_file(
            "localization/russian/x_l_russian.yml",
            "english"
        ));
        assert!(!is_language_file(
            "localization/english_extra/x.yml",
            "english"
        ));
        assert!(!is_language_file("common/traits/x.yml", "english"));
        assert!(!is_language_file(
            "localization/english/readme.txt",
            "english"
        ));
    }
}
