//! Interface-script (`.gui`) analysis — Milestone 1.
//!
//! `.gui` files use the PDXScript token grammar plus a handful of extra
//! productions. Most of them already parse into the shared flat tree via the
//! sibling-scalar idiom (the same shape as `scripted_trigger NAME = { }`):
//!
//! - `template NAME { … }` / `types NAME { … }` → `Scalar(keyword)` +
//!   `TaggedBlock(NAME)` siblings;
//! - `type name = base { … }` → `Scalar("type")` + `Field(name = TaggedBlock(base))`;
//! - `block "name" { … }` → `Scalar(keyword)` + `Scalar("name")` + `Block`.
//!
//! The only construct the script parser rejects — `enabled = [Fn.Chain]` — is
//! handled by [`pdxl_parser::parse_gui`], which this crate uses.
//!
//! **Symbols** (definitions): `template` / `local_template` names and `type`
//! names (including inside `types` groups), kinded via the game-supplied
//! [`GuiKinds`].
//!
//! **References** are *name-gated* (the calls model, never diagnosed): a
//! `using = X` value, a `type x = base` base, or a `name = { … }` widget
//! instantiation is recorded only when `X` names a defined template/type.
//! This is deliberate: `using` can target engine-side jomini/editor templates
//! (`editor_button`, `dockable_background`, ~90 corpus refs) and type bases
//! are usually builtin widgets (`margin_widget`, `dropDown`) — neither set is
//! enumerable from game files, so unresolved names must stay silent. The
//! resolvable remainder measured on vanilla + T4N: ~8.1k `using` refs and
//! ~30.8k instantiations across 389 files.

pub mod datafn;
pub mod docs;
pub mod vocab;
use std::collections::HashSet;
use std::sync::Arc;

use pdxl_analysis::{FileFacts, GuiKinds, Ref, Symbol};
use pdxl_ast::{NodeId, NodeKind, SyntaxTree};
use pdxl_parser::Parse;

/// Parses a `.gui` source with the interface dialect.
pub fn parse(filename: impl Into<Arc<str>>, source: impl Into<Arc<[u8]>>) -> Parse {
    pdxl_parser::parse_gui(filename, source)
}

/// The project-wide defined-name sets gui references are gated on.
/// Built from every `.gui` file's definitions (pass 1), consumed when
/// harvesting references (pass 2) — the same two-pass shape as call-by-name.
#[derive(Debug, Default)]
pub struct GuiNames {
    pub templates: HashSet<String>,
    pub types: HashSet<String>,
}

impl GuiNames {
    /// Accumulates the defined names out of one file's facts.
    pub fn add_facts(&mut self, facts: &FileFacts, kinds: GuiKinds) {
        for def in &facts.defs {
            if def.kind == kinds.template {
                self.templates.insert(def.name.clone());
            } else if def.kind == kinds.ty {
                self.types.insert(def.name.clone());
            }
        }
    }

    /// The kind of a defined name, templates first (they share no namespace
    /// in practice; collisions resolve to the template).
    fn kind_of(&self, name: &str, kinds: GuiKinds) -> Option<pdxl_analysis::KindId> {
        if self.templates.contains(name) {
            Some(kinds.template)
        } else if self.types.contains(name) {
            Some(kinds.ty)
        } else {
            None
        }
    }
}

/// Extracts a `.gui` file's definitions (pass 1). `rel_path` keys the symbol
/// (FileSet overlay key); the tree should come from [`parse`].
pub fn gui_defs(tree: &SyntaxTree, rel_path: &str, kinds: GuiKinds) -> FileFacts {
    let mut facts = FileFacts::default();
    let file: Arc<str> = Arc::from(rel_path);
    collect_defs(tree, tree.root(), kinds, &file, &mut facts);
    facts
}

/// The pending-keyword def walk over one item list (file top level or a
/// `types` group body). Mirrors the script engine's typed-def harvesting.
fn collect_defs(
    tree: &SyntaxTree,
    parent: NodeId,
    kinds: GuiKinds,
    file: &Arc<str>,
    facts: &mut FileFacts,
) {
    #[derive(Clone, Copy, PartialEq)]
    enum Pending {
        None,
        Template,
        Type,
        TypesGroup,
    }
    let mut pending = Pending::None;
    for child in tree.children(parent) {
        let node = tree.node(child);
        match node.kind {
            NodeKind::Scalar => {
                pending = match tree.node_text(child) {
                    b"template" | b"local_template" => Pending::Template,
                    b"type" => Pending::Type,
                    b"types" => Pending::TypesGroup,
                    _ => Pending::None,
                };
            }
            // `template NAME { … }` — the TaggedBlock's range covers NAME.
            NodeKind::TaggedBlock => {
                match pending {
                    Pending::Template => push_def(tree, child, kinds.template, file, facts),
                    // `types NAME { … }` — recurse for the `type` defs inside.
                    Pending::TypesGroup => collect_defs(tree, child, kinds, file, facts),
                    _ => {}
                }
                pending = Pending::None;
            }
            // `type name = base { … }` — a Field whose key is the new name.
            NodeKind::Field => {
                if pending == Pending::Type {
                    push_def(tree, child, kinds.ty, file, facts);
                }
                pending = Pending::None;
            }
            _ => pending = Pending::None,
        }
    }
}

/// Records one definition symbol; `node`'s range covers the name text.
fn push_def(
    tree: &SyntaxTree,
    node: NodeId,
    kind: pdxl_analysis::KindId,
    file: &Arc<str>,
    facts: &mut FileFacts,
) {
    let n = tree.node(node);
    facts.defs.push(Symbol {
        name: String::from_utf8_lossy(tree.node_text(node)).into_owned(),
        kind,
        file: Arc::clone(file),
        offset: n.range.start,
        end_offset: n.range.end,
        params: Vec::new(),
    });
}

/// Extracts a `.gui` file's name-gated references (pass 2): `using = X`,
/// `type x = base` bases, and `name = { … }` instantiations of defined
/// templates/types. `full_path` labels the refs (clickable diagnostics path).
pub fn gui_refs(tree: &SyntaxTree, full_path: &str, names: &GuiNames, kinds: GuiKinds) -> Vec<Ref> {
    let mut refs = Vec::new();
    let file: Arc<str> = Arc::from(full_path);
    walk_refs(tree, tree.root(), true, names, kinds, &file, &mut refs);
    refs
}

/// Walks one item list. `top` marks a definition context (file top level or a
/// `types` group body) where a `type name = base { }` Field's *key* is the
/// definition itself, not an instantiation.
fn walk_refs(
    tree: &SyntaxTree,
    parent: NodeId,
    top: bool,
    names: &GuiNames,
    kinds: GuiKinds,
    file: &Arc<str>,
    refs: &mut Vec<Ref>,
) {
    let mut pending_types_group = false;
    let mut pending_type_def = false;
    for child in tree.children(parent) {
        let node = tree.node(child);
        match node.kind {
            NodeKind::Scalar if top => {
                let text = tree.node_text(child);
                pending_types_group = text == b"types";
                pending_type_def = text == b"type";
                continue;
            }
            // `types NAME { … }` grouping — its body is still definition
            // context; a bare TaggedBlock elsewhere has no ref semantics
            // (template defs are handled as defs, not refs).
            NodeKind::TaggedBlock => {
                let group = top && pending_types_group;
                walk_refs(tree, child, group, names, kinds, file, refs);
            }
            NodeKind::Block => {
                walk_refs(tree, child, false, names, kinds, file, refs);
            }
            NodeKind::Field => {
                let kids = tree.child_ids(child);
                if kids.len() == 2 {
                    let (key_id, val_id) = (kids[0], kids[1]);
                    let key = tree.node_text(key_id);
                    let val = tree.node(val_id);
                    // `using = X` — the value names a template (or type).
                    if key == b"using" && val.kind == NodeKind::Scalar {
                        push_ref(tree, val_id, names, kinds, file, refs);
                    }
                    // `name = { … }` — a widget instantiation when `name` is
                    // a defined template/type. Skipped for the `type x = …`
                    // definition Field itself (its key is the *new* name).
                    let is_type_def = top && pending_type_def;
                    if !is_type_def && matches!(val.kind, NodeKind::Block | NodeKind::TaggedBlock) {
                        push_ref(tree, key_id, names, kinds, file, refs);
                    }
                    // `… = base { … }` — a TaggedBlock value's tag names the
                    // base widget (`type x = base { }` and template-call
                    // shapes alike).
                    if val.kind == NodeKind::TaggedBlock {
                        push_ref(tree, val_id, names, kinds, file, refs);
                    }
                    // Recurse into the value; inside a value we are no longer
                    // in definition context.
                    walk_refs(tree, val_id, false, names, kinds, file, refs);
                }
            }
            _ => {}
        }
        pending_types_group = false;
        pending_type_def = false;
    }
}

/// Records a reference for `node`'s text when it names a defined symbol.
fn push_ref(
    tree: &SyntaxTree,
    node: NodeId,
    names: &GuiNames,
    kinds: GuiKinds,
    file: &Arc<str>,
    refs: &mut Vec<Ref>,
) {
    let text = tree.node_text(node);
    let Ok(name) = std::str::from_utf8(text) else {
        return;
    };
    let Some(kind) = names.kind_of(name, kinds) else {
        return;
    };
    let n = tree.node(node);
    refs.push(Ref {
        kind,
        alt: &[],
        name: name.to_string(),
        file: Arc::clone(file),
        start: n.range.start,
        end: n.range.end,
    });
}
