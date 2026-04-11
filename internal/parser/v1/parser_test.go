package v1

import (
	"testing"
)

func TestSimpleField(t *testing.T) {
	src := []byte(`key = value`)
	f, err := ParseBytes("test", src)
	if err != nil {
		t.Fatal(err)
	}
	if len(f.Items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(f.Items))
	}
	field := f.Items[0].Field
	if field == nil {
		t.Fatal("expected Field, got Scalar")
	}
	scalar, ok := field.Value.(*Scalar)
	if !ok {
		t.Fatalf("expected Scalar value, got %T", field.Value)
	}
	if field.Key() != "key" || field.Operator != "=" || scalar.Value() != "value" {
		t.Fatalf("unexpected field: key=%q op=%q val=%q", field.Key(), field.Operator, scalar.Value())
	}
}

func TestBlockField(t *testing.T) {
	src := []byte(`limit = { age > 18 }`)
	f, err := ParseBytes("test", src)
	if err != nil {
		t.Fatal(err)
	}
	field := f.Items[0].Field
	if field == nil {
		t.Fatal("expected Field")
	}
	block, ok := field.Value.(*Block)
	if !ok {
		t.Fatalf("expected Block value, got %T", field.Value)
	}
	inner := block.Items[0].Field
	if inner == nil || inner.Key() != "age" || inner.Operator != ">" {
		t.Fatalf("unexpected inner field: %+v", inner)
	}
}

func TestValueListBlock(t *testing.T) {
	// Bare values, no operators — should produce a Block with Scalar items.
	src := []byte(`color = { 255 255 255 }`)
	f, err := ParseBytes("test", src)
	if err != nil {
		t.Fatal(err)
	}
	field := f.Items[0].Field
	if field == nil {
		t.Fatal("expected Field")
	}
	block, ok := field.Value.(*Block)
	if !ok {
		t.Fatalf("expected Block value, got %T", field.Value)
	}
	if len(block.Items) != 3 {
		t.Fatalf("expected 3 items in block, got %d", len(block.Items))
	}
	for i, item := range block.Items {
		if item.Scalar == nil {
			t.Fatalf("item %d: expected Scalar, got Field", i)
		}
	}
}

func TestTaggedBlock(t *testing.T) {
	src := []byte(`color = rgb { 218 215 56 }`)
	f, err := ParseBytes("test", src)
	if err != nil {
		t.Fatal(err)
	}
	field := f.Items[0].Field
	if field == nil {
		t.Fatal("expected Field")
	}
	tb, ok := field.Value.(*TaggedBlock)
	if !ok {
		t.Fatalf("expected TaggedBlock, got %T", field.Value)
	}
	if tb.Tag != "rgb" {
		t.Fatalf("expected tag 'rgb', got %q", tb.Tag)
	}
	if len(tb.Items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(tb.Items))
	}
}

func TestIdentifierList(t *testing.T) {
	src := []byte(`members = { GEN GAZ }`)
	f, err := ParseBytes("test", src)
	if err != nil {
		t.Fatal(err)
	}
	field := f.Items[0].Field
	block, ok := field.Value.(*Block)
	if !ok {
		t.Fatalf("expected Block, got %T", field.Value)
	}
	if len(block.Items) != 2 {
		t.Fatalf("expected 2 members, got %d", len(block.Items))
	}
}

func TestBoolean(t *testing.T) {
	src := []byte(`is_adult = yes`)
	f, err := ParseBytes("test", src)
	if err != nil {
		t.Fatal(err)
	}
	field := f.Items[0].Field
	scalar, ok := field.Value.(*Scalar)
	if !ok {
		t.Fatalf("expected Scalar, got %T", field.Value)
	}
	if scalar.Value() != "yes" {
		t.Fatalf("expected 'yes', got %q", scalar.Value())
	}
}

func TestMultipleFields(t *testing.T) {
	src := []byte(`
name = "William"
age = 42
culture = english
`)
	f, err := ParseBytes("test", src)
	if err != nil {
		t.Fatal(err)
	}
	if len(f.Items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(f.Items))
	}
}
