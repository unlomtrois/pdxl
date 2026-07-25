//! The extraction engine: one AST walk → [`FileFacts`].
//!
//! A direct port of `internal/validate`'s `extractFacts` and helpers, with the
//! CK3-specific decisions parameterized through [`Schema`]. Behavior was
//! oracle-checked byte-for-byte against Go and is now pinned by the facts
//! golden snapshots in `pdxl-ck3` (`tests/facts.rs`).

use std::collections::BTreeSet;
use std::sync::Arc;

use pdxl_ast::{NodeId, NodeKind, SyntaxTree};

use crate::kind::KindId;
use crate::model::{CallTargets, FileFacts, Ref, Symbol};
use crate::schema::{KeyForm, Schema};

/// Walks a parsed file once, collecting its definitions, aliases, and
/// references.
///
/// `rel_path` is the FileSet overlay key — it drives the def rule and the
/// per-rule reference gates. `full_path` is the on-disk path used in reference
/// `loc` strings, so diagnostics point at a clickable file.
///
/// `calls` supplies the project-wide scripted effect/trigger names used to
/// recognize call-by-name references; pass `None` to skip that pass (e.g. the
/// pre-pass that gathers those very names, or callers that don't need calls).
pub fn extract_facts(
    tree: &SyntaxTree,
    rel_path: &str,
    full_path: &str,
    schema: &Schema,
    calls: Option<&CallTargets>,
) -> FileFacts {
    let mut facts = FileFacts::default();

    // A top-level `KEYWORD NAME = { … }` typed definition parses as a bare
    // scalar keyword followed by a `NAME = { }` field sibling (CLAUDE.md). The
    // keyword decides the kind (`scripted_effect` → ScriptedEffect) regardless
    // of directory, so these are harvested before — and instead of — the
    // directory rule, which would otherwise mis-kind them (e.g. as events).
    // A file can match several def rules (EU5 building files host both the
    // building defs and nested production-method containers); every matching
    // rule's shape is applied per top-level node.
    let rules: Vec<&crate::schema::DefRule> = schema.rules_for(rel_path).collect();
    let mut pending_typed: Option<KindId> = None;
    for node in tree.children(tree.root()) {
        let kind = tree.node(node).kind;
        if kind == NodeKind::Scalar {
            pending_typed = std::str::from_utf8(tree.node_text(node))
                .ok()
                .and_then(|kw| schema.typed_def_kind(kw));
            continue;
        }
        if let Some(typed_kind) = pending_typed.take()
            && kind == NodeKind::Field
        {
            harvest_def(tree, node, typed_kind, rel_path, schema, &mut facts);
            continue;
        }
        // A keyed-value definition (`namespace = X`): the value is the symbol.
        if kind == NodeKind::Field
            && let Some(kv_kind) = keyed_value_kind(tree, node, schema)
        {
            harvest_keyed_value_def(tree, node, kv_kind, rel_path, &mut facts);
            continue;
        }
        // A script-constant definition (`@name = value`) is skipped here so
        // shapes like TopLevelValued (script values) don't claim it as a
        // definition of their own kind; the dedicated walk below harvests it
        // (constants may also be defined nested, e.g. inside ethnicities).
        if kind == NodeKind::Field
            && let Some(&key_id) = tree.child_ids(node).first()
            && tree.node_text(key_id).starts_with(b"@")
        {
            continue;
        }
        for rule in &rules {
            match rule.shape {
                crate::schema::DefShape::TopLevel => {
                    harvest_def(tree, node, rule.kind, rel_path, schema, &mut facts)
                }
                crate::schema::DefShape::TopLevelValued => {
                    harvest_valued_def(tree, node, rule.kind, rel_path, &mut facts)
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
                crate::schema::DefShape::QualifiedFields { separator } => {
                    harvest_qualified_fields(tree, node, separator, rule.kind, rel_path, &mut facts)
                }
                // CSV-sourced definitions never reach the script extractor —
                // the project layer routes those files to its own CSV reader.
                crate::schema::DefShape::IdCsv => {}
            }
        }
    }

    harvest_constant_defs(tree, tree.root(), rel_path, &mut facts);
    if schema.has_nested_value_defs() {
        harvest_nested_value_defs(tree, tree.root(), rel_path, schema, &mut facts);
    }

    extract_refs(
        tree,
        tree.root(),
        rel_path,
        full_path,
        b"",
        0,
        schema,
        &mut facts.refs,
    );
    // Constant references are collected inline (they need no separate walk)
    // and split out afterwards: they resolve file-locally, so they must not
    // reach the global reference stream.
    facts.constant_refs = facts
        .refs
        .extract_if(.., |r| r.kind == crate::kind::SCRIPT_CONSTANT)
        .collect();

    // Call-by-name references (`my_effect = yes`).
    if let Some(targets) = calls {
        facts.calls = extract_calls(tree, full_path, targets);
    }
    facts
}

/// Collects every call-by-name reference in a parsed file: nested `KEY = value`
/// fields whose `KEY` names a defined scripted effect/trigger (a whole-project
/// fact supplied via `targets`). A matching top-level field is skipped — there
/// the name is the definition, not a call. This is a project-level second pass,
/// because the callable-name set isn't known until every file's definitions
/// (including inline typed defs) have been harvested.
pub fn extract_calls(tree: &SyntaxTree, full_path: &str, targets: &CallTargets) -> Vec<Ref> {
    let mut calls = Vec::new();
    for item in tree.children(tree.root()) {
        walk_calls(tree, item, full_path, targets, true, &mut calls);
    }
    calls
}

/// Records name-gated references in the subtree rooted at `node_id`:
/// - **scripted effect/trigger calls** — a nested `KEY = value` field whose
///   `KEY` names one (skipped at file top level, where that name is the
///   definition itself);
/// - **script-value references** — any scalar in *value* position (a field's
///   value, or a loose list item) whose text names a defined script value.
///
/// Each recorded range covers the matched name, so go-to-definition and
/// find-references land on it precisely.
fn walk_calls(
    tree: &SyntaxTree,
    node_id: NodeId,
    path: &str,
    targets: &CallTargets,
    is_top: bool,
    calls: &mut Vec<Ref>,
) {
    let node = tree.node(node_id);
    if node.kind == NodeKind::Field {
        let children = tree.child_ids(node_id);
        if children.len() == 2 {
            // Key position: scripted effect/trigger call (not at top level).
            if !is_top && let Ok(key) = std::str::from_utf8(tree.node_text(children[0])) {
                let kind = if targets.effects.contains(key) {
                    Some(targets.kinds.effect)
                } else if targets.triggers.contains(key) {
                    Some(targets.kinds.trigger)
                } else {
                    None
                };
                if let Some(kind) = kind {
                    push_name_ref(tree, kind, children[0], path, calls);
                }
            }
            // Value position: script-value reference (`add_stress = X`).
            push_script_value(tree, children[1], targets, path, calls);
        }
    } else if node.kind == NodeKind::Block {
        // Loose list items (`add_gold = { named_a named_b }`) are values too.
        for item in tree.children(node_id) {
            if tree.node(item).kind == NodeKind::Scalar {
                push_script_value(tree, item, targets, path, calls);
            }
        }
    }
    // A top-level field's descendants are no longer top level.
    for child in tree.children(node_id) {
        walk_calls(tree, child, path, targets, false, calls);
    }
}

/// Emits a script-value reference if `value_id` is a scalar naming a defined
/// script value. Bare names only — a scope chain (`mother.example_age`) or
/// numeric literal never matches the name set, so both are skipped for free.
fn push_script_value(
    tree: &SyntaxTree,
    value_id: NodeId,
    targets: &CallTargets,
    path: &str,
    calls: &mut Vec<Ref>,
) {
    if tree.node(value_id).kind != NodeKind::Scalar {
        return;
    }
    if let Ok(val) = std::str::from_utf8(tree.node_text(value_id))
        && targets.script_values.contains(val)
    {
        push_name_ref(tree, targets.kinds.value, value_id, path, calls);
    }
}

/// Records a name-gated reference covering exactly the `node_id` scalar's range.
fn push_name_ref(
    tree: &SyntaxTree,
    kind: KindId,
    node_id: NodeId,
    path: &str,
    calls: &mut Vec<Ref>,
) {
    let node = tree.node(node_id);
    calls.push(Ref {
        kind,
        alt: &[],
        name: String::from_utf8_lossy(tree.node_text(node_id)).into_owned(),
        file: Arc::from(path),
        start: node.range.start,
        end: node.range.end,
    });
}

fn harvest_container_defs(
    tree: &SyntaxTree,
    node_id: NodeId,
    containers: &[&str],
    kind: KindId,
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
                // Containers may nest (EU5 start scenarios:
                // `countries = { countries = { TAG = { … } } }`) — a child
                // that is itself a named container is descended into, not
                // harvested as a definition.
                if is_container_field(tree, child, containers) {
                    harvest_container_defs(tree, child, containers, kind, rel_path, schema, facts);
                } else {
                    harvest_def(tree, child, kind, rel_path, schema, facts);
                }
            }
            return;
        }
    }
    for child in tree.children(node_id) {
        harvest_container_defs(tree, child, containers, kind, rel_path, schema, facts);
    }
}

/// Whether `node_id` is a `NAME = { … }` field whose key names a container.
fn is_container_field(tree: &SyntaxTree, node_id: NodeId, containers: &[&str]) -> bool {
    if tree.node(node_id).kind != NodeKind::Field {
        return false;
    }
    let kids = tree.child_ids(node_id);
    kids.len() == 2
        && containers
            .iter()
            .any(|name| tree.node_text(kids[0]) == name.as_bytes())
        && matches!(
            tree.node(kids[1]).kind,
            NodeKind::Block | NodeKind::TaggedBlock
        )
}

/// Harvests definitions that are the block-valued children of a top-level
/// group block, excluding named group attributes. `node_id` is a top-level
/// item; if it is `GROUP = { … }`, each block-valued child whose key is not
/// in `exclude` is a definition (CK3 laws inside law groups).
fn harvest_grouped_defs(
    tree: &SyntaxTree,
    node_id: NodeId,
    exclude: &[&str],
    kind: KindId,
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

/// Harvests `NAMESPACE = { FIELD = value }` as `NAMESPACE|FIELD`-style
/// definitions. The symbol range lands on FIELD while its lookup name retains
/// the namespace required by reference syntax.
fn harvest_qualified_fields(
    tree: &SyntaxTree,
    node_id: NodeId,
    separator: &str,
    kind: KindId,
    rel_path: &str,
    facts: &mut FileFacts,
) {
    if tree.node(node_id).kind != NodeKind::Field {
        return;
    }
    let outer = tree.child_ids(node_id);
    if outer.len() != 2 || tree.node(outer[1]).kind != NodeKind::Block {
        return;
    }
    let namespace = String::from_utf8_lossy(tree.node_text(outer[0]));
    if namespace.starts_with('@') {
        return;
    }
    for child in tree.children(outer[1]) {
        if tree.node(child).kind != NodeKind::Field {
            continue;
        }
        let kids = tree.child_ids(child);
        if kids.len() != 2 {
            continue;
        }
        let key = tree.node_text(kids[0]);
        if key.starts_with(b"@") {
            continue;
        }
        facts.defs.push(Symbol {
            name: format!("{namespace}{separator}{}", String::from_utf8_lossy(key)),
            kind,
            file: Arc::from(rel_path),
            offset: tree.node(kids[0]).range.start,
            end_offset: tree.node(kids[0]).range.end,
            params: Vec::new(),
        });
    }
}

/// Records the definition (and any aliases) for a single top-level node, if it
/// is a `NAME = { … }` field.
fn harvest_def(
    tree: &SyntaxTree,
    node_id: NodeId,
    kind: KindId,
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

    // Some kinds expose extra resolvable names via direct-child fields. The
    // field's value may be a single scalar (CK3 traits: group /
    // group_equivalence) or a list block (CK3 game concepts:
    // `alias = { vassals vassalize … }`) — each name resolves to this def.
    if let Some(alias_keys) = schema.alias_keys(kind) {
        for alias_key in alias_keys {
            let Some(field_val) = direct_field_node(tree, value_id, alias_key) else {
                continue;
            };
            match tree.node(field_val).kind {
                NodeKind::Scalar => {
                    let name = String::from_utf8_lossy(tree.node_text(field_val)).into_owned();
                    if !name.is_empty() {
                        push_alias(facts, name, kind, rel_path, node.range.start);
                    }
                }
                NodeKind::Block | NodeKind::TaggedBlock => {
                    for item in tree.children(field_val) {
                        if tree.node(item).kind != NodeKind::Scalar {
                            continue;
                        }
                        let name = String::from_utf8_lossy(tree.node_text(item)).into_owned();
                        if !name.is_empty() {
                            push_alias(facts, name, kind, rel_path, node.range.start);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Records one extra resolvable name for a definition. Go parity: an alias's
/// offset and EndOffset both equal the def's SrcStart, so navigation from a
/// reference-by-alias lands on the definition.
fn push_alias(facts: &mut FileFacts, name: String, kind: KindId, rel_path: &str, def_start: u32) {
    facts.aliases.push(Symbol {
        name,
        kind,
        file: Arc::from(rel_path),
        offset: def_start,
        end_offset: def_start,
        params: Vec::new(),
    });
}

/// Recursively records script-constant definitions: every `@name = value`
/// field, at any depth — most sit at file top level, but ethnicities and
/// coats of arms define them inside blocks (the engine treats them as
/// file-scoped either way). The symbol name keeps its `@` so definitions and
/// references match textually.
fn harvest_constant_defs(
    tree: &SyntaxTree,
    node_id: NodeId,
    rel_path: &str,
    facts: &mut FileFacts,
) {
    let node = tree.node(node_id);
    if node.kind == NodeKind::Field {
        let children = tree.child_ids(node_id);
        if children.len() == 2 && tree.node_text(children[0]).starts_with(b"@") {
            facts.constants.push(Symbol {
                name: String::from_utf8_lossy(tree.node_text(children[0])).into_owned(),
                kind: crate::kind::SCRIPT_CONSTANT,
                file: Arc::from(rel_path),
                offset: node.range.start,
                end_offset: tree.node(children[0]).range.end,
                params: Vec::new(),
            });
            return;
        }
    }
    for child in tree.children(node_id) {
        harvest_constant_defs(tree, child, rel_path, facts);
    }
}

/// Recursively records nested keyed-value definitions: any-depth
/// `KEY = value` fields whose `KEY` the schema maps (EU5's
/// `define_unique_country_tag = SAGEO` inside an event effect creates the
/// tag). The definition is the *value*, like `namespace = X`.
fn harvest_nested_value_defs(
    tree: &SyntaxTree,
    node_id: NodeId,
    rel_path: &str,
    schema: &Schema,
    facts: &mut FileFacts,
) {
    let node = tree.node(node_id);
    if node.kind == NodeKind::Field {
        let kids = tree.child_ids(node_id);
        if kids.len() == 2
            && tree.node(kids[1]).kind == NodeKind::Scalar
            && let Ok(key) = std::str::from_utf8(tree.node_text(kids[0]))
            && let Some(kind) = schema.nested_value_def_kind(key)
        {
            let value = tree.node(kids[1]);
            facts.defs.push(Symbol {
                name: String::from_utf8_lossy(trim_quotes(tree.node_text(kids[1]))).into_owned(),
                kind,
                file: Arc::from(rel_path),
                offset: value.range.start,
                end_offset: value.range.end,
                params: Vec::new(),
            });
            return;
        }
    }
    for child in tree.children(node_id) {
        harvest_nested_value_defs(tree, child, rel_path, schema, facts);
    }
}

/// The keyed-value kind of a top-level `KEY = value` field (`namespace = X` →
/// Namespace), or `None` if `KEY` isn't a keyed-value key.
fn keyed_value_kind(tree: &SyntaxTree, node_id: NodeId, schema: &Schema) -> Option<KindId> {
    let children = tree.child_ids(node_id);
    if children.len() != 2 {
        return None;
    }
    let key = std::str::from_utf8(tree.node_text(children[0])).ok()?;
    schema.keyed_value_def_kind(key)
}

/// Records a keyed-value definition: the *value* of `KEY = value` is the symbol
/// (`namespace = T4N_drill` → a Namespace named `T4N_drill`), so hovering the
/// value shows the file's doc while nothing else in the file is touched.
fn harvest_keyed_value_def(
    tree: &SyntaxTree,
    node_id: NodeId,
    kind: KindId,
    rel_path: &str,
    facts: &mut FileFacts,
) {
    let children = tree.child_ids(node_id);
    if children.len() != 2 {
        return;
    }
    let value_id = children[1];
    if tree.node(value_id).kind != NodeKind::Scalar {
        return; // `namespace = { … }` is not a namespace declaration
    }
    let value = tree.node(value_id);
    facts.defs.push(Symbol {
        name: String::from_utf8_lossy(trim_quotes(tree.node_text(value_id))).into_owned(),
        kind,
        file: Arc::from(rel_path),
        offset: value.range.start,
        end_offset: value.range.end,
        params: Vec::new(),
    });
}

/// Records a definition whose value may be a scalar *or* a block (CK3 script
/// values: `minor_stress_gain = 10` and `formula = { … }` are both defs). Like
/// [`harvest_def`] but without the block requirement; no aliases apply.
fn harvest_valued_def(
    tree: &SyntaxTree,
    node_id: NodeId,
    kind: KindId,
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
    let key_id = children[0];
    let mut params = BTreeSet::new();
    collect_params(tree, children[1], &mut params);
    facts.defs.push(Symbol {
        name: String::from_utf8_lossy(tree.node_text(key_id)).into_owned(),
        kind,
        file: Arc::from(rel_path),
        offset: node.range.start,
        end_offset: tree.node(key_id).range.end,
        params: params.into_iter().collect(),
    });
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
    kind: KindId,
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
/// `depth` counts enclosing fields (a top-level definition's body fields sit
/// at depth 1 — what [`KeyForm::ValueTop`] matches).
#[allow(clippy::too_many_arguments)]
fn extract_refs(
    tree: &SyntaxTree,
    node_id: NodeId,
    rel_path: &str,
    path: &str,
    parent_key: &[u8],
    depth: u32,
    schema: &Schema,
    refs: &mut Vec<Ref>,
) {
    let node = tree.node(node_id);
    if node.kind == NodeKind::Field {
        let children = tree.child_ids(node_id);
        if children.len() == 2 {
            let key = tree.node_text(children[0]);
            // Top-level `X = { … }` block keys as references (province ids in
            // `history/provinces/`). `@var` script constants are not names.
            if depth == 0
                && !key.starts_with(b"@")
                && matches!(
                    tree.node(children[1]).kind,
                    NodeKind::Block | NodeKind::TaggedBlock
                )
            {
                for rule in schema.top_key_rules() {
                    if rule.applies(rel_path) {
                        append_ref(tree, rule.kind, &[], children[0], path, schema, refs);
                    }
                }
            }
            extract_field_refs(
                tree,
                key,
                children[1],
                rel_path,
                path,
                parent_key,
                depth,
                schema,
                refs,
            );
            // The key itself is a scalar position for scope literals only
            // (`title:k_x = { … }` appears as a key) — never for script
            // constants, whose `@name` keys are the definitions. The value's
            // subtree gets this field's key as its parent.
            scan_prefix_refs(tree, children[0], rel_path, path, schema, refs);
            extract_refs(
                tree,
                children[1],
                rel_path,
                path,
                key,
                depth + 1,
                schema,
                refs,
            );
            return;
        }
    }
    // Self-identifying scope literals (`title:<name>[.chain]`) can appear in
    // ANY scalar position — value, key, or list item — so every scalar in the
    // tree is scanned, not just known-key values.
    if node.kind == NodeKind::Scalar {
        scan_prefix_refs(tree, node_id, rel_path, path, schema, refs);
        // A `@name` scalar below file top level is a script-constant
        // reference (top-level `@name` keys are the definitions). `@[…]`
        // inline math is skipped — its variables appear un-prefixed inside.
        let text = tree.node_text(node_id);
        if depth >= 1 && text.len() > 1 && text[0] == b'@' && text[1] != b'[' {
            refs.push(Ref {
                kind: crate::kind::SCRIPT_CONSTANT,
                alt: &[],
                name: String::from_utf8_lossy(text).into_owned(),
                file: Arc::from(path),
                start: node.range.start,
                end: node.range.end,
            });
        }
    }
    for child in tree.children(node_id) {
        extract_refs(tree, child, rel_path, path, parent_key, depth, schema, refs);
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
        // A '$' immediately after the scalar means the name is the prefix of
        // a macro-interpolated identifier (`culture_innovation:innovation_$X$`)
        // — the lexer splits it, so only the prefix was captured.
        let src = tree.source();
        if name_end == text.len()
            && (node.range.end as usize) < src.len()
            && src[node.range.end as usize] == b'$'
        {
            continue;
        }
        let start = node.range.start + name_start as u32;
        let end = node.range.start + name_end as u32;
        refs.push(Ref {
            kind: rule.kind,
            alt: rule.alt,
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
    depth: u32,
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
                append_ref(tree, rule.kind, rule.alt, value_id, path, schema, refs);
            }
            // Scalar form, only directly inside a top-level definition body.
            KeyForm::ValueTop if value.kind == NodeKind::Scalar && depth == 1 => {
                append_ref(tree, rule.kind, rule.alt, value_id, path, schema, refs);
            }
            // Scalar form constrained to a parent block: option = { name = X }.
            KeyForm::ValueUnder(parent) if value.kind == NodeKind::Scalar => {
                if parent_key == parent.as_bytes() {
                    append_ref(tree, rule.kind, rule.alt, value_id, path, schema, refs);
                }
            }
            // Block form carrying a named field: key = { field = value … }.
            KeyForm::BlockField(field) if value.kind == NodeKind::Block => {
                if let Some(id_node) = direct_field_node(tree, value_id, field)
                    && tree.node(id_node).kind == NodeKind::Scalar
                {
                    append_ref(tree, rule.kind, rule.alt, id_node, path, schema, refs);
                }
            }
            // List form: key = { item item … } — loose scalar items.
            KeyForm::List if value.kind == NodeKind::Block => {
                for item in tree.children(value_id) {
                    if tree.node(item).kind == NodeKind::Scalar {
                        append_ref(tree, rule.kind, rule.alt, item, path, schema, refs);
                    }
                }
            }
            // Block-keys form: key = { X = v … } — each field key is a ref.
            KeyForm::BlockKeys if value.kind == NodeKind::Block => {
                for item in tree.children(value_id) {
                    if tree.node(item).kind != NodeKind::Field {
                        continue;
                    }
                    let kids = tree.child_ids(item);
                    if let Some(&key_id) = kids.first()
                        && tree.node(key_id).kind == NodeKind::Scalar
                    {
                        append_ref(tree, rule.kind, rule.alt, key_id, path, schema, refs);
                    }
                }
            }
            // Block-values form: key = { ANY = X ANY = { X Y } … } — each
            // field's scalar value (or block value's loose items) is a ref.
            KeyForm::BlockValues if value.kind == NodeKind::Block => {
                for item in tree.children(value_id) {
                    if tree.node(item).kind != NodeKind::Field {
                        continue;
                    }
                    let kids = tree.child_ids(item);
                    let Some(&val_id) = kids.get(1) else { continue };
                    match tree.node(val_id).kind {
                        NodeKind::Scalar => {
                            append_ref(tree, rule.kind, rule.alt, val_id, path, schema, refs);
                        }
                        NodeKind::Block => {
                            for entry in tree.children(val_id) {
                                if tree.node(entry).kind == NodeKind::Scalar {
                                    append_ref(
                                        tree, rule.kind, rule.alt, entry, path, schema, refs,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Weighted form: key = { WEIGHT = id … } — numeric-keyed entries.
            KeyForm::Weighted if value.kind == NodeKind::Block => {
                extract_weighted_refs(tree, rule.kind, rule.alt, value_id, path, schema, refs);
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
    kind: KindId,
    alt: &'static [KindId],
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
            append_ref(tree, kind, alt, kids[1], path, schema, refs);
        }
    }
}

/// Records a resolvable reference from a scalar value node, applying the
/// quote-strip, macro-concatenation, and scope/macro skips.
fn append_ref(
    tree: &SyntaxTree,
    kind: KindId,
    alt: &'static [KindId],
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
        alt,
        name: val.into_owned(),
        file: Arc::from(path),
        start: value.range.start,
        end: value.range.end,
    });
}

/// The scalar value of a direct-child `key = value` field in `block_id`.
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
