//! The extraction engine: one AST walk → [`FileFacts`].
//!
//! A direct port of `internal/validate`'s `extractFacts` and helpers, with the
//! CK3-specific decisions parameterized through [`Schema`]. Behavior is
//! oracle-checked byte-for-byte by the `pdxl-parity` facts differential.

use std::collections::BTreeSet;
use std::sync::Arc;

use pdxl_ast::{NodeId, NodeKind, SyntaxTree};

use crate::model::{FileFacts, Ref, Symbol, SymbolKind};
use crate::schema::{KeyForm, Schema};

/// Walks a parsed file once, collecting its definitions, aliases, and
/// references.
///
/// `rel_path` is the FileSet overlay key — it drives the def rule and the
/// per-rule reference gates. `full_path` is the on-disk path used in reference
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
            match rule.shape {
                crate::schema::DefShape::TopLevel => {
                    harvest_def(tree, node, rule.kind, rel_path, schema, &mut facts)
                }
                crate::schema::DefShape::Tree { key_prefixes } => {
                    harvest_nested_defs(tree, node, key_prefixes, rule.kind, rel_path, &mut facts)
                }
                crate::schema::DefShape::ChildrenOf { containers } => harvest_container_defs(
                    tree, node, containers, rule.kind, rel_path, schema, &mut facts,
                ),
                crate::schema::DefShape::GroupedBlocks { exclude } => harvest_grouped_defs(
                    tree, node, exclude, rule.kind, rel_path, schema, &mut facts,
                ),
            }
        }
    }

    extract_refs(
        tree,
        tree.root(),
        rel_path,
        full_path,
        b"",
        schema,
        &mut facts.refs,
    );
    facts
}

fn harvest_container_defs(
    tree: &SyntaxTree,
    node_id: NodeId,
    containers: &[&str],
    kind: SymbolKind,
    rel_path: &str,
    schema: &Schema,
    facts: &mut FileFacts,
) {
    let node = tree.node(node_id);
    if node.kind == NodeKind::Field {
        let children = tree.child_ids(node_id);
        if children.len() == 2
            && containers
                .iter()
                .any(|name| tree.node_text(children[0]) == name.as_bytes())
            && matches!(
                tree.node(children[1]).kind,
                NodeKind::Block | NodeKind::TaggedBlock
            )
        {
            for child in tree.children(children[1]) {
                harvest_def(tree, child, kind, rel_path, schema, facts);
            }
            return;
        }
    }
    for child in tree.children(node_id) {
        harvest_container_defs(tree, child, containers, kind, rel_path, schema, facts);
    }
}

/// Harvests definitions that are the block-valued children of a top-level
/// group block, excluding named group attributes. `node_id` is a top-level
/// item; if it is `GROUP = { … }`, each block-valued child whose key is not
/// in `exclude` is a definition (CK3 laws inside law groups).
fn harvest_grouped_defs(
    tree: &SyntaxTree,
    node_id: NodeId,
    exclude: &[&str],
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
    let group_body = children[1];
    if !matches!(
        tree.node(group_body).kind,
        NodeKind::Block | NodeKind::TaggedBlock
    ) {
        return;
    }
    for child in tree.children(group_body) {
        let c = tree.node(child);
        if c.kind != NodeKind::Field {
            continue;
        }
        let kids = tree.child_ids(child);
        if kids.len() != 2 {
            continue;
        }
        // Only block-valued children that aren't excluded group attributes.
        if !matches!(
            tree.node(kids[1]).kind,
            NodeKind::Block | NodeKind::TaggedBlock
        ) {
            continue;
        }
        let key = tree.node_text(kids[0]);
        if exclude.iter().any(|e| key == e.as_bytes()) {
            continue;
        }
        harvest_def(tree, child, kind, rel_path, schema, facts);
    }
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
        file: Arc::from(rel_path),
        offset: node.range.start,
        end_offset: tree.node(key_id).range.end,
        params: params.into_iter().collect(),
    });

    // Some kinds expose extra resolvable names via direct-child fields
    // (CK3 traits: group / group_equivalence).
    if let Some(alias_keys) = schema.alias_keys(kind) {
        for alias_key in alias_keys {
            if let Some(name) = direct_field_value(tree, value_id, alias_key)
                && !name.is_empty()
            {
                facts.aliases.push(Symbol {
                    name,
                    kind,
                    file: Arc::from(rel_path),
                    offset: node.range.start,
                    // Go parity: alias EndOffset equals the def's SrcStart.
                    end_offset: node.range.start,
                    params: Vec::new(),
                });
            }
        }
    }
}

/// Recursively harvests tree-shaped definitions (CK3 landed titles): a key is
/// a definition iff it starts with one of `prefixes` AND its value is a block,
/// and its block is recursed into for child definitions. Attribute keys
/// (`color`, `capital`, `cultural_names`, …) are neither definitions nor
/// recursion targets — which also keeps loc-key decoys like
/// `cultural_names = { x = k_something }` out (scalar value, not a block).
///
/// `params` is left empty for nested definitions: titles carry no `$PARAM$`s,
/// and collecting over each subtree would be quadratic on real title trees
/// (a single empire subtree spans thousands of lines).
fn harvest_nested_defs(
    tree: &SyntaxTree,
    node_id: NodeId,
    prefixes: &[&str],
    kind: SymbolKind,
    rel_path: &str,
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
    if value.kind != NodeKind::Block && value.kind != NodeKind::TaggedBlock {
        return;
    }
    let key = tree.node_text(key_id);
    if !prefixes.iter().any(|p| key.starts_with(p.as_bytes())) {
        return;
    }

    facts.defs.push(Symbol {
        name: String::from_utf8_lossy(key).into_owned(),
        kind,
        file: Arc::from(rel_path),
        offset: node.range.start,
        end_offset: tree.node(key_id).range.end,
        params: Vec::new(),
    });
    for child in tree.children(value_id) {
        harvest_nested_defs(tree, child, prefixes, kind, rel_path, facts);
    }
}

/// Recursively collects references from the subtree rooted at `node_id`.
/// `rel_path` drives per-rule gates; `path` labels the extracted refs.
fn extract_refs(
    tree: &SyntaxTree,
    node_id: NodeId,
    rel_path: &str,
    path: &str,
    parent_key: &[u8],
    schema: &Schema,
    refs: &mut Vec<Ref>,
) {
    let node = tree.node(node_id);
    if node.kind == NodeKind::Field {
        let children = tree.child_ids(node_id);
        if children.len() == 2 {
            let key = tree.node_text(children[0]);
            extract_field_refs(
                tree,
                key,
                children[1],
                rel_path,
                path,
                parent_key,
                schema,
                refs,
            );
            // The key itself is a scalar position (scope literals like
            // `title:k_x = { … }` appear as keys); the value's subtree gets
            // this field's key as its parent.
            extract_refs(tree, children[0], rel_path, path, parent_key, schema, refs);
            extract_refs(tree, children[1], rel_path, path, key, schema, refs);
            return;
        }
    }
    // Self-identifying scope literals (`title:<name>[.chain]`) can appear in
    // ANY scalar position — value, key, or list item — so every scalar in the
    // tree is scanned, not just known-key values.
    if node.kind == NodeKind::Scalar {
        scan_prefix_refs(tree, node_id, rel_path, path, schema, refs);
    }
    for child in tree.children(node_id) {
        extract_refs(tree, child, rel_path, path, parent_key, schema, refs);
    }
}

/// Extracts a reference from a scalar of the form `<prefix>:<name>[.chain…]`
/// for each configured scope prefix. The reference's byte range covers exactly
/// `<name>` — not the prefix, not the trailing scope chain — so diagnostics
/// and go-to-definition land on the title name itself.
fn scan_prefix_refs(
    tree: &SyntaxTree,
    node_id: NodeId,
    rel_path: &str,
    path: &str,
    schema: &Schema,
    refs: &mut Vec<Ref>,
) {
    let text = tree.node_text(node_id);
    for rule in schema.scope_rules() {
        let prefix = rule.prefix;
        let plen = prefix.len();
        if !rule.applies(rel_path)
            || text.len() <= plen + 1
            || !text.starts_with(prefix.as_bytes())
            || text[plen] != b':'
        {
            continue;
        }
        let name_start = plen + 1;
        let name_end = text[name_start..]
            .iter()
            .position(|&b| b == b'.')
            .map_or(text.len(), |i| name_start + i);
        let name = String::from_utf8_lossy(&text[name_start..name_end]);
        if schema.skip_ref_value(&name) {
            continue; // macro-interpolated ($X$) or empty names
        }

        let node = tree.node(node_id);
        let start = node.range.start + name_start as u32;
        let end = node.range.start + name_end as u32;
        refs.push(Ref {
            kind: rule.kind,
            name: name.into_owned(),
            file: Arc::from(path),
            start,
            end,
        });
        break; // a scalar names at most one scope literal
    }
}

/// Collects references from a single `key = value` field, applying every
/// key-triggered rule (in schema declaration order) whose gate admits the
/// file and whose form matches the value's node kind.
#[allow(clippy::too_many_arguments)]
fn extract_field_refs(
    tree: &SyntaxTree,
    key: &[u8],
    value_id: NodeId,
    rel_path: &str,
    path: &str,
    parent_key: &[u8],
    schema: &Schema,
    refs: &mut Vec<Ref>,
) {
    let Ok(key) = std::str::from_utf8(key) else {
        return; // rule keys are ASCII; a non-UTF-8 key matches nothing
    };
    let Some(rules) = schema.key_rules(key) else {
        return;
    };
    let value = tree.node(value_id);

    for rule in rules {
        if !rule.applies(rel_path) {
            continue;
        }
        match rule.form {
            // Scalar form: key = value.
            KeyForm::Value if value.kind == NodeKind::Scalar => {
                append_ref(tree, rule.kind, value_id, path, schema, refs);
            }
            // Scalar form constrained to a parent block: option = { name = X }.
            KeyForm::ValueUnder(parent) if value.kind == NodeKind::Scalar => {
                if parent_key == parent.as_bytes() {
                    append_ref(tree, rule.kind, value_id, path, schema, refs);
                }
            }
            // Block form carrying a named field: key = { field = value … }.
            KeyForm::BlockField(field) if value.kind == NodeKind::Block => {
                if let Some(id_node) = direct_field_node(tree, value_id, field)
                    && tree.node(id_node).kind == NodeKind::Scalar
                {
                    append_ref(tree, rule.kind, id_node, path, schema, refs);
                }
            }
            // List form: key = { item item … } — loose scalar items.
            KeyForm::List if value.kind == NodeKind::Block => {
                for item in tree.children(value_id) {
                    if tree.node(item).kind == NodeKind::Scalar {
                        append_ref(tree, rule.kind, item, path, schema, refs);
                    }
                }
            }
            // Weighted form: key = { WEIGHT = id … } — numeric-keyed entries.
            KeyForm::Weighted if value.kind == NodeKind::Block => {
                extract_weighted_refs(tree, rule.kind, value_id, path, schema, refs);
            }
            _ => {}
        }
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

    refs.push(Ref {
        kind,
        name: val.into_owned(),
        file: Arc::from(path),
        start: value.range.start,
        end: value.range.end,
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
