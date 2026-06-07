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

func TestResolveEventReferences(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "events/test_events.txt",
		"namespace = test\ntest.0001 = { type = character_event }\n")
	writeFile(t, dir, "common/scripted_effects/00_e.txt",
		"fire = {\n"+
			"\ttrigger_event = test.0001\n"+ // scalar, defined
			"\ttrigger_event = test.9999\n"+ // scalar, undefined
			"\ttrigger_event = { id = test.0001 days = 5 }\n"+ // block, defined
			"\ttrigger_event = { id = test.8888 }\n"+ // block, undefined
			"}\n")

	diags := resolveDir(t, dir)

	if len(diags) != 2 {
		t.Fatalf("expected 2 diagnostics, got %d: %v", len(diags), diags)
	}
	joined := diags[0].Msg + " " + diags[1].Msg
	for _, want := range []string{"test.9999", "test.8888", "event"} {
		if !strings.Contains(joined, want) {
			t.Errorf("expected diagnostics to mention %q, got %q", want, joined)
		}
	}
}

func TestResolveOnActionLists(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "events/test_events.txt",
		"namespace = test\ntest.0001 = { }\n")
	writeFile(t, dir, "common/on_action/00_oa.txt",
		"real_oa = { }\n"+
			"my_oa = {\n"+
			"\tevents = { test.0001 test.9999 }\n"+ // loose: 1 undefined
			"\tfirst_valid = { test.0001 }\n"+ // loose: ok
			"\trandom_events = { 100 = test.0001  50 = test.8888  chance_to_happen = 10  0 = 0 }\n"+ // weighted: 1 undefined
			"\ton_actions = { real_oa missing_oa }\n"+ // loose on_action: 1 undefined
			"}\n")

	diags := resolveDir(t, dir)

	if len(diags) != 3 {
		t.Fatalf("expected 3 diagnostics, got %d: %v", len(diags), diags)
	}
	joined := ""
	for _, d := range diags {
		joined += d.Msg + "\n"
	}
	for _, want := range []string{"test.9999", "test.8888", "missing_oa"} {
		if !strings.Contains(joined, want) {
			t.Errorf("expected diagnostics to mention %q, got:\n%s", want, joined)
		}
	}
}

func TestResolveListRulesOnlyInOnActionFiles(t *testing.T) {
	dir := t.TempDir()
	// `events = { ... }` outside common/on_action/ must not be resolved.
	writeFile(t, dir, "common/scripted_effects/00_e.txt",
		"e = { events = { totally.9999 } }\n")

	diags := resolveDir(t, dir)

	if len(diags) != 0 {
		t.Fatalf("expected no diagnostics outside on_action files, got %d: %v", len(diags), diags)
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
