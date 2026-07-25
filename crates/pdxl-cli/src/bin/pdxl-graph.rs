//! Schema x-ray: parses PDXScript files and prints one line per node with
//! everything the compiled-in schema concluded about it — the clause context,
//! any definition/reference/alias/constant extracted at that position (with
//! its kind), or the explicit absence of a rule. Ends the guesswork about
//! whether `tag = GEN` inside `maona.visible_through_diplomacy` references a
//! country: the line says so, either way.
//!
//! Usage (the game feature picks the schema, like every pdxl binary):
//!
//!   cargo run -p pdxl-cli --features eu5 --bin pdxl-graph -- \
//!     <file.txt> [more files…] [--rel <overlay path>] [--game <game dir>]
//!
//! `--rel` overrides the overlay-relative path used for directory-gated
//! rules; without it, the path is derived by searching the absolute path for
//! a known game root (`common/`, `events/`, `history/`, `in_game/`,
//! `main_menu/`, `setup/`). `--game` additionally builds the full project so
//! every reference is marked resolved (`✓`) or unresolved (`✗`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

use pdxl_analysis::context::{ClauseKind, context_of_chain};
use pdxl_analysis::{FileFacts, KindId, Schema, SymbolTable, extract_facts};
use pdxl_ast::{NodeId, NodeKind, SyntaxTree};

fn main() -> ExitCode {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut rel_override: Option<String> = None;
    let mut game: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rel" => match args.next() {
                Some(v) => rel_override = Some(v),
                None => return usage("missing value for --rel"),
            },
            "--game" => match args.next() {
                Some(v) => game = Some(v),
                None => return usage("missing value for --game"),
            },
            flag if flag.starts_with("--") => return usage(&format!("unknown flag: {flag}")),
            file => files.push(PathBuf::from(file)),
        }
    }
    if files.is_empty() {
        return usage("no input files");
    }

    let schema = pdxl_game::schema();
    // With --game, resolve refs against the whole project's symbol table.
    let table = game.map(|dir| {
        eprintln!("[{}] building project from {dir}…", pdxl_game::GAME);
        let mut fs = pdxl_fileset::FileSet::new();
        fs.add(&dir, pdxl_fileset::FileKind::Vanilla)
            .and_then(|()| pdxl_project::analyze(&fs, &schema))
            .map(|(table, _)| table)
            .unwrap_or_else(|e| {
                eprintln!("pdxl-graph: --game: {e}");
                std::process::exit(1);
            })
    });

    for file in &files {
        if files.len() > 1 {
            println!("# {}", file.display());
        }
        if let Err(e) = graph_file(file, rel_override.as_deref(), &schema, table.as_ref()) {
            eprintln!("pdxl-graph: {}: {e}", file.display());
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

fn usage(msg: &str) -> ExitCode {
    eprintln!("pdxl-graph: {msg}");
    eprintln!("usage: pdxl-graph <file.txt>… [--rel <overlay path>] [--game <game dir>]");
    ExitCode::from(2)
}

/// Derives the overlay-relative path (drives gated rules) from an absolute
/// path by cutting at the last known game-root component.
fn derive_rel(path: &std::path::Path) -> String {
    let full = path.to_string_lossy().replace('\\', "/");
    for root in [
        "in_game/",
        "main_menu/",
        "common/",
        "events/",
        "history/",
        "setup/",
    ] {
        if let Some(idx) = full.rfind(&format!("/{root}")) {
            return full[idx + 1..].to_lowercase();
        }
    }
    // No known root: bare file name (ungated rules still apply).
    path.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// A classification extracted at a byte span.
enum Mark {
    Def(KindId),
    Alias(KindId),
    Ref {
        kind: KindId,
        alt: &'static [KindId],
    },
    ConstDef,
    ConstRef,
}

fn graph_file(
    path: &std::path::Path,
    rel_override: Option<&str>,
    schema: &Schema,
    table: Option<&SymbolTable>,
) -> std::io::Result<()> {
    let rel = rel_override
        .map(str::to_string)
        .unwrap_or_else(|| derive_rel(path));
    println!("# rel: {rel}   (schema: {})", pdxl_game::GAME);

    let src = std::fs::read(path)?;
    let (tree, _) = pdxl_parser::parse(path.to_string_lossy().into_owned(), src).into_parts();
    let facts = extract_facts(&tree, &rel, &rel, schema, None);

    // Span-keyed classification maps. Defs anchor at the def's start offset
    // (their span covers `offset..end_offset` of the name); refs at start.
    let mut marks: HashMap<u32, Mark> = HashMap::new();
    for d in &facts.defs {
        marks.insert(d.offset, Mark::Def(d.kind));
    }
    for a in &facts.aliases {
        // Alias markers share the def's start; only add where nothing is.
        marks.entry(a.offset).or_insert(Mark::Alias(a.kind));
    }
    for r in &facts.refs {
        marks.insert(
            r.start,
            Mark::Ref {
                kind: r.kind,
                alt: r.alt,
            },
        );
    }
    for c in &facts.constants {
        marks.insert(c.offset, Mark::ConstDef);
    }
    for r in &facts.constant_refs {
        marks.insert(r.start, Mark::ConstRef);
    }

    let mut chain: Vec<Vec<u8>> = Vec::new();
    for item in tree.children(tree.root()) {
        walk(&tree, item, &mut chain, &rel, schema, &facts, &marks, table);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn walk(
    tree: &SyntaxTree,
    node_id: NodeId,
    chain: &mut Vec<Vec<u8>>,
    rel: &str,
    schema: &Schema,
    facts: &FileFacts,
    marks: &HashMap<u32, Mark>,
    table: Option<&SymbolTable>,
) {
    let node = tree.node(node_id);
    match node.kind {
        NodeKind::Field => {
            let kids = tree.child_ids(node_id);
            if kids.len() != 2 {
                return;
            }
            let key = tree.node_text(kids[0]).to_vec();
            let value_node = tree.node(kids[1]);
            let path = render_path(chain, Some(&key));
            match value_node.kind {
                NodeKind::Scalar => {
                    let value = String::from_utf8_lossy(tree.node_text(kids[1]));
                    let note = classify_scalar_field(
                        tree, node_id, kids[1], chain, &key, rel, schema, facts, marks, table,
                    );
                    let lhs = format!("{path} = {value}");
                    println!("{lhs:<56}  {note}");
                }
                NodeKind::Block | NodeKind::TaggedBlock => {
                    chain.push(key.clone());
                    let ctx = context_of_chain(
                        chain.iter().map(|k| k.as_slice()),
                        rel,
                        pdxl_game::contexts::context_schema(),
                    );
                    let def_note = marks
                        .get(&node.range.start)
                        .map(|m| format!("  {}", render_mark(m, table, "")))
                        .unwrap_or_default();
                    let lhs = format!("{path} = {{…}}");
                    println!("{lhs:<56}  ctx: {}{def_note}", ctx_name(ctx));
                    for child in tree.children(kids[1]) {
                        walk(tree, child, chain, rel, schema, facts, marks, table);
                    }
                    chain.pop();
                }
                _ => {}
            }
        }
        NodeKind::Scalar => {
            // A loose list item.
            let value = String::from_utf8_lossy(tree.node_text(node_id));
            let note = match marks.get(&node.range.start) {
                Some(m) => render_mark(m, table, &value),
                None => "· list item (no rule)".to_string(),
            };
            println!("{} [{value}]  {note}", render_path(chain, None));
        }
        _ => {
            for child in tree.children(node_id) {
                walk(tree, child, chain, rel, schema, facts, marks, table);
            }
        }
    }
}

/// The classification note for a `key = scalar` field: what the extractor
/// recorded at the value (or key) span, or the explicit absence of a rule.
#[allow(clippy::too_many_arguments)]
fn classify_scalar_field(
    tree: &SyntaxTree,
    field_id: NodeId,
    value_id: NodeId,
    chain: &[Vec<u8>],
    key: &[u8],
    rel: &str,
    schema: &Schema,
    _facts: &FileFacts,
    marks: &HashMap<u32, Mark>,
    table: Option<&SymbolTable>,
) -> String {
    let field_start = tree.node(field_id).range.start;
    let value = tree.node(value_id);
    let value_text = String::from_utf8_lossy(tree.node_text(value_id));

    // Definitions anchor at the field start (keyed-value defs at the value).
    if let Some(m) = marks
        .get(&field_start)
        .or_else(|| marks.get(&value.range.start))
    {
        return render_mark(m, table, &value_text);
    }
    // A scope-literal ref inside the value anchors mid-span (`c:GEN` → GEN).
    for off in value.range.start..value.range.end {
        if let Some(m @ Mark::Ref { .. }) = marks.get(&off) {
            return render_mark(m, table, &value_text);
        }
    }
    // Nothing extracted: say what the schema knows structurally.
    if schema.skip_ref_value(&value_text) {
        return "· skipped value (scope keyword / chain / macro)".to_string();
    }
    let key_str = String::from_utf8_lossy(key);
    let kinds: Vec<&str> = schema
        .value_kinds(&key_str, rel)
        .map(|k| k.name())
        .collect();
    if !kinds.is_empty() {
        // A rule exists for this key but did not fire here (e.g. wrong depth).
        return format!(
            "· rule for `{key_str}` exists ({}) but did not fire here",
            kinds.join("|")
        );
    }
    let ctx = context_of_chain(
        chain.iter().map(|k| k.as_slice()),
        rel,
        pdxl_game::contexts::context_schema(),
    );
    format!("· no rule ({} field)", ctx_name(ctx))
}

fn render_mark(mark: &Mark, table: Option<&SymbolTable>, value: &str) -> String {
    match mark {
        Mark::Def(kind) => format!("DEF {}", kind.name()),
        Mark::Alias(kind) => format!("ALIAS of {}", kind.name()),
        Mark::Ref { kind, alt } => {
            let mut s = format!("REF {}", kind.name());
            if !alt.is_empty() {
                let alts: Vec<&str> = alt.iter().map(|k| k.name()).collect();
                s.push_str(&format!("|{}", alts.join("|")));
            }
            if let Some(table) = table {
                let name = value.split(':').next_back().unwrap_or(value);
                let resolved = std::iter::once(*kind)
                    .chain(alt.iter().copied())
                    .any(|k| table.lookup(k, name).is_some());
                s.push_str(if resolved {
                    "  ✓ resolved"
                } else {
                    "  ✗ UNRESOLVED"
                });
            }
            s
        }
        Mark::ConstDef => "DEF script_constant".to_string(),
        Mark::ConstRef => "REF script_constant (file-local)".to_string(),
    }
}

fn render_path(chain: &[Vec<u8>], leaf: Option<&[u8]>) -> String {
    let mut parts: Vec<String> = chain
        .iter()
        .map(|k| String::from_utf8_lossy(k).into_owned())
        .collect();
    if let Some(leaf) = leaf {
        parts.push(String::from_utf8_lossy(leaf).into_owned());
    }
    parts.join(".")
}

fn ctx_name(ctx: ClauseKind) -> String {
    match ctx {
        ClauseKind::Effect => "Effect".to_string(),
        ClauseKind::Trigger => "Trigger".to_string(),
        ClauseKind::ScriptValue => "ScriptValue".to_string(),
        ClauseKind::ScriptedModifier => "ScriptedModifier".to_string(),
        ClauseKind::StaticModifier => "StaticModifier".to_string(),
        ClauseKind::DynamicDesc => "DynamicDesc".to_string(),
        ClauseKind::Color => "Color".to_string(),
        ClauseKind::Struct(spec) => format!("Struct({})", spec.name),
        ClauseKind::Config => "Config".to_string(),
        ClauseKind::Unknown => "Unknown".to_string(),
    }
}
