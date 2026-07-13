//! The extraction engine: one AST walk → [`FileFacts`].
//!
//! A direct port of `internal/validate`'s `extractFacts` and helpers, with the
//! CK3-specific decisions parameterized through [`Schema`]. Behavior is
//! oracle-checked byte-for-byte by the `pdxl-parity` facts differential.

use std::collections::BTreeSet;

use pdxl_ast::{NodeId, NodeKind, SyntaxTree};

use crate::model::{FileFacts, Ref, Symbol, SymbolKind};
use crate::schema::Schema;

/// Walks a parsed file once, collecting its definitions, aliases, and
/// references.
///
/// `rel_path` is the FileSet overlay key — it drives the def rule and the
/// list/weighted gating. `full_path` is the on-disk path used in reference
/// `loc` strings, so diagnostics point at a clickable file.
pub fn extract_facts(
    tree: &SyntaxTree,
    rel_path: &str,
    full_path: &str,
    schema: &Schema,
) -> FileFacts {
    let mut facts = FileFacts::default();

    if let Some(rule) = schema.rule_for(rel_path) {
        for node in tree.children(tree.root()) {
            harvest_def(tree, node, rule.kind, rel_path, schema, &mut facts);
        }
    }

    let gated =
        !schema.list_gate_prefix.is_empty() && rel_path.starts_with(schema.list_gate_prefix);
    extract_refs(tree, tree.root(), full_path, gated, schema, &mut facts.refs);
    facts
}

/// Records the definition (and any aliases) for a single top-level node, if it
/// is a `NAME = { … }` field.
fn harvest_def(
    tree: &SyntaxTree,
    node_id: NodeId,
    kind: SymbolKind,
    rel_path: &str,
    schema: &Schema,
    facts: &mut FileFacts,
) {
    let node = tree.node(node_id);
    if node.kind != NodeKind::Field {
        return;
    }
    let children = tree.child_ids(node_id);
    if children.len() != 2 {
        return;
    }
    let (key_id, value_id) = (children[0], children[1]);
    let value = tree.node(value_id);
    // A definition has a block body; skips metadata like `namespace = x`.
    if value.kind != NodeKind::Block && value.kind != NodeKind::TaggedBlock {
        return;
    }

    // BTreeSet gives the sorted, deduped param order Go gets from sortedKeys.
    let mut params = BTreeSet::new();
    collect_params(tree, value_id, &mut params);
    facts.defs.push(Symbol {
        name: String::from_utf8_lossy(tree.node_text(key_id)).into_owned(),
        kind,
        file: rel_path.to_string(),
        offset: node.range.start,
        end_offset: tree.node(key_id).range.end,
        params: params.into_iter().collect(),
    });

    // Some kinds expose extra resolvable names via direct-child fields
    // (CK3 traits: group / group_equivalence).
    if let Some(alias_keys) = schema.alias_keys.get(&kind) {
        for alias_key in *alias_keys {
            if let Some(name) = direct_field_value(tree, value_id, alias_key)
                && !name.is_empty()
            {
                facts.aliases.push(Symbol {
                    name,
                    kind,
                    file: rel_path.to_string(),
                    offset: node.range.start,
                    // Go parity: alias EndOffset equals the def's SrcStart.
                    end_offset: node.range.start,
                    params: Vec::new(),
                });
            }
        }
    }
}

/// Recursively collects references from the subtree rooted at `node_id`.
/// `gated` enables list/weighted forms (on_action files only in CK3).
fn extract_refs(
    tree: &SyntaxTree,
    node_id: NodeId,
    path: &str,
    gated: bool,
    schema: &Schema,
    refs: &mut Vec<Ref>,
) {
    if tree.node(node_id).kind == NodeKind::Field {
        let children = tree.child_ids(node_id);
        if children.len() == 2 {
            let key = tree.node_text(children[0]);
            extract_field_refs(tree, key, children[1], path, gated, schema, refs);
        }
    }
    for child in tree.children(node_id) {
        extract_refs(tree, child, path, gated, schema, refs);
    }
}

/// Collects references from a single `key = value` field.
fn extract_field_refs(
    tree: &SyntaxTree,
    key: &[u8],
    value_id: NodeId,
    path: &str,
    gated: bool,
    schema: &Schema,
    refs: &mut Vec<Ref>,
) {
    let Ok(key) = std::str::from_utf8(key) else {
        return; // rule keys are ASCII; a non-UTF-8 key matches nothing
    };
    let value = tree.node(value_id);

    // Scalar form: key = value.
    if let Some(&kind) = schema.ref_rules.get(key)
        && value.kind == NodeKind::Scalar
    {
        append_ref(tree, kind, value_id, path, schema, refs);
    }
    // Block form carrying an id: key = { id = value … }.
    if let Some(&kind) = schema.block_id_ref_rules.get(key)
        && value.kind == NodeKind::Block
        && let Some(id_node) = direct_field_node(tree, value_id, "id")
        && tree.node(id_node).kind == NodeKind::Scalar
    {
        append_ref(tree, kind, id_node, path, schema, refs);
    }
    if !gated || value.kind != NodeKind::Block {
        return;
    }
    // List form: key = { item item … } — loose scalar items.
    if let Some(&kind) = schema.list_ref_rules.get(key) {
        for item in tree.children(value_id) {
            if tree.node(item).kind == NodeKind::Scalar {
                append_ref(tree, kind, item, path, schema, refs);
            }
        }
    }
    // Weighted form: key = { WEIGHT = id … } — only numeric-keyed entries.
    if let Some(&kind) = schema.weighted_ref_rules.get(key) {
        extract_weighted_refs(tree, kind, value_id, path, schema, refs);
    }
}

/// Collects references from a weighted block like `random_events = { 50 = ns.id }`:
/// only numeric-keyed entries are weight→ref; word keys are config, and a
/// numeric value (`100 = 0`) means "no event".
fn extract_weighted_refs(
    tree: &SyntaxTree,
    kind: SymbolKind,
    block_id: NodeId,
    path: &str,
    schema: &Schema,
    refs: &mut Vec<Ref>,
) {
    for field in tree.children(block_id) {
        if tree.node(field).kind != NodeKind::Field {
            continue;
        }
        let kids = tree.child_ids(field);
        if kids.len() != 2 || tree.node(kids[1]).kind != NodeKind::Scalar {
            continue;
        }
        if starts_with_digit(tree.node_text(kids[0])) && !starts_with_digit(tree.node_text(kids[1]))
        {
            append_ref(tree, kind, kids[1], path, schema, refs);
        }
    }
}

/// Records a resolvable reference from a scalar value node, applying the
/// quote-strip, macro-concatenation, and scope/macro skips.
fn append_ref(
    tree: &SyntaxTree,
    kind: SymbolKind,
    value_id: NodeId,
    path: &str,
    schema: &Schema,
    refs: &mut Vec<Ref>,
) {
    let value = tree.node(value_id);
    let raw = tree.node_text(value_id);
    let val = String::from_utf8_lossy(trim_quotes(raw));

    // A '$' immediately after the value means it is the prefix of a
    // macro-interpolated identifier (e.g. education_$EDUCATION$_5); the lexer
    // splits it, so only the prefix was captured.
    let src = tree.source();
    let concat_macro =
        (value.range.end as usize) < src.len() && src[value.range.end as usize] == b'$';
    if concat_macro || schema.skip_ref_value(&val) {
        return;
    }

    let (line, col) = pdxl_src::line_col(src, value.range.start);
    refs.push(Ref {
        kind,
        name: val.into_owned(),
        file: path.to_string(),
        start: value.range.start,
        end: value.range.end,
        loc: format!("{path}:{line}:{col}"),
    });
}

/// The scalar value of a direct-child `key = value` field in `block_id`.
fn direct_field_value(tree: &SyntaxTree, block_id: NodeId, key: &str) -> Option<String> {
    let node_id = direct_field_node(tree, block_id, key)?;
    if tree.node(node_id).kind != NodeKind::Scalar {
        return None;
    }
    Some(String::from_utf8_lossy(tree.node_text(node_id)).into_owned())
}

/// The value node of a direct-child `key = value` field in `block_id`.
fn direct_field_node(tree: &SyntaxTree, block_id: NodeId, key: &str) -> Option<NodeId> {
    for child in tree.children(block_id) {
        if tree.node(child).kind != NodeKind::Field {
            continue;
        }
        let kids = tree.child_ids(child);
        if kids.len() == 2 && tree.node_text(kids[0]) == key.as_bytes() {
            return Some(kids[1]);
        }
    }
    None
}

/// Walks the subtree recording every `$PARAM$` name in scalar/tagged-block
/// text. Block nodes carry no source span themselves, so params come from
/// their leaf descendants (Go's `collectParams`).
fn collect_params(tree: &SyntaxTree, node_id: NodeId, seen: &mut BTreeSet<String>) {
    let node = tree.node(node_id);
    if node.kind == NodeKind::Scalar || node.kind == NodeKind::TaggedBlock {
        find_macro_params(tree.node_text(node_id), seen);
    }
    for child in tree.children(node_id) {
        collect_params(tree, child, seen);
    }
}

/// Finds every `$NAME$` occurrence, matching Go's `\$(\w+)\$` regex semantics:
/// `\w` is ASCII `[0-9A-Za-z_]`, matches are non-overlapping left-to-right, and
/// the scan resumes after each match's closing `$`.
fn find_macro_params(text: &[u8], seen: &mut BTreeSet<String>) {
    let mut i = 0;
    while i < text.len() {
        if text[i] != b'$' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < text.len() && is_word_byte(text[j]) {
            j += 1;
        }
        if j > i + 1 && j < text.len() && text[j] == b'$' {
            // Word bytes only — always valid UTF-8.
            seen.insert(String::from_utf8_lossy(&text[i + 1..j]).into_owned());
            i = j + 1; // resume after the closing '$'
        } else {
            i += 1;
        }
    }
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Whether `s` begins with an ASCII digit (a weight or config number; event
/// IDs start with a namespace letter).
fn starts_with_digit(s: &[u8]) -> bool {
    s.first().is_some_and(u8::is_ascii_digit)
}

/// Strips all leading and trailing `"` bytes (Go `strings.Trim` semantics).
fn trim_quotes(mut b: &[u8]) -> &[u8] {
    while b.first() == Some(&b'"') {
        b = &b[1..];
    }
    while b.last() == Some(&b'"') {
        b = &b[..b.len() - 1];
    }
    b
}
