//! Context-aware completion: what can be written at the cursor.
//!
//! Combines the three knowledge sources the analysis layers provide:
//! - `pdxl_analysis::context::context_at` — is the cursor in an effect
//!   clause, a trigger clause, a struct with known fields, …
//! - the generated doc tables (`pdxl_ck3::tables`) — every built-in effect
//!   and trigger name, with its supported scopes as detail text
//! - the project symbol table — scripted effects/triggers defined by the
//!   mod+game corpus
//!
//! Item ordering (via `sort_text`): struct fields and scripted symbols
//! first (most specific), control keywords next, the big built-in tables
//! last — clients filter as the user types either way.

use lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat};
use pdxl_analysis::context::{ClauseKind, Fallback, StructSpec};
use pdxl_analysis::{IconHint, Schema, SymbolKind, SymbolTable};
use pdxl_ck3::tables::{DocRow, EFFECTS, TRIGGERS};

/// Control keywords legal inside effect clauses (not in the doc tables:
/// they are flow structure, not effects).
const EFFECT_CONTROL: &[&str] = &[
    "if",
    "else_if",
    "else",
    "while",
    "random",
    "random_list",
    "hidden_effect",
    "show_as_tooltip",
    "custom_tooltip",
    "switch",
];

/// Control/combinator keywords legal inside trigger clauses.
const TRIGGER_CONTROL: &[&str] = &[
    "AND",
    "OR",
    "NOT",
    "NOR",
    "NAND",
    "trigger_if",
    "trigger_else_if",
    "trigger_else",
    "custom_description",
    "custom_tooltip",
    "calc_true_if",
];

const SCRIPT_VALUE_KEYS: &[&str] = &[
    "value", "add", "subtract", "multiply", "divide", "modulo", "min", "max", "round", "floor",
    "ceiling", "if", "else_if", "else", "limit", "desc",
];

const SCRIPTED_MODIFIER_KEYS: &[&str] = &["base", "add", "factor", "modifier", "desc"];

const DYNAMIC_DESC_KEYS: &[&str] = &[
    "desc",
    "triggered_desc",
    "first_valid",
    "random_valid",
    "switch",
    "count",
];

/// Completion items for a clause context.
pub fn items_for(ctx: ClauseKind, table: &SymbolTable) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    match ctx {
        ClauseKind::Effect => push_effect_items(&mut items, table),
        ClauseKind::Trigger => push_trigger_items(&mut items, table),
        ClauseKind::ScriptValue => push_keywords(&mut items, SCRIPT_VALUE_KEYS, "script value"),
        ClauseKind::ScriptedModifier => {
            push_keywords(&mut items, SCRIPTED_MODIFIER_KEYS, "scripted modifier");
        }
        ClauseKind::DynamicDesc => {
            push_keywords(&mut items, DYNAMIC_DESC_KEYS, "dynamic description");
        }
        ClauseKind::Struct(spec) => push_struct_items(&mut items, spec, table),
        ClauseKind::Config | ClauseKind::Unknown => {}
    }
    items
}

/// Defined symbols matching a schema reference query, for value completion.
pub fn symbol_value_items<I>(table: &SymbolTable, schema: &Schema, kinds: I) -> Vec<CompletionItem>
where
    I: IntoIterator<Item = SymbolKind>,
{
    let mut items = Vec::new();
    for kind in kinds {
        for name in table.names(kind) {
            let symbol = table.lookup(kind, name).expect("name from symbol table");
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(completion_kind(schema.icon(kind))),
                detail: Some(format!("{} · defined in {}", kind.as_str(), symbol.file)),
                sort_text: Some(format!("0_{name}")),
                ..CompletionItem::default()
            });
        }
    }
    items
}

fn completion_kind(icon: IconHint) -> CompletionItemKind {
    match icon {
        IconHint::Function => CompletionItemKind::FUNCTION,
        IconHint::Event => CompletionItemKind::EVENT,
        IconHint::Tag => CompletionItemKind::ENUM_MEMBER,
        IconHint::Action => CompletionItemKind::METHOD,
        IconHint::Object => CompletionItemKind::CLASS,
        IconHint::Hierarchy => CompletionItemKind::ENUM,
    }
}

/// Completion at file top level (between definitions).
pub fn top_level_items(rel_path: &str) -> Vec<CompletionItem> {
    if !rel_path.starts_with("events/") {
        return Vec::new();
    }
    vec![
        snippet(
            "event",
            "event skeleton (type/title/desc/trigger/immediate/option)",
            "${1:namespace}.${2:0001} = {\n\
             \ttype = character_event\n\
             \ttitle = ${1:namespace}.${2:0001}.t\n\
             \tdesc = ${1:namespace}.${2:0001}.desc\n\
             \ttheme = ${3:default}\n\
             \tleft_portrait = ${4:root}\n\
             \n\
             \ttrigger = {\n\t\t$5\n\t}\n\
             \n\
             \timmediate = {\n\t\t$6\n\t}\n\
             \n\
             \toption = {\n\t\tname = ${1:namespace}.${2:0001}.a\n\t\t$0\n\t}\n\
             }",
        ),
        snippet("namespace", "event namespace declaration", "namespace = $0"),
    ]
}

fn push_struct_items(
    items: &mut Vec<CompletionItem>,
    spec: &'static StructSpec,
    table: &SymbolTable,
) {
    for (key, field) in spec.fields {
        // The insert form follows the value forms the field accepts; the
        // richest field (option) gets a fuller body.
        let insert = match (*key, field.block.is_some(), field.scalar.is_some()) {
            ("option", ..) => format!("{key} = {{\n\tname = $1\n\t$0\n}}"),
            (_, true, false) => format!("{key} = {{\n\t$0\n}}"),
            _ => format!("{key} = $0"),
        };
        items.push(CompletionItem {
            label: (*key).to_string(),
            kind: Some(CompletionItemKind::PROPERTY),
            detail: Some(format!("{} field", spec.name)),
            insert_text: Some(insert),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            sort_text: Some(format!("0_{key}")),
            ..CompletionItem::default()
        });
    }
    // The mixed-context rule: where unknown keys are inline effects or
    // triggers, offer those names too.
    match spec.fallback {
        Fallback::Effect => push_effect_items(items, table),
        Fallback::Trigger => push_trigger_items(items, table),
        Fallback::Ignore | Fallback::Deny => {}
    }
}

fn push_effect_items(items: &mut Vec<CompletionItem>, table: &SymbolTable) {
    push_scripted(items, table, SymbolKind::ScriptedEffect, "scripted effect");
    push_keywords(items, EFFECT_CONTROL, "effect control");
    push_doc_rows(items, EFFECTS, "effect");
}

fn push_trigger_items(items: &mut Vec<CompletionItem>, table: &SymbolTable) {
    push_scripted(
        items,
        table,
        SymbolKind::ScriptedTrigger,
        "scripted trigger",
    );
    push_keywords(items, TRIGGER_CONTROL, "trigger control");
    push_doc_rows(items, TRIGGERS, "trigger");
}

fn push_scripted(
    items: &mut Vec<CompletionItem>,
    table: &SymbolTable,
    kind: SymbolKind,
    detail: &str,
) {
    for name in table.names(kind) {
        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(detail.to_string()),
            sort_text: Some(format!("1_{name}")),
            ..CompletionItem::default()
        });
    }
}

fn push_keywords(items: &mut Vec<CompletionItem>, keys: &[&str], what: &str) {
    for key in keys {
        items.push(CompletionItem {
            label: (*key).to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(what.to_string()),
            sort_text: Some(format!("2_{key}")),
            ..CompletionItem::default()
        });
    }
}

fn push_doc_rows(items: &mut Vec<CompletionItem>, rows: &[DocRow], what: &str) {
    for row in rows {
        items.push(CompletionItem {
            label: row.name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!("{what} · scopes: {}", row.scopes.join(", "))),
            sort_text: Some(format!("3_{}", row.name)),
            ..CompletionItem::default()
        });
    }
}

fn snippet(label: &str, detail: &str, body: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some(detail.to_string()),
        insert_text: Some(body.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        sort_text: Some(format!("0_{label}")),
        ..CompletionItem::default()
    }
}
