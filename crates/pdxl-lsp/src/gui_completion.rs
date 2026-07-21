//! Completion for interface scripts (`.gui`) — Milestone 3.
//!
//! Three contexts, detected textually (robust to the half-typed input
//! completion always runs on):
//!
//! 1. **Datafunction** — the cursor is inside an unclosed `[…]`. At the chain
//!    root, offer global promotes/functions and registered type names; after
//!    a `.`, resolve the receiver chain and offer that type's members. Both
//!    come from the `DumpDataTypes` registry.
//! 2. **Value** — the cursor follows `key = `. `using` offers the defined
//!    templates/types; other keys offer their corpus-mined value vocabulary
//!    (`parentanchor` → `center`, `top|right`, …).
//! 3. **Key** — otherwise. The enclosing widget is recovered from a
//!    token-stream brace scan (`icon = {` pushes `icon`; `= widget {` pushes
//!    `widget`), and its corpus-mined property keys are offered most-frequent
//!    first, plus the dialect keywords and defined template/type names for
//!    instantiation.

use lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, MarkupContent, MarkupKind,
};
use pdxl_analysis::GuiKinds;
use pdxl_gui::datafn::{DataFnKind, DataFnRegistry};
use pdxl_lexer::TokenKind as T;
use pdxl_project::Project;

/// The identifier-prefix immediately before `off` (what the user has typed).
fn typed_prefix(src: &[u8], off: usize) -> &[u8] {
    let mut start = off.min(src.len());
    while start > 0 && (src[start - 1].is_ascii_alphanumeric() || src[start - 1] == b'_') {
        start -= 1;
    }
    &src[start..off.min(src.len())]
}

/// If the cursor sits inside an unclosed `[…]`, the expression text from `[`
/// to the cursor.
fn datafn_prefix(src: &[u8], off: usize) -> Option<&[u8]> {
    let upto = &src[..off.min(src.len())];
    let lb = upto.iter().rposition(|&b| b == b'[')?;
    if upto[lb..].contains(&b']') || upto[lb..].contains(&b'\n') {
        return None;
    }
    Some(&upto[lb + 1..])
}

/// If the cursor is in value position, the key it belongs to.
fn value_key(src: &[u8], off: usize) -> Option<String> {
    let mut i = off.min(src.len());
    // Skip the typed value prefix.
    while i > 0 && (src[i - 1].is_ascii_alphanumeric() || src[i - 1] == b'_') {
        i -= 1;
    }
    while i > 0 && (src[i - 1] == b' ' || src[i - 1] == b'\t') {
        i -= 1;
    }
    if i == 0 || src[i - 1] != b'=' {
        return None;
    }
    i -= 1;
    while i > 0 && (src[i - 1] == b' ' || src[i - 1] == b'\t') {
        i -= 1;
    }
    let end = i;
    while i > 0 && (src[i - 1].is_ascii_alphanumeric() || src[i - 1] == b'_') {
        i -= 1;
    }
    (i < end).then(|| String::from_utf8_lossy(&src[i..end]).into_owned())
}

/// The enclosing widget/property name at `off`, from a token-stream brace
/// scan: `key = {` pushes `key`, `key = base {` pushes `base`,
/// `template NAME {` pushes `NAME`; a bare `{` pushes an unknown frame.
fn owner_at(src: &[u8], off: u32) -> Option<String> {
    let toks = pdxl_lexer::tokenize(src);
    let mut stack: Vec<Option<String>> = Vec::new();
    let text_of = |t: &pdxl_lexer::Token| {
        String::from_utf8_lossy(&src[t.range.start as usize..t.range.end as usize]).into_owned()
    };
    let mut i = 0;
    let toks: Vec<&pdxl_lexer::Token> = toks.iter().filter(|t| t.kind != T::Comment).collect();
    while i < toks.len() {
        let t = toks[i];
        if t.range.start >= off {
            break;
        }
        match t.kind {
            T::LBrace => {
                // The owner is the nearest preceding identifier on the same
                // item: `ident {` (tag) or `ident = {`.
                let owner = match (i.checked_sub(1).map(|j| toks[j]), i.checked_sub(2)) {
                    (Some(prev), _) if prev.kind == T::Identifier => Some(text_of(prev)),
                    (Some(prev), Some(j2))
                        if prev.kind == T::Equal && toks[j2].kind == T::Identifier =>
                    {
                        Some(text_of(toks[j2]))
                    }
                    _ => None,
                };
                stack.push(owner);
            }
            T::RBrace => {
                stack.pop();
            }
            _ => {}
        }
        i += 1;
    }
    stack.last().cloned().flatten()
}

fn snippet(label: &str, detail: &str, insert: String, kind: CompletionItemKind) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: Some(detail.to_string()),
        insert_text: Some(insert),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..CompletionItem::default()
    }
}

/// Completion items for a `.gui` file at byte offset `off`.
pub fn items(project: &Project, src: &[u8], off: u32) -> Vec<CompletionItem> {
    let Some(kinds) = project.schema().gui_kinds() else {
        return Vec::new();
    };
    let registry = pdxl_ck3::datafn_registry();

    // 1. Datafunction chains.
    if let Some(prefix) = datafn_prefix(src, off as usize) {
        return datafn_items(prefix, src, off as usize, registry);
    }

    // 2. Value position.
    if let Some(key) = value_key(src, off as usize) {
        let mut items = Vec::new();
        if key == "using" {
            for name in project.table().names(kinds.template) {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some("gui template".to_string()),
                    ..CompletionItem::default()
                });
            }
            return items;
        }
        if let Some(vocab) = project.gui_vocab() {
            for (value, n) in vocab.values_for(&key) {
                items.push(CompletionItem {
                    label: value.to_string(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    detail: Some(format!("{key} value · {n}× in corpus")),
                    sort_text: Some(format!("{:08}_{value}", u32::MAX - n)),
                    ..CompletionItem::default()
                });
            }
        }
        return items;
    }

    // 3. Key position: the enclosing widget's mined properties.
    let mut items = Vec::new();
    if let (Some(vocab), Some(owner)) = (project.gui_vocab(), owner_at(src, off)) {
        for (key, n) in vocab.keys_for(&owner) {
            items.push(CompletionItem {
                label: key.to_string(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some(format!("{owner} property · {n}× in corpus")),
                documentation: pdxl_gui::docs::property_doc(key).map(|doc| {
                    Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: doc.to_string(),
                    })
                }),
                insert_text: Some(format!("{key} = $0")),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some(format!("0_{:08}_{key}", u32::MAX - n)),
                ..CompletionItem::default()
            });
        }
    }
    // Dialect structure.
    items.push(snippet(
        "block",
        "named override point",
        "block \"$1\" {\n\t$0\n}".to_string(),
        CompletionItemKind::KEYWORD,
    ));
    items.push(snippet(
        "blockoverride",
        "override a template's block",
        "blockoverride \"$1\" {\n\t$0\n}".to_string(),
        CompletionItemKind::KEYWORD,
    ));
    items.push(snippet(
        "using",
        "apply a template",
        "using = $0".to_string(),
        CompletionItemKind::KEYWORD,
    ));
    // Defined templates/types — instantiation (`my_marker = { … }`).
    push_defined(&mut items, project, kinds);
    items
}

/// Defined template/type names as instantiation snippets, sorted after keys.
fn push_defined(items: &mut Vec<CompletionItem>, project: &Project, kinds: GuiKinds) {
    for (kind, detail) in [(kinds.ty, "gui type"), (kinds.template, "gui template")] {
        for name in project.table().names(kind) {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(detail.to_string()),
                insert_text: Some(format!("{name} = {{\n\t$0\n}}")),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                sort_text: Some(format!("2_{name}")),
                ..CompletionItem::default()
            });
        }
    }
}

/// Datafunction completion: members after `.`, roots otherwise.
fn datafn_items(
    prefix: &[u8],
    src: &[u8],
    off: usize,
    registry: &DataFnRegistry,
) -> Vec<CompletionItem> {
    let typed = typed_prefix(src, off);
    // The chain up to (excluding) the segment being typed.
    let chain_end = prefix.len() - typed.len();
    let chain = &prefix[..chain_end];
    let after_dot = chain.ends_with(b".");

    let mut items = Vec::new();
    let mut push_row = |row: &'static pdxl_gui::datafn::DataFnRow| {
        let kind = match row.kind {
            DataFnKind::Promote | DataFnKind::GlobalPromote => CompletionItemKind::FIELD,
            _ => CompletionItemKind::FUNCTION,
        };
        let args = if row.args > 0 {
            let names: Vec<String> = (0..row.args).map(|i| format!("Arg{i}")).collect();
            format!("( {} )", names.join(", "))
        } else {
            String::new()
        };
        items.push(CompletionItem {
            label: row.name.to_string(),
            kind: Some(kind),
            detail: Some(format!("{args} → {}", row.ret)),
            documentation: (!row.desc.is_empty()).then(|| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: row.desc.to_string(),
                })
            }),
            ..CompletionItem::default()
        });
    };

    if after_dot {
        // Resolve the receiver chain (drop the trailing dot).
        let Some(segments) = pdxl_gui::datafn::parse_chain(&chain[..chain.len() - 1], 0) else {
            return items;
        };
        let (resolved, err) = pdxl_gui::datafn::resolve_chain(&segments, registry);
        if err.is_some() {
            return items;
        }
        // The receiver type: the last segment's return, or the type name
        // itself for a datacontext root.
        let receiver = match resolved.last() {
            Some(info) => match info.row {
                Some(row) => row.ret.to_string(),
                None => segments.last().map(|s| s.name.clone()).unwrap_or_default(),
            },
            None => return items,
        };
        for row in registry.members_of(&receiver) {
            push_row(row);
        }
    } else if chain.is_empty() {
        // Chain root: globals plus type names (datacontext access). Filter by
        // the typed prefix server-side — the full set is ~5k items.
        let typed_str = String::from_utf8_lossy(typed).into_owned();
        for row in registry.globals_iter() {
            if row.name.starts_with(typed_str.as_str()) {
                push_row(row);
            }
        }
        for name in registry.type_names() {
            if name.starts_with(typed_str.as_str()) {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some("data type (datacontext access)".to_string()),
                    ..CompletionItem::default()
                });
            }
        }
    }
    items
}
