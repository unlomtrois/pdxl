package lsp

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/tliron/glsp"
	protocol "github.com/tliron/glsp/protocol_3_16"

	"pdxl/internal/config"
)

func writeFile(t *testing.T, dir, rel, content string) string {
	t.Helper()
	full := filepath.Join(dir, filepath.FromSlash(rel))
	if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(full, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
	return full
}

// captureCtx returns a glsp.Context whose Notify records published diagnostics
// per URI (last publish wins).
func captureCtx(got map[string][]protocol.Diagnostic) *glsp.Context {
	return &glsp.Context{Notify: func(method string, params any) {
		if method != protocol.ServerTextDocumentPublishDiagnostics {
			return
		}
		if p, ok := params.(protocol.PublishDiagnosticsParams); ok {
			got[p.URI] = p.Diagnostics
		}
	}}
}

func TestServerDiagnosticsLifecycle(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "common/traits/00_t.txt", "brave = { }\n")
	effectPath := writeFile(t, dir, "common/scripted_effects/00_e.txt",
		"e = { add_trait = brave }\n") // disk version is clean

	cfg := config.Default()
	cfg.Cache.Enabled = false
	s := NewServer(Options{Config: cfg})
	if err := s.buildProject("", dir); err != nil {
		t.Fatal(err)
	}

	uri := pathToURI(effectPath)
	got := map[string][]protocol.Diagnostic{}
	ctx := captureCtx(got)

	// Open with an unsaved buffer that references an undefined trait.
	if err := s.didOpen(ctx, &protocol.DidOpenTextDocumentParams{
		TextDocument: protocol.TextDocumentItem{URI: uri, Text: "e = { add_trait = nope }\n"},
	}); err != nil {
		t.Fatal(err)
	}
	d := got[uri]
	if len(d) != 1 {
		t.Fatalf("expected 1 diagnostic after open, got %d: %v", len(d), d)
	}
	if !strings.Contains(d[0].Message, "nope") {
		t.Errorf("expected message to mention nope, got %q", d[0].Message)
	}
	if d[0].Range.End.Character <= d[0].Range.Start.Character {
		t.Errorf("expected a non-empty range, got %+v", d[0].Range)
	}

	// Fix the reference in the buffer and re-analyze synchronously.
	s.mu.Lock()
	s.docs[uri] = []byte("e = { add_trait = brave }\n")
	s.mu.Unlock()
	s.analyzeAndPublish(ctx.Notify, uri)
	if d := got[uri]; len(d) != 0 {
		t.Fatalf("expected 0 diagnostics after fix, got %d: %v", len(d), d)
	}

	// Closing clears diagnostics.
	if err := s.didClose(ctx, &protocol.DidCloseTextDocumentParams{
		TextDocument: protocol.TextDocumentIdentifier{URI: uri},
	}); err != nil {
		t.Fatal(err)
	}
	if d, ok := got[uri]; !ok || len(d) != 0 {
		t.Errorf("expected cleared diagnostics on close, got %v (present=%v)", d, ok)
	}
}

func TestProjectWideDiagnostics(t *testing.T) {
	vanillaDir := t.TempDir()
	modDir := t.TempDir()

	// Vanilla file with an unresolved reference — must NOT be flagged.
	vanillaEffect := writeFile(t, vanillaDir, "common/scripted_effects/00_v.txt",
		"v = { add_trait = ghost }\n")
	// Mod file with an unresolved reference — must be flagged even though it's
	// never opened.
	writeFile(t, modDir, "common/traits/00_t.txt", "brave = { }\n")
	modEffect := writeFile(t, modDir, "common/scripted_effects/00_e.txt",
		"e = { add_trait = nope }\n")

	cfg := config.Default()
	cfg.Cache.Enabled = false
	s := NewServer(Options{Config: cfg})
	s.initMod = modDir
	if err := s.buildProject(vanillaDir, modDir); err != nil {
		t.Fatal(err)
	}

	got := map[string][]protocol.Diagnostic{}
	ctx := captureCtx(got)

	// Publish project-wide without opening any document.
	s.mu.Lock()
	s.publishProjectDiagnostics(ctx.Notify)
	s.mu.Unlock()

	// The unopened mod effect file is flagged.
	modURI := pathToURI(modEffect)
	if d := got[modURI]; len(d) != 1 {
		t.Fatalf("expected 1 diagnostic for unopened mod file, got %d: %v", len(d), d)
	} else if !strings.Contains(d[0].Message, "nope") {
		t.Errorf("expected message to mention nope, got %q", d[0].Message)
	}

	// The vanilla file is never published (mod-only scope).
	if d, ok := got[pathToURI(vanillaEffect)]; ok {
		t.Errorf("expected no publish for vanilla file, got %v", d)
	}

	// Fix the reference on disk equivalent by defining the trait via the buffer,
	// then re-publish: the mod file's diagnostics must be cleared.
	s.mu.Lock()
	if err := s.proj.UpdateSource(modEffect, []byte("e = { add_trait = brave }\n")); err != nil {
		t.Fatal(err)
	}
	s.publishProjectDiagnostics(ctx.Notify)
	s.mu.Unlock()
	if d, ok := got[modURI]; !ok || len(d) != 0 {
		t.Errorf("expected cleared diagnostics for mod file after fix, got %v (present=%v)", d, ok)
	}
}

func TestOffsetToPosition(t *testing.T) {
	text := []byte("ab\ncdé f\n")
	// Line 1 is "cdé f". 'é' is 2 UTF-8 bytes but 1 UTF-16 unit, so the space
	// (byte offset 7) is character 3: c=0, d=1, é=2, space=3.
	pos := offsetToPosition(text, 7)
	if pos.Line != 1 || pos.Character != 3 {
		t.Errorf("got line=%d char=%d, want line=1 char=3", pos.Line, pos.Character)
	}
}

func TestPositionToOffset(t *testing.T) {
	text := []byte("ab\ncdé f\n")
	// Round-trip: offset → position → offset. Only test character boundaries;
	// mid-character offsets (inside a multi-byte rune) can't round-trip because
	// the LSP Position is character-based, not byte-based.
	for _, off := range []int{0, 1, 2, 3, 4, 5, 7, 8} {
		pos := offsetToPosition(text, off)
		got := positionToOffset(text, pos)
		if got != off {
			t.Errorf("round-trip: offset %d → pos {%d,%d} → offset %d", off, pos.Line, pos.Character, got)
		}
	}
	// A position past the end returns len(text).
	if got := positionToOffset(text, protocol.Position{Line: 99, Character: 0}); got != len(text) {
		t.Errorf("past-end position: want %d, got %d", len(text), got)
	}
}

func TestDefinition(t *testing.T) {
	dir := t.TempDir()
	traitPath := writeFile(t, dir, "common/traits/00_t.txt", "brave = { }\n")
	effectPath := writeFile(t, dir, "common/scripted_effects/00_e.txt",
		"e = { add_trait = brave }\n")

	cfg := config.Default()
	cfg.Cache.Enabled = false
	s := NewServer(Options{Config: cfg})
	if err := s.buildProject("", dir); err != nil {
		t.Fatal(err)
	}

	effectURI := pathToURI(effectPath)
	traitURI := pathToURI(traitPath)

	discard := map[string][]protocol.Diagnostic{}
	ctx := captureCtx(discard)

	// Open the effect file with the buffer so facts are built for it.
	if err := s.didOpen(ctx, &protocol.DidOpenTextDocumentParams{
		TextDocument: protocol.TextDocumentItem{URI: effectURI, Text: "e = { add_trait = brave }\n"},
	}); err != nil {
		t.Fatal(err)
	}

	// Cursor on 'b' of "brave" (byte 18, line 0 char 18).
	loc, err := s.definition(ctx, &protocol.DefinitionParams{
		TextDocumentPositionParams: protocol.TextDocumentPositionParams{
			TextDocument: protocol.TextDocumentIdentifier{URI: effectURI},
			Position:     protocol.Position{Line: 0, Character: 18},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if loc == nil {
		t.Fatal("expected a location for 'brave' reference, got nil")
	}
	location, ok := loc.(protocol.Location)
	if !ok {
		t.Fatalf("expected protocol.Location, got %T", loc)
	}
	if location.URI != traitURI {
		t.Errorf("expected URI %s, got %s", traitURI, location.URI)
	}
	if location.Range.Start.Line != 0 || location.Range.Start.Character != 0 {
		t.Errorf("expected range start {0,0}, got {%d,%d}",
			location.Range.Start.Line, location.Range.Start.Character)
	}
	if location.Range.End.Line != 0 || location.Range.End.Character != 5 {
		t.Errorf("expected range end {0,5}, got {%d,%d}",
			location.Range.End.Line, location.Range.End.Character)
	}

	// Cursor not on a reference (on 'e' at byte 0).
	loc, err = s.definition(ctx, &protocol.DefinitionParams{
		TextDocumentPositionParams: protocol.TextDocumentPositionParams{
			TextDocument: protocol.TextDocumentIdentifier{URI: effectURI},
			Position:     protocol.Position{Line: 0, Character: 0},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if loc != nil {
		t.Errorf("expected nil for cursor not on a reference, got %v", loc)
	}

	// Cursor on an unresolved reference.
	s.mu.Lock()
	s.docs[effectURI] = []byte("e = { add_trait = nope }\n")
	s.mu.Unlock()
	s.analyzeAndPublish(ctx.Notify, effectURI)

	// "nope" is at bytes 18-22.
	loc, err = s.definition(ctx, &protocol.DefinitionParams{
		TextDocumentPositionParams: protocol.TextDocumentPositionParams{
			TextDocument: protocol.TextDocumentIdentifier{URI: effectURI},
			Position:     protocol.Position{Line: 0, Character: 18},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if loc != nil {
		t.Errorf("expected nil for unresolved reference, got %v", loc)
	}
}

func TestReferences(t *testing.T) {
	dir := t.TempDir()
	traitPath := writeFile(t, dir, "common/traits/00_t.txt", "brave = { }\n")
	// Two files referencing brave (identical prefix → "brave" at char 18), one
	// referencing an undefined trait.
	eff1 := writeFile(t, dir, "common/scripted_effects/00_e1.txt", "e = { add_trait = brave }\n")
	eff2 := writeFile(t, dir, "common/scripted_effects/00_e2.txt", "f = { add_trait = brave }\n")
	writeFile(t, dir, "common/scripted_effects/00_e3.txt", "g = { add_trait = nope }\n")

	cfg := config.Default()
	cfg.Cache.Enabled = false
	s := NewServer(Options{Config: cfg})
	if err := s.buildProject("", dir); err != nil {
		t.Fatal(err)
	}

	eff1URI := pathToURI(eff1)
	eff2URI := pathToURI(eff2)
	traitURI := pathToURI(traitPath)
	ctx := &glsp.Context{}

	uriSet := func(locs []protocol.Location) map[string]int {
		m := map[string]int{}
		for _, l := range locs {
			m[l.URI]++
		}
		return m
	}

	// Cursor on the "brave" reference in eff1 (char 18); declaration excluded.
	locs, err := s.references(ctx, &protocol.ReferenceParams{
		TextDocumentPositionParams: protocol.TextDocumentPositionParams{
			TextDocument: protocol.TextDocumentIdentifier{URI: eff1URI},
			Position:     protocol.Position{Line: 0, Character: 18},
		},
		Context: protocol.ReferenceContext{IncludeDeclaration: false},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(locs) != 2 {
		t.Fatalf("expected 2 references, got %d: %v", len(locs), locs)
	}
	got := uriSet(locs)
	if got[eff1URI] != 1 || got[eff2URI] != 1 {
		t.Errorf("expected one ref in each effect file, got %v", got)
	}
	if got[traitURI] != 0 {
		t.Errorf("declaration should be excluded, got %v", got)
	}

	// With IncludeDeclaration the trait definition is appended.
	locs, err = s.references(ctx, &protocol.ReferenceParams{
		TextDocumentPositionParams: protocol.TextDocumentPositionParams{
			TextDocument: protocol.TextDocumentIdentifier{URI: eff1URI},
			Position:     protocol.Position{Line: 0, Character: 18},
		},
		Context: protocol.ReferenceContext{IncludeDeclaration: true},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(locs) != 3 || uriSet(locs)[traitURI] != 1 {
		t.Fatalf("expected 3 locations incl. declaration, got %d: %v", len(locs), locs)
	}

	// Cursor on the definition NAME "brave" in the traits file (char 0) finds the
	// same references.
	locs, err = s.references(ctx, &protocol.ReferenceParams{
		TextDocumentPositionParams: protocol.TextDocumentPositionParams{
			TextDocument: protocol.TextDocumentIdentifier{URI: traitURI},
			Position:     protocol.Position{Line: 0, Character: 0},
		},
		Context: protocol.ReferenceContext{IncludeDeclaration: false},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(locs) != 2 {
		t.Errorf("expected 2 references from the definition name, got %d: %v", len(locs), locs)
	}

	// Cursor on whitespace resolves no symbol.
	locs, err = s.references(ctx, &protocol.ReferenceParams{
		TextDocumentPositionParams: protocol.TextDocumentPositionParams{
			TextDocument: protocol.TextDocumentIdentifier{URI: eff1URI},
			Position:     protocol.Position{Line: 0, Character: 1},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if locs != nil {
		t.Errorf("expected nil for cursor on whitespace, got %v", locs)
	}
}
