package files

import (
	"os"
	"path/filepath"
	"sort"
	"testing"

	"pdxl/internal/testutil"
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

func TestAddAndResolve(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/traits/noble.txt", "")

	var s FileSet
	if err := s.Add(dir, FileKindVanilla); err != nil {
		t.Fatal(err)
	}

	e, ok := s.Resolve("common/traits/noble.txt")
	if !ok {
		t.Fatal("expected entry to be found")
	}
	if e.Kind != FileKindVanilla {
		t.Errorf("kind: got %d, want FileKindVanilla", e.Kind)
	}
	if e.RelPath != "common/traits/noble.txt" {
		t.Errorf("relPath: got %q", e.RelPath)
	}
}

func TestSetIgnore(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/traits/noble.txt", "")
	writeFile(t, dir, "licenses/software/zlib.txt", "")
	writeFile(t, dir, "credits.txt", "")
	writeFile(t, dir, "fonts/Open_Sans/LICENSE.txt", "") // case-insensitive

	var s FileSet
	s.SetIgnore([]string{"licenses"}, []string{"credits.txt", "license.txt"})
	if err := s.Add(dir, FileKindVanilla); err != nil {
		t.Fatal(err)
	}

	if _, ok := s.Resolve("common/traits/noble.txt"); !ok {
		t.Error("expected script file to be kept")
	}
	for _, rel := range []string{
		"licenses/software/zlib.txt",
		"credits.txt",
		"fonts/Open_Sans/LICENSE.txt",
	} {
		if _, ok := s.Resolve(rel); ok {
			t.Errorf("expected %q to be ignored", rel)
		}
	}
	if got := s.Stats().Total; got != 1 {
		t.Errorf("total: got %d, want 1", got)
	}
}

func TestOverlayShadowing(t *testing.T) {
	vanilla := t.TempDir()
	mod := t.TempDir()
	writeFile(t, vanilla, "common/traits/noble.txt", "vanilla")
	writeFile(t, mod, "common/traits/noble.txt", "mod")

	var s FileSet
	if err := s.Add(vanilla, FileKindVanilla); err != nil {
		t.Fatal(err)
	}
	if err := s.Add(mod, FileKindMod); err != nil {
		t.Fatal(err)
	}

	e, ok := s.Resolve("common/traits/noble.txt")
	if !ok {
		t.Fatal("expected entry")
	}
	if e.Kind != FileKindMod {
		t.Errorf("mod should shadow vanilla; got kind %d", e.Kind)
	}
}

func TestWalkAllWinners(t *testing.T) {
	vanilla := t.TempDir()
	mod := t.TempDir()
	writeFile(t, vanilla, "a.txt", "")
	writeFile(t, vanilla, "b.txt", "")
	writeFile(t, mod, "b.txt", "") // shadows vanilla b.txt
	writeFile(t, mod, "c.txt", "")

	var s FileSet
	_ = s.Add(vanilla, FileKindVanilla)
	_ = s.Add(mod, FileKindMod)

	var paths []string
	_ = s.Walk(func(e FileEntry) error {
		paths = append(paths, e.RelPath)
		return nil
	})
	sort.Strings(paths)

	want := []string{"a.txt", "b.txt", "c.txt"}
	if len(paths) != len(want) {
		t.Fatalf("Walk returned %v, want %v", paths, want)
	}
	for i, p := range paths {
		if p != want[i] {
			t.Errorf("paths[%d]: got %q, want %q", i, p, want[i])
		}
	}

	// b.txt winner must be from mod
	e, _ := s.Resolve("b.txt")
	if e.Kind != FileKindMod {
		t.Error("b.txt winner should be mod")
	}
}

func TestSkipDotDirs(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, ".git/config.txt", "")
	writeFile(t, dir, "real.txt", "")

	var s FileSet
	_ = s.Add(dir, FileKindMod)

	var paths []string
	_ = s.Walk(func(e FileEntry) error { paths = append(paths, e.RelPath); return nil })

	if len(paths) != 1 || paths[0] != "real.txt" {
		t.Errorf("expected [real.txt], got %v", paths)
	}
}

func TestReplacePathDropsVanilla(t *testing.T) {
	vanilla := t.TempDir()
	mod := t.TempDir()
	// vanilla has files in a replaced dir and a normal dir
	writeFile(t, vanilla, "common/landed_titles/base.txt", "")
	writeFile(t, vanilla, "common/traits/noble.txt", "")
	// mod only provides its own landed_titles
	writeFile(t, mod, "common/landed_titles/custom.txt", "")

	var s FileSet
	s.SetReplacePaths([]string{"common/landed_titles"})
	_ = s.Add(vanilla, FileKindVanilla)
	_ = s.Add(mod, FileKindMod)

	// vanilla file in replaced dir must be absent
	if _, ok := s.Resolve("common/landed_titles/base.txt"); ok {
		t.Error("vanilla base.txt should be dropped due to replace_path")
	}
	// mod file in replaced dir must be present
	if _, ok := s.Resolve("common/landed_titles/custom.txt"); !ok {
		t.Error("mod custom.txt should be present")
	}
	// non-replaced vanilla file must still be present
	if _, ok := s.Resolve("common/traits/noble.txt"); !ok {
		t.Error("vanilla noble.txt should still be present")
	}
}

func TestParseMod(t *testing.T) {
	modFile := filepath.Join(testutil.TestdataDir(), "T4N.mod")
	m, err := ParseMod(modFile)
	if err != nil {
		t.Fatal(err)
	}
	if m.Name != "The Four Nations" {
		t.Errorf("name: got %q, want %q", m.Name, "The Four Nations")
	}
	if m.Path == "" {
		t.Error("expected non-empty path")
	}
	if len(m.ReplacePaths) == 0 {
		t.Error("expected replace_path entries")
	}
	// spot-check a known replace_path
	found := false
	for _, rp := range m.ReplacePaths {
		if rp == "common/landed_titles" {
			found = true
		}
	}
	if !found {
		t.Errorf("expected common/landed_titles in replace_paths, got %v", m.ReplacePaths)
	}
}

func TestParseModWindowsPath(t *testing.T) {
	dir := t.TempDir()
	modFile := filepath.Join(dir, "test.mod")
	content := `name="Test Mod"` + "\n" + `path="C:/users/steamuser/Documents/Paradox Interactive/Crusader Kings III/mod/mods/TestMod"` + "\n"
	if err := os.WriteFile(modFile, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
	m, err := ParseMod(modFile)
	if err != nil {
		t.Fatal(err)
	}
	if !IsWindowsAbsolute(m.Path) {
		t.Errorf("expected Windows absolute path returned as-is, got %q", m.Path)
	}
}

func TestParseModRelativePath(t *testing.T) {
	dir := t.TempDir()
	modFile := filepath.Join(dir, "mymod.mod")
	content := `name="My Mod"` + "\n" + `path="mods/mymod"` + "\n"
	if err := os.WriteFile(modFile, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
	m, err := ParseMod(modFile)
	if err != nil {
		t.Fatal(err)
	}
	expected := filepath.Join(dir, "mods", "mymod")
	if m.Path != expected {
		t.Errorf("path: got %q, want %q", m.Path, expected)
	}
}

func TestNonTxtSkipped(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "mod.mod", "")
	writeFile(t, dir, "notes.log", "")
	writeFile(t, dir, "events.txt", "")

	var s FileSet
	_ = s.Add(dir, FileKindMod)

	var paths []string
	_ = s.Walk(func(e FileEntry) error { paths = append(paths, e.RelPath); return nil })

	if len(paths) != 1 || paths[0] != "events.txt" {
		t.Errorf("expected [events.txt], got %v", paths)
	}
}
