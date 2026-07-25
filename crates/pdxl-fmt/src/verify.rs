//! The gofmt-style safety check: formatting must be a pure layout change.
//!
//! The formatted output is rescanned with the same trivia scan as the input;
//! the two `(kind, text)` sequences — comments included — must be identical.
//! Any mismatch means a formatter bug, and the caller must discard the
//! output rather than write it. This is what makes `--write` safe to run
//! over a whole mod: the worst possible outcome is an error, never a
//! corrupted file.

use crate::trivia::{Item, scan};

/// Returns a human-readable description of the first divergence between the
/// input stream and the formatted output's stream, or `None` when equal.
pub(crate) fn divergence(input: &[Item<'_>], output: &str) -> Option<String> {
    let Some(out_items) = scan(output.as_bytes()) else {
        return Some("formatted output failed to lex".to_string());
    };
    let n = input.len().max(out_items.len());
    for i in 0..n {
        match (input.get(i), out_items.get(i)) {
            (Some(a), Some(b)) => {
                if a.kind != b.kind || a.text != b.text {
                    return Some(format!(
                        "item {i}: input {:?} {:?} != output {:?} {:?}",
                        a.kind,
                        String::from_utf8_lossy(a.text),
                        b.kind,
                        String::from_utf8_lossy(b.text),
                    ));
                }
            }
            (Some(a), None) => {
                return Some(format!(
                    "output ends early at item {i} (input has {:?} {:?})",
                    a.kind,
                    String::from_utf8_lossy(a.text)
                ));
            }
            (None, Some(b)) => {
                return Some(format!(
                    "output has extra item {i}: {:?} {:?}",
                    b.kind,
                    String::from_utf8_lossy(b.text)
                ));
            }
            (None, None) => unreachable!(),
        }
    }
    None
}

/// Safety check for schema ordering: layout plus field permutation may change
/// sequence, but must preserve the exact multiset of tokens and comments.
pub(crate) fn inventory_divergence(input: &[Item<'_>], output: &str) -> Option<String> {
    let Some(out_items) = scan(output.as_bytes()) else {
        return Some("field-ordered output failed to lex".to_string());
    };
    let key = |item: &Item<'_>| match item.kind {
        crate::trivia::ItemKind::Comment => (u16::MAX, item.text.to_vec()),
        crate::trivia::ItemKind::Token(kind) => (kind as u16, item.text.to_vec()),
    };
    let mut before: Vec<_> = input.iter().map(key).collect();
    let mut after: Vec<_> = out_items.iter().map(key).collect();
    before.sort_unstable();
    after.sort_unstable();
    (before != after).then(|| {
        format!(
            "field ordering changed token/comment inventory ({} items before, {} after)",
            before.len(),
            after.len()
        )
    })
}
