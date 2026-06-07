package validate

import (
	"strings"
	"testing"

	"pdxl/internal/files"
)

func resolveDir(t *testing.T, dir string) []RefDiag {
	t.Helper()
	var fs files.FileSet
	if err := fs.Add(dir, files.FileKindMod); err != nil {
		t.Fatal(err)
	}
	tbl, err := Build(&fs, nil)
	if err != nil {
		t.Fatal(err)
	}
	diags, err := Resolve(tbl, &fs, nil)
	if err != nil {
		t.Fatal(err)
	}
	return diags
}

func TestResolveUndefinedTrait(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/traits/00_traits.txt", "brave = { }\n")
	writeFile(t, dir, "common/scripted_effects/00_e.txt",
		"give_it = {\n\tadd_trait = brave\n\tadd_trait = nonexistent\n}\n")

	diags := resolveDir(t, dir)

	if len(diags) != 1 {
		t.Fatalf("expected 1 diagnostic, got %d: %v", len(diags), diags)
	}
	if !strings.Contains(diags[0].Msg, "nonexistent") {
		t.Errorf("expected message to mention 'nonexistent', got %q", diags[0].Msg)
	}
	if !strings.Contains(diags[0].Msg, "trait") {
		t.Errorf("expected message to mention 'trait', got %q", diags[0].Msg)
	}
}

func TestResolveSkipsMacroAndScopeValues(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/traits/00_traits.txt", "brave = { }\n")
	writeFile(t, dir, "common/scripted_effects/00_e.txt",
		"give_it = {\n\tadd_trait = $TRAIT$\n\thas_trait = scope:x\n}\n")

	diags := resolveDir(t, dir)

	if len(diags) != 0 {
		t.Fatalf("expected no diagnostics for macro/scope values, got %d: %v", len(diags), diags)
	}
}

func TestResolveTraitGroupsAndQuotes(t *testing.T) {
	dir := t.TempDir()
	// brave is in group "personality_brave"; bastard is quoted at the ref site.
	writeFile(t, dir, "common/traits/00_traits.txt",
		"brave = { group = personality_brave }\nbastard = { }\n")
	writeFile(t, dir, "common/scripted_effects/00_e.txt",
		"give = {\n\thas_trait = personality_brave\n\tadd_trait = \"bastard\"\n}\n")

	diags := resolveDir(t, dir)

	if len(diags) != 0 {
		t.Fatalf("expected no diagnostics (group + quoted ref), got %d: %v", len(diags), diags)
	}
}

func TestResolveDefinedTraitOK(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/traits/00_traits.txt", "brave = { }\ncraven = { }\n")
	writeFile(t, dir, "common/scripted_effects/00_e.txt",
		"give_it = {\n\tadd_trait = brave\n\tremove_trait = craven\n}\n")

	diags := resolveDir(t, dir)

	if len(diags) != 0 {
		t.Fatalf("expected no diagnostics, got %d: %v", len(diags), diags)
	}
}
