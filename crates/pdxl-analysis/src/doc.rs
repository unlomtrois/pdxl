//! Smart-documentation (`#!`) references and anchors.
//!
//! A `#!` comment may name project symbols inline with `![Name]`, optionally
//! kind-qualified as `![kind:Name]`. These are *documentation* links, not
//! script: they land in [`FileFacts::calls`], the soft channel that navigates
//! and counts but never diagnoses. Docs routinely point at things that do not
//! exist yet, and a broken link is already rendered as a faded reference by
//! the editor rather than an error.
//!
//! A comment may also *declare* a name with `#! @key description…`. An anchor
//! is a symbol like any other — it lands in [`FileFacts::defs`], so it is
//! duplicate-tracked, appears in the outline, and answers find-references —
//! but unlike every other kind it has no script definition site. That is the
//! point: it names things the schema cannot model, from a subsystem spanning
//! six files to a TODO.
//!
//! [`FileFacts::calls`]: crate::FileFacts::calls
//! [`FileFacts::defs`]: crate::FileFacts::defs

use crate::KindId;
use crate::schema::Schema;

/// The kind recorded for an unqualified `![Name]`.
///
/// A bare doc ref names a symbol whose kind is unknown until the symbol table
/// exists — extraction is per-file and pre-merge, so it cannot know whether
/// `brave` is a trait or a scheme. Recording the true kind list is not an
/// option either: [`Ref::alt`](crate::Ref::alt) is `&'static [KindId]` and the
/// schema's kind list is built at runtime, so it could only be supplied by
/// leaking. Instead the ref carries this sentinel and consumers match it by
/// name across kinds — which is exactly how such a ref resolves anyway.
pub const DOC_REF: KindId = KindId::new("doc_ref");

/// Splits a `![…]` ref's inner text into an optional explicit kind (resolved
/// against the schema's aliases) and the byte offset where the referenced name
/// begins. `scheme:Name` → `(Some(scheme kind), 7)`; a bare or unknown-prefix
/// text → `(None, 0)`.
pub fn parse_doc_ref(content: &[u8], schema: &Schema) -> (Option<KindId>, usize) {
    if let Some(colon) = content.iter().position(|&b| b == b':')
        && let Ok(prefix) = std::str::from_utf8(&content[..colon])
        && let Some(kind) = schema.kind_by_alias(prefix)
    {
        return (Some(kind), colon + 1);
    }
    (None, 0)
}

/// Whether `b` may appear in an anchor key.
///
/// `:` is included so authors can namespace keys by convention
/// (`@todo:rebalance_piety`); the engine attaches no meaning to any prefix.
/// `.` mirrors the dotted names used elsewhere in PDXScript (`test.0001`).
const fn is_anchor_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b':'
}

/// The byte range of the key in a `#! @key …` anchor declaration, if this
/// comment declares one.
///
/// Declaring is a **whole-line** act: the `@` must be the first thing after
/// `#!`, and everything past the key is the anchor's description. A mid-line
/// `@` is prose — which matters here more than it might elsewhere, because
/// `@name` is PDXScript's own script-constant syntax and reads perfectly
/// naturally in a comment (`#! Scales with @rich_threshold.`). Requiring the
/// line to open with it means no sentence can declare an anchor by accident.
///
/// Same contract as [`doc_ref_spans`]: `start`/`end` must come from
/// [`SyntaxTree::doc_comments`](pdxl_ast::SyntaxTree::doc_comments).
pub fn doc_anchor_span(src: &[u8], start: u32, end: u32) -> Option<(u32, u32)> {
    let (lo, hi) = (start as usize, (end as usize).min(src.len()));
    // Skip the `#!` marker itself; the range starts at `#` (lexer contract).
    let mut k = (lo + 2).min(hi);
    while k < hi && (src[k] == b' ' || src[k] == b'\t') {
        k += 1;
    }
    if src.get(k) != Some(&b'@') {
        return None;
    }
    let ns = k + 1;
    let mut ne = ns;
    while ne < hi && is_anchor_byte(src[ne]) {
        ne += 1;
    }
    // A bare `@` with no key declares nothing.
    (ne > ns).then_some((ns as u32, ne as u32))
}

/// Byte ranges of the `Name` inside every `![Name]` of one `#!` comment.
///
/// `range` must be a doc-comment range from
/// [`SyntaxTree::doc_comments`](pdxl_ast::SyntaxTree::doc_comments) — the
/// caller has already established that these bytes are a `#!` line, so a `#`
/// inside a string can never reach here.
pub fn doc_ref_spans(src: &[u8], start: u32, end: u32, out: &mut Vec<(u32, u32)>) {
    let (lo, hi) = (start as usize, (end as usize).min(src.len()));
    let mut k = lo;
    while k + 1 < hi {
        if src[k] == b'!' && src[k + 1] == b'[' {
            let ns = k + 2;
            if let Some(rel) = src[ns..hi].iter().position(|&b| b == b']') {
                let ne = ns + rel;
                if ne > ns {
                    out.push((ns as u32, ne as u32));
                }
                k = ne + 1;
                continue;
            }
        }
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses `src` as a single doc comment, returning the anchor key text.
    fn anchor(src: &str) -> Option<&str> {
        let b = src.as_bytes();
        let (s, e) = doc_anchor_span(b, 0, b.len() as u32)?;
        Some(&src[s as usize..e as usize])
    }

    #[test]
    fn anchor_key_runs_to_the_first_byte_it_cannot_use() {
        // `:` namespaces by convention and `.` matches PDXScript's dotted
        // names; the space ends the key and opens the description.
        assert_eq!(
            anchor("#! @todo:rebalance_piety rework the piety curve"),
            Some("todo:rebalance_piety")
        );
        assert_eq!(anchor("#! @regency.system"), Some("regency.system"));
        assert_eq!(anchor("#!@tight"), Some("tight"));
        assert_eq!(anchor("#!\t@tabbed desc"), Some("tabbed"));
    }

    #[test]
    fn a_declaration_needs_a_leading_at_and_a_non_empty_key() {
        assert_eq!(anchor("#! @"), None);
        assert_eq!(anchor("#! @ spaced"), None);
        assert_eq!(anchor("#! ordinary prose"), None);
        assert_eq!(anchor("#!"), None);
    }

    #[test]
    fn prose_mentioning_a_script_constant_declares_nothing() {
        // `@name` is PDXScript's own constant syntax and reads naturally in a
        // sentence — the whole-line rule is what keeps it prose.
        assert_eq!(
            anchor("#! Scales with @rich_threshold and @gold_bonus."),
            None
        );
        assert_eq!(
            anchor("#! Created by: [@unlomtrois](https://example)"),
            None
        );
        assert_eq!(anchor("#! ask mail@example.com"), None);
    }

    #[test]
    fn a_declaration_line_still_carries_its_doc_refs() {
        let src = "#! @todo:x see ![brave]";
        assert_eq!(anchor(src), Some("todo:x"));
        let mut spans = Vec::new();
        doc_ref_spans(src.as_bytes(), 0, src.len() as u32, &mut spans);
        let names: Vec<&str> = spans
            .iter()
            .map(|&(s, e)| &src[s as usize..e as usize])
            .collect();
        assert_eq!(names, vec!["brave"]);
    }
}
