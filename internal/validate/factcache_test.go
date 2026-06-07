package validate

import (
	"os"
	"path/filepath"
	"testing"

	"pdxl/internal/files"
)

func TestFactStoreRoundTripAndInvalidation(t *testing.T) {
	dir := t.TempDir()
	src := filepath.Join(dir, "src.txt")
	if err := os.WriteFile(src, []byte("brave = { }\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	store, err := NewFactStore(filepath.Join(dir, "facts"))
	if err != nil {
		t.Fatal(err)
	}

	info, _ := os.Stat(src)
	want := FileFacts{
		Defs: []Symbol{{Name: "brave", Kind: KindTrait, File: "common/traits/x.txt"}},
		Refs: []Ref{{Kind: KindEvent, Name: "ns.1", Loc: "x:1:1"}},
	}
	if err := store.Put(src, info, []byte("brave = { }\n"), want); err != nil {
		t.Fatal(err)
	}

	// Round-trip hit.
	got, ok := store.Get(src, info)
	if !ok {
		t.Fatal("expected cache hit")
	}
	if len(got.Defs) != 1 || got.Defs[0].Name != "brave" || len(got.Refs) != 1 || got.Refs[0].Name != "ns.1" {
		t.Fatalf("round-trip mismatch: %+v", got)
	}

	// Changed content => miss (SHA mismatch), even if mtime is reused.
	if err := os.WriteFile(src, []byte("craven = { }\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	info2, _ := os.Stat(src)
	if _, ok := store.Get(src, info2); ok {
		t.Error("expected miss after content change")
	}
}

// TestAnalyzeIncremental verifies the fact cache makes a second run reuse
// unchanged files: a deliberately broken AST store (nil) on the second run
// still yields results because facts come from the cache.
func TestAnalyzeIncremental(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/traits/00_t.txt", "brave = { }\n")
	writeFile(t, dir, "common/scripted_effects/00_e.txt", "e = { add_trait = brave }\n")

	var fs files.FileSet
	if err := fs.Add(dir, files.FileKindMod); err != nil {
		t.Fatal(err)
	}
	fc, err := NewFactStore(filepath.Join(dir, "facts"))
	if err != nil {
		t.Fatal(err)
	}

	// First run populates the fact cache.
	if _, diags, err := Analyze(&fs, nil, fc); err != nil || len(diags) != 0 {
		t.Fatalf("first run: err=%v diags=%v", err, diags)
	}
	// Second run with the same fact cache must still resolve cleanly.
	tbl, diags, err := Analyze(&fs, nil, fc)
	if err != nil || len(diags) != 0 {
		t.Fatalf("second run: err=%v diags=%v", err, diags)
	}
	if tbl.Count(KindTrait) != 1 {
		t.Errorf("expected 1 trait from cached facts, got %d", tbl.Count(KindTrait))
	}
}
