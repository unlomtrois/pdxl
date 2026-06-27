//! Recovery and termination tests over malformed and fuzz-like byte sequences.
//!
//! The parser must always terminate, always return a tree, and always satisfy
//! the structural invariants — regardless of how broken the input is. These run
//! without the Go oracle (they assert contract properties, not parity).

use pdxl_syntax::{NodeKind, parse, validate_tree};

/// A deterministic pseudo-random byte generator (no external deps, no
/// `Math.random`-style nondeterminism), seeded per case for reproducibility.
fn fuzz_bytes(seed: u64, len: usize) -> Vec<u8> {
    // A small alphabet of structurally interesting bytes plus some raw/invalid
    // ones, so we exercise operators, delimiters, chains, and bad UTF-8.
    const ALPHABET: &[u8] = b"{}[]<>=?!|.:@$-+abc 123\n\t\"%/&'\xff\x80\xc3";
    let mut state = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        // xorshift64*
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let r = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
        out.push(ALPHABET[(r as usize) % ALPHABET.len()]);
    }
    out
}

#[test]
fn always_returns_valid_tree_on_fuzz() {
    for seed in 0..256u64 {
        let len = 1 + (seed as usize % 200);
        let input = fuzz_bytes(seed, len);
        let parsed = parse("fuzz", input.clone());
        validate_tree(parsed.tree()).unwrap_or_else(|e| {
            panic!("seed {seed}: invalid tree for {input:?}: {e:?}");
        });
        // Root is always the File node, even on garbage.
        assert_eq!(
            parsed.tree().node(parsed.tree().root()).kind,
            NodeKind::File
        );
    }
}

#[test]
fn deeply_nested_braces_terminate() {
    // Many unclosed braces must not fail to terminate; ensure it returns and
    // validates. Depth is kept moderate because the recursive-descent parser (a
    // faithful port of Go's) consumes one native stack frame per block level, and
    // test threads have a smaller default stack than Go's growable stacks. Very
    // deep nesting is recorded as a recursion-depth risk in the milestone report.
    let depth = 500;
    let mut src = Vec::new();
    for _ in 0..depth {
        src.extend_from_slice(b"a = { ");
    }
    let parsed = parse("deep", src);
    validate_tree(parsed.tree()).unwrap();
    // One unclosed-block diagnostic per opened block.
    assert_eq!(parsed.diagnostics().len(), depth);
}

#[test]
fn empty_and_whitespace_only() {
    for src in ["", "   ", "\n\n\t", "# only a comment\n", "\u{feff}"] {
        let parsed = parse("ws", src.as_bytes().to_vec());
        assert!(parsed.diagnostics().is_empty(), "{src:?}");
        validate_tree(parsed.tree()).unwrap();
        assert_eq!(parsed.tree().len(), 1, "only the file root for {src:?}");
    }
}

#[test]
fn stray_delimiters_are_skipped() {
    let parsed = parse("test", &b"} } ] ] } key = value ]"[..]);
    assert!(parsed.diagnostics().is_empty());
    let tree = parsed.tree();
    let items: Vec<_> = tree.children(tree.root()).collect();
    assert_eq!(items.len(), 1);
    assert_eq!(tree.node(items[0]).kind, NodeKind::Field);
}
