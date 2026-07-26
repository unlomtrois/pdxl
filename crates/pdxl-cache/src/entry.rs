//! The versioned on-disk entry format (postcard-encoded).
//!
//! Design decisions vs the Go `gob` format it replaces:
//!
//! - **Two version keys lead the entry.** `format_version` is bumped when this
//!   struct's shape changes; `syntax_version` (from [`pdxl_ast::SYNTAX_VERSION`])
//!   is bumped when lexer/parser/tree semantics change. Either mismatching on
//!   read is a clean miss — fixing Go's "content-keyed caveat" where stale
//!   entries silently outlived parser changes.
//! - **Mirror repr types**, not serde derives on the production types: `pdxl-ast`
//!   and `pdxl-parser` stay serialization-free, and this crate owns the entire
//!   persistence contract. Conversions go through the checked `from_u8`
//!   constructors, so a corrupted byte can never materialize an invalid enum.
//! - **Any decode failure is a miss**, never an error and never a partial value.
//!   Go's cache treated a corrupt gob as a miss but silently produced a
//!   nil-source tree from a corrupt gzip stream; here `decode` returns `Option`
//!   and the whole entry either round-trips or is discarded.
//! - **Source is stored raw** (no gzip): script files are small, hot reads skip
//!   a decompress step, and a whole failure class disappears. Compression can
//!   be reintroduced later behind a `format_version` bump.

use std::sync::Arc;

use pdxl_ast::{Node, NodeId, NodeKind, SyntaxTree, TextRange, TokenKind};
use pdxl_parser::{Diagnostic, Severity};
use serde::{Deserialize, Serialize};

/// Version of this on-disk layout. Bump on any change to `DiskEntry`'s shape.
pub const FORMAT_VERSION: u32 = 2; // 2: doc-comment side-channel
// 1: initial postcard layout

/// One cached parse result, self-contained: versions, freshness metadata, the
/// exact source bytes, and the flat tree (its two arrays) plus diagnostics.
#[derive(Serialize, Deserialize)]
pub(crate) struct DiskEntry {
    pub format_version: u32,
    pub syntax_version: u32,
    pub mtime_nanos: i64,
    pub sha256: [u8; 32],
    pub src: Vec<u8>,
    pub nodes: Vec<NodeRepr>,
    pub child_ids: Vec<u32>,
    pub diags: Vec<DiagRepr>,
    /// `#!` doc-comment ranges, flattened `[start, end]` pairs. Part of the
    /// entry because analysis reads doc comments off the *cached* tree — a hit
    /// that dropped them would silently lose every smart-doc reference.
    pub doc_comments: Vec<u32>,
}

/// Serialization mirror of [`pdxl_ast::Node`]; enums stored as raw bytes.
#[derive(Serialize, Deserialize)]
pub(crate) struct NodeRepr {
    pub kind: u8,
    pub start: u32,
    pub end: u32,
    pub operator: u8,
    pub child_start: u32,
    pub child_end: u32,
}

/// Serialization mirror of [`pdxl_parser::Diagnostic`].
#[derive(Serialize, Deserialize)]
pub(crate) struct DiagRepr {
    pub filename: String,
    pub offset: u32,
    pub message: String,
    pub severity: u8,
}

impl DiskEntry {
    /// Builds an entry from a parse result. `src` must be the exact bytes the
    /// tree was parsed from; `sha256` their fingerprint (computed by the caller
    /// via `fingerprint::content_hash` — one definition of truth).
    pub fn build(
        mtime_nanos: i64,
        sha256: [u8; 32],
        src: &[u8],
        tree: &SyntaxTree,
        diags: &[Diagnostic],
    ) -> DiskEntry {
        DiskEntry {
            format_version: FORMAT_VERSION,
            syntax_version: pdxl_ast::SYNTAX_VERSION,
            mtime_nanos,
            sha256,
            src: src.to_vec(),
            nodes: tree
                .nodes()
                .iter()
                .map(|n| NodeRepr {
                    kind: n.kind as u8,
                    start: n.range.start,
                    end: n.range.end,
                    operator: n.operator as u8,
                    child_start: n.child_start,
                    child_end: n.child_end,
                })
                .collect(),
            child_ids: tree.child_index().iter().map(|id| id.raw()).collect(),
            diags: diags
                .iter()
                .map(|d| DiagRepr {
                    filename: d.filename.to_string(),
                    offset: d.offset,
                    message: d.message.clone(),
                    severity: d.severity as u8,
                })
                .collect(),
            doc_comments: tree
                .doc_comments()
                .iter()
                .flat_map(|r| [r.start, r.end])
                .collect(),
        }
    }

    /// Encodes to the postcard wire format.
    pub fn encode(&self) -> Vec<u8> {
        // DiskEntry contains only plain data; postcard cannot fail on it.
        postcard::to_allocvec(self).expect("DiskEntry serialization is infallible")
    }

    /// Decodes and **fully validates** an entry, or reports a miss (`None`).
    ///
    /// Rejects: postcard decode errors, version mismatches, and any enum byte
    /// with no corresponding variant. A surviving entry is safe to reconstruct.
    pub fn decode(bytes: &[u8]) -> Option<DiskEntry> {
        let entry: DiskEntry = postcard::from_bytes(bytes).ok()?;
        if entry.format_version != FORMAT_VERSION
            || entry.syntax_version != pdxl_ast::SYNTAX_VERSION
        {
            return None;
        }
        // Validate every enum byte up front so reconstruction cannot panic.
        for n in &entry.nodes {
            NodeKind::from_u8(n.kind)?;
            TokenKind::from_u8(n.operator)?;
        }
        for d in &entry.diags {
            Severity::from_u8(d.severity)?;
        }
        // Ranges are stored as flat pairs; an odd count means a truncated
        // entry. Reject it rather than let `chunks_exact` drop the tail.
        if !entry.doc_comments.len().is_multiple_of(2) {
            return None;
        }
        Some(entry)
    }

    /// Rebuilds the tree and diagnostics. Consumes the entry so the source and
    /// message buffers move rather than copy.
    pub fn reconstruct(self) -> (Arc<SyntaxTree>, Arc<[Diagnostic]>) {
        let source: Arc<[u8]> = self.src.into();
        let nodes: Box<[Node]> = self
            .nodes
            .into_iter()
            .map(|n| Node {
                // Validated by decode(); unwrap cannot fire on a decoded entry.
                kind: NodeKind::from_u8(n.kind).expect("validated in decode"),
                range: TextRange::new(n.start, n.end),
                operator: TokenKind::from_u8(n.operator).expect("validated in decode"),
                child_start: n.child_start,
                child_end: n.child_end,
            })
            .collect();
        let child_ids: Box<[NodeId]> = self.child_ids.into_iter().map(NodeId::new).collect();
        let doc_comments: Box<[TextRange]> = self
            .doc_comments
            .chunks_exact(2)
            .map(|p| TextRange::new(p[0], p[1]))
            .collect();
        let tree = Arc::new(SyntaxTree::from_parts(
            source,
            nodes,
            child_ids,
            doc_comments,
        ));

        let diags: Arc<[Diagnostic]> = self
            .diags
            .into_iter()
            .map(|d| Diagnostic {
                filename: d.filename.into(),
                offset: d.offset,
                message: d.message,
                severity: Severity::from_u8(d.severity).expect("validated in decode"),
            })
            .collect();
        (tree, diags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Vec<u8>, Arc<SyntaxTree>, Vec<Diagnostic>) {
        let src = b"a = { b = 1".to_vec(); // unclosed: produces a diagnostic
        let parsed = pdxl_parser::parse("test", src.clone());
        assert!(!parsed.diagnostics().is_empty());
        let (tree, diags) = parsed.into_parts();
        (src, Arc::new(tree), diags)
    }

    #[test]
    fn roundtrip_preserves_everything() {
        let (src, tree, diags) = sample();
        let hash = crate::fingerprint::content_hash(&src);
        let entry = DiskEntry::build(42, hash, &src, &tree, &diags);
        let decoded = DiskEntry::decode(&entry.encode()).expect("valid entry");
        assert_eq!(decoded.mtime_nanos, 42);
        assert_eq!(decoded.sha256, hash);

        let (tree2, diags2) = decoded.reconstruct();
        assert_eq!(tree2.nodes(), tree.nodes());
        assert_eq!(tree2.child_index(), tree.child_index());
        assert_eq!(tree2.source(), &src[..]);
        assert_eq!(&diags2[..], &diags[..]);
        pdxl_ast::validate_tree(&tree2).unwrap();
    }

    #[test]
    fn roundtrip_preserves_doc_comments() {
        // Analysis reads doc comments off the *cached* tree, so a hit that
        // dropped them would silently lose every smart-doc reference.
        let src = b"#! Doc for ![brave].\n#! Second line.\na = { b = 1 }\n".to_vec();
        let (tree, diags) = pdxl_parser::parse("test", src.clone()).into_parts();
        assert_eq!(tree.doc_comments().len(), 2);

        let hash = crate::fingerprint::content_hash(&src);
        let entry = DiskEntry::build(1, hash, &src, &tree, &diags);
        let (tree2, _) = DiskEntry::decode(&entry.encode())
            .expect("valid entry")
            .reconstruct();
        assert_eq!(tree2.doc_comments(), tree.doc_comments());
        assert_eq!(
            &tree2.source()[tree2.doc_comments()[0].as_range()],
            b"#! Doc for ![brave]."
        );
    }

    #[test]
    fn odd_doc_comment_length_is_a_miss() {
        let (src, tree, diags) = sample();
        let mut entry = DiskEntry::build(
            1,
            crate::fingerprint::content_hash(&src),
            &src,
            &tree,
            &diags,
        );
        entry.doc_comments = vec![7]; // truncated pair
        assert!(DiskEntry::decode(&entry.encode()).is_none());
    }

    #[test]
    fn version_mismatch_is_a_miss() {
        let (src, tree, diags) = sample();
        let hash = crate::fingerprint::content_hash(&src);

        let mut entry = DiskEntry::build(1, hash, &src, &tree, &diags);
        entry.format_version = FORMAT_VERSION + 1;
        assert!(DiskEntry::decode(&entry.encode()).is_none(), "format bump");

        let mut entry = DiskEntry::build(1, hash, &src, &tree, &diags);
        entry.syntax_version = pdxl_ast::SYNTAX_VERSION + 1;
        assert!(DiskEntry::decode(&entry.encode()).is_none(), "syntax bump");
    }

    #[test]
    fn corrupt_bytes_are_a_miss() {
        let (src, tree, diags) = sample();
        let hash = crate::fingerprint::content_hash(&src);
        let mut bytes = DiskEntry::build(1, hash, &src, &tree, &diags).encode();

        assert!(DiskEntry::decode(b"garbage").is_none());
        assert!(DiskEntry::decode(&[]).is_none());
        // Truncation: postcard's length prefixes make the tail run out.
        let cut = bytes.len() / 2;
        assert!(DiskEntry::decode(&bytes[..cut]).is_none());
        // An invalid enum byte deep inside must also be rejected, not panic.
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let _ = DiskEntry::decode(&bytes); // must not panic; miss or (unlucky) valid
    }
}
