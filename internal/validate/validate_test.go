package validate

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"pdxl/internal/files"
)

func writeFile(t *testing.T, dir, rel, content string) {
	t.Helper()
	full := filepath.Join(dir, filepath.FromSlash(rel))
	if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(full, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func buildDir(t *testing.T, dir string) *SymbolTable {
	t.Helper()
	var fs files.FileSet
	if err := fs.Add(dir, files.FileKindMod); err != nil {
		t.Fatal(err)
	}
	tbl, err := Build(&fs, nil)
	if err != nil {
		t.Fatal(err)
	}
	return tbl
}

func TestBuildCollectsDefinitions(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/scripted_triggers/00_t.txt",
		"alpha_trigger = { always = yes }\nbeta_trigger = { always = no }\n")
	writeFile(t, dir, "common/traits/00_traits.txt",
		"diplomat = { diplomacy = 1 }\n")
	writeFile(t, dir, "events/test_events.txt",
		"namespace = test\ntest.0001 = { type = character_event }\n")

	tbl := buildDir(t, dir)

	if got := tbl.Count(KindScriptedTrigger); got != 2 {
		t.Errorf("scripted_trigger count: got %d, want 2", got)
	}
	if got := tbl.Count(KindTrait); got != 1 {
		t.Errorf("trait count: got %d, want 1", got)
	}
	if got := tbl.Count(KindEvent); got != 1 { // namespace = test must be skipped
		t.Errorf("event count: got %d, want 1", got)
	}
	if _, ok := tbl.Lookup(KindScriptedTrigger, "alpha_trigger"); !ok {
		t.Error("expected alpha_trigger to be indexed")
	}
	if _, ok := tbl.Lookup(KindEvent, "test.0001"); !ok {
		t.Error("expected test.0001 to be indexed")
	}
}

func TestBuildCollectsCharacters(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "history/characters/afar.txt",
		"145665 = { name = \"Foo\" }\n145666 = { name = \"Bar\" }\n")

	tbl := buildDir(t, dir)

	if got := tbl.Count(KindCharacter); got != 2 {
		t.Errorf("character count: got %d, want 2", got)
	}
	if _, ok := tbl.Lookup(KindCharacter, "145665"); !ok {
		t.Error("expected character 145665 to be indexed")
	}
}

func TestBuildCapturesMacroParams(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/scripted_triggers/00_t.txt",
		"my_trigger = {\n\tx = $OPERATOR$\n\tcount >= $COUNT$\n}\n")

	tbl := buildDir(t, dir)

	sym, ok := tbl.Lookup(KindScriptedTrigger, "my_trigger")
	if !ok {
		t.Fatal("expected my_trigger to be indexed")
	}
	want := []string{"COUNT", "OPERATOR"} // sorted, deduped
	if !reflect.DeepEqual(sym.Params, want) {
		t.Errorf("params: got %v, want %v", sym.Params, want)
	}
}

func TestBuildRecordsDuplicates(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/traits/00_traits.txt", "brave = { }\n")
	writeFile(t, dir, "common/traits/01_more.txt", "brave = { }\n")

	tbl := buildDir(t, dir)

	if len(tbl.Duplicates) != 1 {
		t.Fatalf("expected 1 duplicate, got %d: %v", len(tbl.Duplicates), tbl.Duplicates)
	}
	if tbl.Duplicates[0].Name != "brave" {
		t.Errorf("duplicate name: got %q", tbl.Duplicates[0].Name)
	}
}

func TestBuildIgnoresUnknownDirs(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "gfx/whatever.txt", "some_block = { x = 1 }\n")

	tbl := buildDir(t, dir)

	if got := tbl.Total(); got != 0 {
		t.Errorf("expected no symbols from unregistered dir, got %d", got)
	}
}
