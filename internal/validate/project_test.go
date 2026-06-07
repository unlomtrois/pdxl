package validate

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pdxl/internal/files"
)

func newProjectDir(t *testing.T, dir string) *Project {
	t.Helper()
	var fs files.FileSet
	if err := fs.Add(dir, files.FileKindMod); err != nil {
		t.Fatal(err)
	}
	p, err := NewProject(&fs, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	return p
}

func TestProjectInitialDiags(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/traits/00_t.txt", "brave = { }\n")
	writeFile(t, dir, "common/scripted_effects/00_e.txt", "e = { add_trait = brave }\n")

	p := newProjectDir(t, dir)

	if got := len(p.Diags()); got != 0 {
		t.Fatalf("expected 0 diags, got %d: %v", got, p.Diags())
	}
	if p.Table().Count(KindTrait) != 1 {
		t.Errorf("expected 1 trait, got %d", p.Table().Count(KindTrait))
	}
}

func TestProjectIncrementalUpdate(t *testing.T) {
	dir := t.TempDir()
	traitPath := filepath.Join(dir, "common/traits/00_t.txt")
	writeFile(t, dir, "common/traits/00_t.txt", "brave = { }\n")
	writeFile(t, dir, "common/scripted_effects/00_e.txt", "e = { add_trait = brave }\n")

	p := newProjectDir(t, dir)
	if got := len(p.Diags()); got != 0 {
		t.Fatalf("baseline: expected 0 diags, got %v", p.Diags())
	}

	// Rename the trait on disk: brave is gone. Update only the trait file;
	// the effect file is not re-read, but its reference must now be unresolved.
	if err := os.WriteFile(traitPath, []byte("bold = { }\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := p.Update(traitPath); err != nil {
		t.Fatal(err)
	}

	diags := p.Diags()
	if len(diags) != 1 {
		t.Fatalf("after update: expected 1 diag, got %d: %v", len(diags), diags)
	}
	if p.Table().Count(KindTrait) != 1 {
		t.Errorf("expected still 1 trait (bold), got %d", p.Table().Count(KindTrait))
	}
	if _, ok := p.Table().Lookup(KindTrait, "bold"); !ok {
		t.Error("expected 'bold' to be defined after update")
	}
}

func TestProjectUpdateSource(t *testing.T) {
	dir := t.TempDir()
	traitPath := filepath.Join(dir, "common/traits/00_t.txt")
	writeFile(t, dir, "common/traits/00_t.txt", "brave = { }\n")
	writeFile(t, dir, "common/scripted_effects/00_e.txt", "e = { add_trait = brave }\n")

	p := newProjectDir(t, dir)
	if len(p.Diags()) != 0 {
		t.Fatalf("baseline: expected 0 diags, got %v", p.Diags())
	}

	// In-memory edit (disk unchanged): the trait is renamed in the buffer.
	if err := p.UpdateSource(traitPath, []byte("bold = { }\n")); err != nil {
		t.Fatal(err)
	}
	if len(p.Diags()) != 1 {
		t.Fatalf("after in-memory edit: expected 1 diag, got %v", p.Diags())
	}
	// Disk still has brave; a disk-based Update reverts the buffer view.
	if err := p.Update(traitPath); err != nil {
		t.Fatal(err)
	}
	if len(p.Diags()) != 0 {
		t.Fatalf("after disk reload: expected 0 diags, got %v", p.Diags())
	}
}

func TestRefDiagCarriesOffsets(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/scripted_effects/a.txt", "e = { add_trait = nope }\n")
	p := newProjectDir(t, dir)
	d := p.Diags()
	if len(d) != 1 {
		t.Fatalf("expected 1 diag, got %v", d)
	}
	if d[0].File == "" || d[0].End <= d[0].Start {
		t.Errorf("expected structured offsets, got File=%q Start=%d End=%d", d[0].File, d[0].Start, d[0].End)
	}
}

func TestProjectFileDiags(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/scripted_effects/a.txt", "e = { add_trait = missing_a }\n")
	writeFile(t, dir, "common/scripted_effects/b.txt", "e = { add_trait = missing_b }\n")

	p := newProjectDir(t, dir)

	if got := len(p.Diags()); got != 2 {
		t.Fatalf("expected 2 project diags, got %d: %v", got, p.Diags())
	}
	aPath := filepath.Join(dir, "common/scripted_effects/a.txt")
	fd := p.FileDiags(aPath)
	if len(fd) != 1 {
		t.Fatalf("expected 1 diag for a.txt, got %d: %v", len(fd), fd)
	}
	if !strings.Contains(fd[0].Msg, "missing_a") {
		t.Errorf("expected a.txt diag to mention missing_a, got %q", fd[0].Msg)
	}
}
