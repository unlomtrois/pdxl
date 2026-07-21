//! The corpus-mined gui vocabulary: which property keys appear under which
//! widget, and which enum-like values each key takes.
//!
//! The engine does not document widget properties anywhere dumpable, but the
//! project itself parses every `.gui` file (vanilla + mod) — so the
//! vocabulary is *mined* from real usage during the project's gui pass.
//! This self-adapts: a mod-defined widget or property joins the vocabulary
//! the moment a file uses it. Frequencies are kept so completion can rank
//! common properties first.

use std::collections::HashMap;

use pdxl_ast::{NodeKind, SyntaxTree};

/// Accumulated key/value usage across every `.gui` file.
#[derive(Debug, Default)]
pub struct GuiVocab {
    /// owner widget/property name → key → occurrence count.
    /// The owner of a block body is the *base* for `type x = base { … }`
    /// bodies and tagged values (`= base { … }`), else the field key
    /// (`icon = { … }` → `icon`).
    keys_under: HashMap<String, HashMap<String, u32>>,
    /// key → bare scalar value → occurrence count (identifier-like values
    /// only: `center`, `expanding`, `bottom|left`, `yes` — not strings,
    /// numbers, paths, or datafunctions).
    values_of: HashMap<String, HashMap<String, u32>>,
}

/// Whether a scalar value is enum-like (worth offering as a value).
fn enum_like(text: &[u8]) -> bool {
    !text.is_empty()
        && !text[0].is_ascii_digit()
        && text
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'|')
}

impl GuiVocab {
    /// Harvests one parsed `.gui` tree into the vocabulary.
    pub fn add_tree(&mut self, tree: &SyntaxTree) {
        self.walk(tree, tree.root(), None);
    }

    fn walk(&mut self, tree: &SyntaxTree, parent: pdxl_ast::NodeId, owner: Option<&str>) {
        for child in tree.children(parent) {
            let node = tree.node(child);
            match node.kind {
                NodeKind::Field => {
                    let kids = tree.child_ids(child);
                    if kids.len() != 2 {
                        continue;
                    }
                    let key_text = tree.node_text(kids[0]);
                    let Ok(key) = std::str::from_utf8(key_text) else {
                        continue;
                    };
                    if let Some(owner) = owner {
                        *self
                            .keys_under
                            .entry(owner.to_string())
                            .or_default()
                            .entry(key.to_string())
                            .or_insert(0) += 1;
                    }
                    let val = tree.node(kids[1]);
                    match val.kind {
                        NodeKind::Scalar => {
                            let text = tree.node_text(kids[1]);
                            if enum_like(text) {
                                *self
                                    .values_of
                                    .entry(key.to_string())
                                    .or_default()
                                    .entry(String::from_utf8_lossy(text).into_owned())
                                    .or_insert(0) += 1;
                            }
                        }
                        // `key = { … }` — the body's owner is the key;
                        // `key = base { … }` — the body's owner is the base.
                        NodeKind::Block => self.walk(tree, kids[1], Some(key)),
                        NodeKind::TaggedBlock => {
                            let tag = String::from_utf8_lossy(tree.node_text(kids[1])).into_owned();
                            self.walk(tree, kids[1], Some(&tag));
                        }
                        _ => {}
                    }
                }
                // Standalone tagged blocks (`template NAME { … }`) — the body
                // has no widget owner (a template body's keys belong to
                // whatever base it is eventually applied to), but nested
                // fields still contribute.
                NodeKind::TaggedBlock | NodeKind::Block => self.walk(tree, child, None),
                _ => {}
            }
        }
    }

    /// The property keys observed under `owner`, most frequent first.
    pub fn keys_for(&self, owner: &str) -> Vec<(&str, u32)> {
        let Some(map) = self.keys_under.get(owner) else {
            return Vec::new();
        };
        let mut v: Vec<(&str, u32)> = map.iter().map(|(k, &n)| (k.as_str(), n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        v
    }

    /// The enum-like values observed for `key`, most frequent first — only
    /// when the distinct set is small enough to be a real vocabulary (large
    /// sets mean the key takes arbitrary names, not an enum).
    pub fn values_for(&self, key: &str) -> Vec<(&str, u32)> {
        let Some(map) = self.values_of.get(key) else {
            return Vec::new();
        };
        if map.len() > 40 {
            return Vec::new();
        }
        let mut v: Vec<(&str, u32)> = map.iter().map(|(k, &n)| (k.as_str(), n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        v
    }

    /// Whether anything was mined (false when no `.gui` files exist).
    pub fn is_empty(&self) -> bool {
        self.keys_under.is_empty()
    }
}
