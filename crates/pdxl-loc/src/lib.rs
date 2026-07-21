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
