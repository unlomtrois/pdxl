//! Unit tests ported from `internal/parser/v3/parser_test.go`, plus the focused
//! coverage required by the milestone. Intent is preserved over test names.

use super::*;

/// Parses `src`, asserting no diagnostics, and returns the parse.
fn parse_ok(src: &str) -> Parse {
    let parse = parse("test", src.as_bytes().to_vec());
    assert!(
        parse.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
    parse
}

/// The immediate children of a node as a `Vec<NodeId>`.
fn kids(tree: &SyntaxTree, id: NodeId) -> Vec<NodeId> {
    tree.children(id).collect()
}

/// The single top-level item, asserting there is exactly one.
fn only_item(tree: &SyntaxTree) -> NodeId {
    let items = kids(tree, tree.root());
    assert_eq!(items.len(), 1, "expected 1 top-level item");
    items[0]
}

fn text(tree: &SyntaxTree, id: NodeId) -> &str {
    std::str::from_utf8(tree.node_text(id)).unwrap()
}

#[test]
fn simple_field() {
    let p = parse_ok("key = value");
    let tree = p.tree();
    let field = only_item(tree);
    assert_eq!(tree.node(field).kind, NodeKind::Field);
    assert_eq!(tree.node(field).op_string(), "=");
    let fc = kids(tree, field);
    assert_eq!(fc.len(), 2);
    assert_eq!(text(tree, fc[0]), "key");
    assert_eq!(tree.node(fc[1]).kind, NodeKind::Scalar);
    assert_eq!(text(tree, fc[1]), "value");
    // The validator must accept a valid tree.
    validate_tree(tree).unwrap();
}

#[test]
fn block_field() {
    let p = parse_ok("limit = { age > 18 }");
    let tree = p.tree();
    let field = only_item(tree);
    let val = kids(tree, field)[1];
    assert_eq!(tree.node(val).kind, NodeKind::Block);
    let block_children = kids(tree, val);
    assert_eq!(block_children.len(), 1);
    let inner = block_children[0];
    assert_eq!(tree.node(inner).kind, NodeKind::Field);
    assert_eq!(text(tree, inner), "age");
}

#[test]
fn value_list_block() {
    let p = parse_ok("color = { 255 255 255 }");
    let tree = p.tree();
    let field = only_item(tree);
    let val = kids(tree, field)[1];
    assert_eq!(tree.node(val).kind, NodeKind::Block);
    assert_eq!(kids(tree, val).len(), 3);
}

#[test]
fn tagged_block() {
    let p = parse_ok("color = rgb { 218 215 56 }");
    let tree = p.tree();
    let field = only_item(tree);
    let val = kids(tree, field)[1];
    assert_eq!(tree.node(val).kind, NodeKind::TaggedBlock);
    assert_eq!(text(tree, val), "rgb");
    assert_eq!(kids(tree, val).len(), 3);
}

#[test]
fn scope_key_with_operator() {
    let p = parse_ok("scope:actor ?= { is_subject = yes }");
    let tree = p.tree();
    let field = only_item(tree);
    assert_eq!(tree.node(field).kind, NodeKind::Field);
    assert_eq!(text(tree, field), "scope:actor");
    assert_eq!(tree.node(field).op_string(), "?=");
}

#[test]
fn negative_number() {
    let p = parse_ok("modifier = -0.25");
    let tree = p.tree();
    let field = only_item(tree);
    let val = kids(tree, field)[1];
    assert_eq!(text(tree, val), "-0.25");
}

#[test]
fn scope_chain_value() {
    let p = parse_ok("target = define:NMapColors|CONSTANT");
    let tree = p.tree();
    let field = only_item(tree);
    let val = kids(tree, field)[1];
    assert_eq!(text(tree, val), "define:NMapColors|CONSTANT");
}

#[test]
fn comparator_as_value() {
    for cmp in ["<=", ">=", "==", "!=", "<", ">"] {
        let src = format!("OPERATOR = {cmp}");
        let p = parse("test", src.into_bytes());
        assert!(
            p.diagnostics().is_empty(),
            "{cmp}: unexpected diagnostics: {:?}",
            p.diagnostics()
        );
        let tree = p.tree();
        let field = only_item(tree);
        assert_eq!(tree.node(field).kind, NodeKind::Field);
        let val = kids(tree, field)[1];
        assert_eq!(text(tree, val), cmp);
    }
}

#[test]
fn double_comparator_still_errors() {
    let p = parse("test", &b"x >= <="[..]);
    assert!(
        !p.diagnostics().is_empty(),
        "expected a diagnostic for double comparator"
    );
}

#[test]
fn negative_date_key() {
    let p = parse_ok("-221.1.1 = { holder = 100 }");
    let tree = p.tree();
    let field = only_item(tree);
    assert_eq!(tree.node(field).kind, NodeKind::Field);
    assert_eq!(text(tree, field), "-221.1.1");
}

#[test]
fn slash_path_value() {
    let p = parse_ok("reference = event:/SFX/Events/Themes/generic");
    let tree = p.tree();
    let field = only_item(tree);
    let val = kids(tree, field)[1];
    assert_eq!(text(tree, val), "event:/SFX/Events/Themes/generic");
}

#[test]
fn script_value_definition() {
    let p = parse_ok("@my_const = 0.15");
    let tree = p.tree();
    let field = only_item(tree);
    assert_eq!(tree.node(field).kind, NodeKind::Field);
    assert_eq!(text(tree, field), "@my_const");
}

#[test]
fn script_value_reference() {
    let p = parse_ok("key = @my_const");
    let tree = p.tree();
    let field = only_item(tree);
    let val = kids(tree, field)[1];
    assert_eq!(text(tree, val), "@my_const");
}

#[test]
fn inline_math_value() {
    let p = parse_ok("key = @[ my_const * -1 ]");
    let tree = p.tree();
    let field = only_item(tree);
    let val = kids(tree, field)[1];
    assert_eq!(text(tree, val), "@[ my_const * -1 ]");
}

#[test]
fn inline_math_as_key_is_structurally_valid() {
    let p = parse_ok("@[a * 2] = value");
    let tree = p.tree();
    let field = only_item(tree);
    assert_eq!(tree.node(field).kind, NodeKind::Field);
    assert_eq!(text(tree, field), "@[a * 2]");
}

#[test]
fn macro_param_as_value() {
    let p = parse_ok("exists = $CHILD$");
    let tree = p.tree();
    let field = only_item(tree);
    let val = kids(tree, field)[1];
    assert_eq!(text(tree, val), "$CHILD$");
}

#[test]
fn macro_param_as_key() {
    let p = parse_ok("$CHILD$ = { a = b }");
    let tree = p.tree();
    let field = only_item(tree);
    assert_eq!(text(tree, field), "$CHILD$");
}

#[test]
fn macro_param_scope_chain() {
    let p = parse_ok("$CHILD$.host = scope:player");
    let tree = p.tree();
    let field = only_item(tree);
    assert_eq!(text(tree, field), "$CHILD$.host");
}

#[test]
fn empty_file() {
    let p = parse_ok("");
    let tree = p.tree();
    assert_eq!(tree.len(), 1, "only the file root");
    assert_eq!(tree.node(tree.root()).kind, NodeKind::File);
    assert_eq!(kids(tree, tree.root()).len(), 0);
    validate_tree(tree).unwrap();
}

#[test]
fn simple_scalar_item() {
    // A bare scalar at file level (valid).
    let p = parse_ok("bare_value");
    let tree = p.tree();
    let item = only_item(tree);
    assert_eq!(tree.node(item).kind, NodeKind::Scalar);
    assert_eq!(text(tree, item), "bare_value");
}

#[test]
fn nested_block() {
    let p = parse_ok("a = { b = { c = 1 } }");
    let tree = p.tree();
    let field = only_item(tree);
    let outer = kids(tree, field)[1];
    assert_eq!(tree.node(outer).kind, NodeKind::Block);
    let inner_field = kids(tree, outer)[0];
    let inner_block = kids(tree, inner_field)[1];
    assert_eq!(tree.node(inner_block).kind, NodeKind::Block);
}

#[test]
fn boolean_and_date_values() {
    let p = parse_ok("flag = yes\nwhen = 1099.1.1");
    let tree = p.tree();
    let items = kids(tree, tree.root());
    assert_eq!(items.len(), 2);
    assert_eq!(text(tree, kids(tree, items[0])[1]), "yes");
    assert_eq!(text(tree, kids(tree, items[1])[1]), "1099.1.1");
}

#[test]
fn quoted_string_value() {
    let p = parse_ok("name = \"Linnéa José\"");
    let tree = p.tree();
    let field = only_item(tree);
    assert_eq!(text(tree, kids(tree, field)[1]), "\"Linnéa José\"");
}

#[test]
fn typed_definition_is_two_items() {
    // `scripted_trigger foo = { ... }` is two sibling file items: a bare scalar
    // and a field. The parser does not interpret it semantically.
    let p = parse_ok("scripted_trigger foo = { x = 1 }");
    let tree = p.tree();
    let items = kids(tree, tree.root());
    assert_eq!(items.len(), 2);
    assert_eq!(tree.node(items[0]).kind, NodeKind::Scalar);
    assert_eq!(text(tree, items[0]), "scripted_trigger");
    assert_eq!(tree.node(items[1]).kind, NodeKind::Field);
    assert_eq!(text(tree, items[1]), "foo");
}

// ── Recovery ───────────────────────────────────────────────────────────────

const UNCLOSED: &str =
    "unclosed block (missing '}'; an inner block may have stolen the closing brace)";

#[test]
fn unclosed_block() {
    let p = parse("test", &b"key = { inner = value"[..]);
    assert_eq!(p.diagnostics().len(), 1);
    assert_eq!(p.diagnostics()[0].severity, Severity::Error);
    assert_eq!(p.diagnostics()[0].message, UNCLOSED);
    let tree = p.tree();
    let items = kids(tree, tree.root());
    assert_eq!(items.len(), 1);
    let val = kids(tree, items[0])[1];
    assert_eq!(tree.node(val).kind, NodeKind::Block);
    assert_eq!(kids(tree, val).len(), 1);
    validate_tree(tree).unwrap();
}

#[test]
fn unclosed_block_offset() {
    let p = parse("test", &b"key = { inner = value"[..]);
    assert_eq!(p.diagnostics().len(), 1);
    assert_eq!(p.diagnostics()[0].offset, 6, "offset of the '{{'");
}

#[test]
fn multiple_unclosed_blocks() {
    let p = parse("test", &b"a = { b = { y = 2"[..]);
    assert_eq!(p.diagnostics().len(), 2);
    for d in p.diagnostics() {
        assert_eq!(d.message, UNCLOSED);
    }
}

#[test]
fn recovery_after_missing_operator() {
    let p = parse("test", &b"bad_line\ngood = ok"[..]);
    assert!(p.diagnostics().is_empty());
    let tree = p.tree();
    assert_eq!(kids(tree, tree.root()).len(), 2);
}

#[test]
fn continues_after_unclosed_block() {
    let p = parse("test", &b"a = { x = 1\nb = ok"[..]);
    assert_eq!(p.diagnostics().len(), 1);
    assert_eq!(p.diagnostics()[0].message, UNCLOSED);
}

#[test]
fn missing_field_value() {
    // `key =` then EOF: parseValue sees EOF, reports "expected value, got eof".
    let p = parse("test", &b"key ="[..]);
    assert_eq!(p.diagnostics().len(), 1);
    assert_eq!(p.diagnostics()[0].message, "expected value, got eof");
    validate_tree(p.tree()).unwrap();
}

#[test]
fn unexpected_closing_delimiter() {
    // A stray '}' / ']' at item level is consumed and skipped without diagnostic.
    let p = parse("test", &b"} ] key = value"[..]);
    assert!(p.diagnostics().is_empty());
    let tree = p.tree();
    let items = kids(tree, tree.root());
    assert_eq!(items.len(), 1);
    assert_eq!(tree.node(items[0]).kind, NodeKind::Field);
}

#[test]
fn nested_malformed_block_terminates() {
    // Arbitrary junk inside a block must not loop forever and must return a tree.
    let p = parse("test", &b"a = { ! ? @ = = = }"[..]);
    // Always returns a tree; validator holds even on malformed input.
    validate_tree(p.tree()).unwrap();
}

#[test]
fn unexpected_eof_after_minus() {
    let p = parse("test", &b"x = -"[..]);
    assert_eq!(p.diagnostics().len(), 1);
    assert_eq!(p.diagnostics()[0].message, "unexpected EOF after '-'");
}

#[test]
fn terminates_on_arbitrary_bytes() {
    // Fuzz-like: a pile of operators and delimiters must terminate and validate.
    let inputs: &[&[u8]] = &[
        b"={}[]<><=>=?===!=|.:@$",
        b"}}}}}}",
        b"{{{{{{",
        b"= = = = =",
        b"-----",
        b"a:b:c:d = = = }",
        &[0xFF, 0x80, 0xC3, b' ', b'=', b' ', 0xFF],
    ];
    for input in inputs {
        let p = parse("test", input.to_vec());
        // The contract: a tree always exists and invariants hold.
        validate_tree(p.tree()).unwrap();
        assert_eq!(p.tree().node(p.tree().root()).kind, NodeKind::File);
    }
}
