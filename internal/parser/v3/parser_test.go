package v3

import (
	"testing"
)

func TestSimpleField(t *testing.T) {
	tree, err := Parse("test", []byte(`key = value`))
	if err != nil {
		t.Fatal(err)
	}
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
	tree, err := Parse("test", []byte(`limit = { age > 18 }`))
	if err != nil {
		t.Fatal(err)
	}
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
	tree, err := Parse("test", []byte(`color = { 255 255 255 }`))
	if err != nil {
		t.Fatal(err)
	}
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
	tree, err := Parse("test", []byte(`color = rgb { 218 215 56 }`))
	if err != nil {
		t.Fatal(err)
	}
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
	tree, err := Parse("test", []byte(`scope:actor ?= { is_subject = yes }`))
	if err != nil {
		t.Fatal(err)
	}
	field := tree.Children(tree.Root())[0]
	if field.Kind != KindField {
		t.Fatalf("expected KindField")
	}
	if field.Value(tree.Src) != "scope:actor" {
		t.Fatalf("expected key 'scope:actor', got %q", field.Value(tree.Src))
	}
}

func TestNegativeNumber(t *testing.T) {
	tree, err := Parse("test", []byte(`modifier = -0.25`))
	if err != nil {
		t.Fatal(err)
	}
	field := tree.Children(tree.Root())[0]
	val := tree.Children(field)[1]
	if val.Value(tree.Src) != "-0.25" {
		t.Fatalf("expected '-0.25', got %q", val.Value(tree.Src))
	}
}

func TestScopeChainValue(t *testing.T) {
	tree, err := Parse("test", []byte(`target = define:NMapColors|CONSTANT`))
	if err != nil {
		t.Fatal(err)
	}
	field := tree.Children(tree.Root())[0]
	val := tree.Children(field)[1]
	if val.Value(tree.Src) != "define:NMapColors|CONSTANT" {
		t.Fatalf("expected scope chain, got %q", val.Value(tree.Src))
	}
}
