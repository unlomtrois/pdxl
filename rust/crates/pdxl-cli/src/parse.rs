//! `pdxl parse` — print the AST, mirroring Go's `cmd/pdxl/parse.go`:
//! the default flat printer (tab-indented source-like rendering) and the
//! `--tree` labelled node tree with box-drawing characters. Parse diagnostics
//! go to stderr as `file:line:col: message`, exactly like `Diagnostic.String()`.

use std::io::{self, Write};
use std::process::ExitCode;

use pdxl_parser::{NodeId, NodeKind, SyntaxTree, parse};

pub fn run(file: &str, tree_mode: bool) -> io::Result<ExitCode> {
    let data = std::fs::read(file)
        .map_err(|e| io::Error::new(e.kind(), format!("reading {file}: {e}")))?;
    let parsed = parse(file.to_string(), data.clone());
    for d in parsed.diagnostics() {
        let (line, col) = pdxl_src::line_col(&data, d.offset);
        eprintln!("{}:{line}:{col}: {}", d.filename, d.message);
    }

    let stdout = io::stdout();
    let mut w = io::BufWriter::new(stdout.lock());
    let tree = parsed.tree();
    if tree_mode {
        print_node_tree(&mut w, tree)?;
    } else {
        for child in tree.children(tree.root()) {
            print_flat_node(&mut w, tree, child, 0)?;
        }
    }
    w.flush()?;
    Ok(ExitCode::SUCCESS)
}

fn text<'t>(tree: &'t SyntaxTree, id: NodeId) -> std::borrow::Cow<'t, str> {
    String::from_utf8_lossy(tree.node_text(id))
}

// ── flat printer (default) — Go printFlat/printFlatNode ─────────────────────

fn print_flat_node(
    w: &mut impl Write,
    tree: &SyntaxTree,
    id: NodeId,
    depth: usize,
) -> io::Result<()> {
    let ind = "\t".repeat(depth);
    let node = tree.node(id);
    match node.kind {
        NodeKind::Field => {
            let kids = tree.child_ids(id);
            let key = text(tree, kids[0]);
            let op = node.op_string();
            let val = kids[1];
            match tree.node(val).kind {
                NodeKind::Scalar => {
                    writeln!(w, "{ind}{key} {op} {}", text(tree, val))?;
                }
                NodeKind::TaggedBlock => {
                    writeln!(w, "{ind}{key} {op} {} {{", text(tree, val))?;
                    for item in tree.children(val) {
                        print_flat_node(w, tree, item, depth + 1)?;
                    }
                    writeln!(w, "{ind}}}")?;
                }
                NodeKind::Block => {
                    writeln!(w, "{ind}{key} {op} {{")?;
                    for item in tree.children(val) {
                        print_flat_node(w, tree, item, depth + 1)?;
                    }
                    writeln!(w, "{ind}}}")?;
                }
                _ => {}
            }
        }
        NodeKind::Scalar => writeln!(w, "{ind}{}", text(tree, id))?,
        NodeKind::Block => {
            writeln!(w, "{ind}{{")?;
            for item in tree.children(id) {
                print_flat_node(w, tree, item, depth + 1)?;
            }
            writeln!(w, "{ind}}}")?;
        }
        _ => {}
    }
    Ok(())
}

// ── tree printer (--tree) — Go printNodeTree/printTreeNode ──────────────────

fn print_node_tree(w: &mut impl Write, tree: &SyntaxTree) -> io::Result<()> {
    writeln!(w, "Root (KindFile)")?;
    let refs = tree.child_ids(tree.root());
    for (i, id) in refs.iter().enumerate() {
        print_tree_node(w, tree, *id, "", i == refs.len() - 1)?;
    }
    Ok(())
}

fn print_tree_node(
    w: &mut impl Write,
    tree: &SyntaxTree,
    id: NodeId,
    prefix: &str,
    last: bool,
) -> io::Result<()> {
    let (branch, child_prefix) = if last {
        ("└── ", format!("{prefix}    "))
    } else {
        ("├── ", format!("{prefix}│   "))
    };

    let node = tree.node(id);
    match node.kind {
        NodeKind::Scalar => {
            writeln!(w, "{prefix}{branch}KindScalar  {:?}", text(tree, id))?;
        }
        NodeKind::Field => {
            let kids = tree.child_ids(id);
            writeln!(
                w,
                "{prefix}{branch}KindField   key={:?}  op={:?}",
                text(tree, kids[0]),
                node.op_string()
            )?;
            print_tree_node(w, tree, kids[1], &child_prefix, true)?;
        }
        NodeKind::Block => {
            writeln!(w, "{prefix}{branch}KindBlock")?;
            let refs = tree.child_ids(id);
            for (i, kid) in refs.iter().enumerate() {
                print_tree_node(w, tree, *kid, &child_prefix, i == refs.len() - 1)?;
            }
        }
        NodeKind::TaggedBlock => {
            writeln!(
                w,
                "{prefix}{branch}KindTaggedBlock  tag={:?}",
                text(tree, id)
            )?;
            let refs = tree.child_ids(id);
            for (i, kid) in refs.iter().enumerate() {
                print_tree_node(w, tree, *kid, &child_prefix, i == refs.len() - 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
