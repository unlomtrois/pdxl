package v3

import (
	"testing"
)

func mustParse(t *testing.T, src []byte) *Tree {
	t.Helper()
	tree, diags := Parse("test", src)
	if len(diags) > 0 {
		t.Fatalf("unexpected diagnostics: %v", diags)
	}
	return tree
}

func TestSimpleField(t *testing.T) {
	tree := mustParse(t, []byte(`key = value`))
	root := tree.Root()
	children := tree.Children(root)
	if len(children) != 1 {
		t.Fatalf("expected 1 item, got %d", len(children))
	}
	field := children[0]
	if field.Kind != KindField {
		t.Fatalf("expected KindField, got %v", field.Kind)
	}
	if field.OpString() != "=" {
		t.Fatalf("expected '=' operator, got %q", field.OpString())
	}
	fc := tree.Children(field)
	if len(fc) != 2 {
		t.Fatalf("expected 2 field children (key+value), got %d", len(fc))
	}
	if fc[0].Value(tree.Src) != "key" {
		t.Fatalf("expected key 'key', got %q", fc[0].Value(tree.Src))
	}
	if fc[1].Kind != KindScalar || fc[1].Value(tree.Src) != "value" {
		t.Fatalf("expected scalar 'value', got %q", fc[1].Value(tree.Src))
	}
}

func TestBlockField(t *testing.T) {
	tree := mustParse(t, []byte(`limit = { age > 18 }`))
	root := tree.Root()
	field := tree.Children(root)[0]
	if field.Kind != KindField {
		t.Fatalf("expected KindField")
	}
	fc := tree.Children(field)
	val := fc[1]
	if val.Kind != KindBlock {
		t.Fatalf("expected KindBlock, got %v", val.Kind)
	}
	blockChildren := tree.Children(val)
	if len(blockChildren) != 1 {
		t.Fatalf("expected 1 inner field, got %d", len(blockChildren))
	}
	inner := blockChildren[0]
	if inner.Kind != KindField {
		t.Fatalf("expected inner KindField")
	}
	if inner.Value(tree.Src) != "age" {
		t.Fatalf("expected key 'age', got %q", inner.Value(tree.Src))
	}
}

func TestValueListBlock(t *testing.T) {
	tree := mustParse(t, []byte(`color = { 255 255 255 }`))
	field := tree.Children(tree.Root())[0]
	val := tree.Children(field)[1]
	if val.Kind != KindBlock {
		t.Fatalf("expected KindBlock, got %v", val.Kind)
	}
	if n := len(tree.Children(val)); n != 3 {
		t.Fatalf("expected 3 scalars, got %d", n)
	}
}

func TestTaggedBlock(t *testing.T) {
	tree := mustParse(t, []byte(`color = rgb { 218 215 56 }`))
	field := tree.Children(tree.Root())[0]
	val := tree.Children(field)[1]
	if val.Kind != KindTaggedBlock {
		t.Fatalf("expected KindTaggedBlock, got %v", val.Kind)
	}
	if val.Value(tree.Src) != "rgb" {
		t.Fatalf("expected tag 'rgb', got %q", val.Value(tree.Src))
	}
	if n := len(tree.Children(val)); n != 3 {
		t.Fatalf("expected 3 items, got %d", n)
	}
}

func TestScopeKeyWithOperator(t *testing.T) {
	tree := mustParse(t, []byte(`scope:actor ?= { is_subject = yes }`))
	field := tree.Children(tree.Root())[0]
	if field.Kind != KindField {
		t.Fatalf("expected KindField")
	}
	if field.Value(tree.Src) != "scope:actor" {
		t.Fatalf("expected key 'scope:actor', got %q", field.Value(tree.Src))
	}
}

func TestNegativeNumber(t *testing.T) {
	tree := mustParse(t, []byte(`modifier = -0.25`))
	field := tree.Children(tree.Root())[0]
	val := tree.Children(field)[1]
	if val.Value(tree.Src) != "-0.25" {
		t.Fatalf("expected '-0.25', got %q", val.Value(tree.Src))
	}
}

func TestScopeChainValue(t *testing.T) {
	tree := mustParse(t, []byte(`target = define:NMapColors|CONSTANT`))
	field := tree.Children(tree.Root())[0]
	val := tree.Children(field)[1]
	if val.Value(tree.Src) != "define:NMapColors|CONSTANT" {
		t.Fatalf("expected scope chain, got %q", val.Value(tree.Src))
	}
}

// ── Recovery tests ────────────────────────────────────────────────────────────

func TestUnclosedBlock(t *testing.T) {
	src := []byte("key = { inner = value")
	tree, diags := Parse("test", src)
	if len(diags) != 1 {
		t.Fatalf("expected 1 diagnostic, got %d: %v", len(diags), diags)
	}
	if diags[0].Severity != SeverityError {
		t.Fatalf("expected error severity")
	}
	if diags[0].Msg != "unclosed block (missing '}'; an inner block may have stolen the closing brace)" {
		t.Fatalf("unexpected message: %q", diags[0].Msg)
	}
	// Partial tree: top-level field with a block containing the inner field.
	items := tree.Children(tree.Root())
	if len(items) != 1 {
		t.Fatalf("expected 1 top-level item, got %d", len(items))
	}
	val := tree.Children(items[0])[1]
	if val.Kind != KindBlock {
		t.Fatalf("expected KindBlock, got %v", val.Kind)
	}
	if n := len(tree.Children(val)); n != 1 {
		t.Fatalf("expected 1 inner item, got %d", n)
	}
}

func TestUnclosedBlockOffset(t *testing.T) {
	// Offset of the '{' should be reported, not the EOF.
	src := []byte("key = { inner = value")
	//                    ^ offset 6
	_, diags := Parse("test", src)
	if len(diags) != 1 {
		t.Fatalf("expected 1 diagnostic, got %d", len(diags))
	}
	if diags[0].Offset != 6 {
		t.Fatalf("expected offset 6 (the '{'), got %d", diags[0].Offset)
	}
}

func TestMultipleUnclosedBlocks(t *testing.T) {
	// Nested unclosed blocks: both the inner { and outer { are unclosed.
	// b = { y = 2 is absorbed into a's block, making two nested unclosed blocks.
	src := []byte("a = { b = { y = 2")
	_, diags := Parse("test", src)
	if len(diags) != 2 {
		t.Fatalf("expected 2 diagnostics, got %d: %v", len(diags), diags)
	}
	for _, d := range diags {
		if d.Msg != "unclosed block (missing '}'; an inner block may have stolen the closing brace)" {
			t.Fatalf("unexpected message: %q", d.Msg)
		}
	}
}

func TestMacroParamAsValue(t *testing.T) {
	tree := mustParse(t, []byte(`exists = $CHILD$`))
	val := tree.Children(tree.Children(tree.Root())[0])[1]
	if val.Value(tree.Src) != "$CHILD$" {
		t.Fatalf("expected $CHILD$, got %q", val.Value(tree.Src))
	}
}

func TestMacroParamAsKey(t *testing.T) {
	tree := mustParse(t, []byte(`$CHILD$ = { a = b }`))
	field := tree.Children(tree.Root())[0]
	if field.Value(tree.Src) != "$CHILD$" {
		t.Fatalf("expected $CHILD$, got %q", field.Value(tree.Src))
	}
}

func TestMacroParamScopeChain(t *testing.T) {
	tree := mustParse(t, []byte(`$CHILD$.host = scope:player`))
	field := tree.Children(tree.Root())[0]
	if field.Value(tree.Src) != "$CHILD$.host" {
		t.Fatalf("expected $CHILD$.host, got %q", field.Value(tree.Src))
	}
}

func TestRecoveryAfterMissingOperator(t *testing.T) {
	// "key value" has no operator; parser should skip it and continue.
	src := []byte("bad_line\ngood = ok")
	tree, diags := Parse("test", src)
	// "bad_line" is a bare scalar (valid); "good = ok" is a field. No errors.
	if len(diags) != 0 {
		t.Fatalf("unexpected diagnostics: %v", diags)
	}
	items := tree.Children(tree.Root())
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}
}

func TestContinuesAfterUnclosedBlock(t *testing.T) {
	// When a block is unclosed, subsequent fields are consumed into it —
	// the parser cannot distinguish them from block-level items without
	// indentation heuristics. The important thing is: one diagnostic, no crash.
	src := []byte("a = { x = 1\nb = ok")
	_, diags := Parse("test", src)
	if len(diags) != 1 {
		t.Fatalf("expected 1 diagnostic, got %d: %v", len(diags), diags)
	}
	if diags[0].Msg != "unclosed block (missing '}'; an inner block may have stolen the closing brace)" {
		t.Fatalf("unexpected message: %q", diags[0].Msg)
	}
}
