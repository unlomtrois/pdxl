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

func TestOffsetToPosition(t *testing.T) {
	text := []byte("ab\ncdé f\n")
	// Line 1 is "cdé f". 'é' is 2 UTF-8 bytes but 1 UTF-16 unit, so the space
	// (byte offset 7) is character 3: c=0, d=1, é=2, space=3.
	pos := offsetToPosition(text, 7)
	if pos.Line != 1 || pos.Character != 3 {
		t.Errorf("got line=%d char=%d, want line=1 char=3", pos.Line, pos.Character)
	}
}
