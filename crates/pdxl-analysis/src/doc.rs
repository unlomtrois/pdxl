//! Smart-documentation (`#!`) references.
//!
//! A `#!` comment may name project symbols inline with `![Name]`, optionally
//! kind-qualified as `![kind:Name]`. These are *documentation* links, not
//! script: they land in [`FileFacts::calls`], the soft channel that navigates
//! and counts but never diagnoses. Docs routinely point at things that do not
//! exist yet, and a broken link is already rendered as a faded reference by
//! the editor rather than an error.
//!
//! [`FileFacts::calls`]: crate::FileFacts::calls

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
