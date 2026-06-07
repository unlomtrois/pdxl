// Package lsp implements a Language Server for PDXScript, exposing the
// cross-file validator (internal/validate) as live editor diagnostics.
//
// Milestone 1: publish unresolved-reference diagnostics for open documents,
// updated incrementally as the user edits. The whole-project symbol table is
// held in memory (validate.Project) and a single file edit re-analyzes only
// that file.
package lsp

import (
	"log/slog"
	"path/filepath"
	"sync"
	"time"

	"github.com/tliron/glsp"
	protocol "github.com/tliron/glsp/protocol_3_16"
	glspserv "github.com/tliron/glsp/server"

	"pdxl/internal/cache"
	"pdxl/internal/config"
	"pdxl/internal/files"
	"pdxl/internal/validate"
)

// debounceDelay coalesces rapid edits before re-analyzing.
const debounceDelay = 200 * time.Millisecond

// Options configures the server.
type Options struct {
	Config   config.Config
	GamePath string // overrides Config.GamePath when non-empty (e.g. --game flag)
}

// Server is a single-connection LSP server. Its mutex guards the Project and
// the document store, which are not otherwise safe for concurrent use.
type Server struct {
	opts Options

	mu     sync.Mutex
	proj   *validate.Project
	docs   map[string][]byte       // document URI -> current text
	timers map[string]*time.Timer  // debounce timer per URI
}

// NewServer creates a server with the given options.
func NewServer(opts Options) *Server {
	return &Server{
		opts:   opts,
		docs:   make(map[string][]byte),
		timers: make(map[string]*time.Timer),
	}
}

// Run serves the LSP over stdio until the connection closes.
func (s *Server) Run() error {
	handler := protocol.Handler{
		Initialize:            s.initialize,
		Initialized:           func(*glsp.Context, *protocol.InitializedParams) error { return nil },
		Shutdown:              func(*glsp.Context) error { return nil },
		SetTrace:              func(*glsp.Context, *protocol.SetTraceParams) error { return nil },
		TextDocumentDidOpen:   s.didOpen,
		TextDocumentDidChange: s.didChange,
		TextDocumentDidSave:   s.didSave,
		TextDocumentDidClose:  s.didClose,
	}
	srv := glspserv.NewServer(&handler, "pdxl", false)
	return srv.RunStdio()
}

func (s *Server) initialize(_ *glsp.Context, params *protocol.InitializeParams) (any, error) {
	game := s.opts.GamePath
	if game == "" {
		game = s.opts.Config.GamePath
	}
	if opts, ok := params.InitializationOptions.(map[string]any); ok {
		if g, ok := opts["gamePath"].(string); ok && g != "" {
			game = g
		}
	}
	modDir := ""
	if params.RootURI != nil {
		modDir = uriToPath(*params.RootURI)
	}

	slog.Info("lsp: initializing", "game", game, "mod", modDir)
	if err := s.buildProject(game, modDir); err != nil {
		slog.Error("lsp: failed to build project", "err", err)
	}

	syncFull := protocol.TextDocumentSyncKindFull
	name := "pdxl"
	return protocol.InitializeResult{
		Capabilities: protocol.ServerCapabilities{TextDocumentSync: syncFull},
		ServerInfo:   &protocol.InitializeResultServerInfo{Name: name},
	}, nil
}

// buildProject scans game (vanilla) + modDir (mod) and builds the in-memory
// Project. Either path may be empty, but at least one must resolve to files.
func (s *Server) buildProject(game, modDir string) error {
	var fset files.FileSet
	fset.SetIgnore(s.opts.Config.Scan.IgnoreDirs, s.opts.Config.Scan.IgnoreFiles)
	if game != "" {
		if err := fset.Add(game, files.FileKindVanilla); err != nil {
			return err
		}
	}
	if modDir != "" {
		if err := fset.Add(modDir, files.FileKindMod); err != nil {
			return err
		}
	}

	var ast *cache.Store
	var fc *validate.FactStore
	if s.opts.Config.Cache.Enabled {
		ast, _ = cache.NewStore(s.opts.Config.Cache.Dir, s.opts.Config.Cache.LRUCap)
		fc, _ = validate.NewFactStore(filepath.Join(s.opts.Config.Cache.Dir, "symbols"))
	}

	proj, err := validate.NewProject(&fset, ast, fc)
	if err != nil {
		return err
	}
	s.mu.Lock()
	s.proj = proj
	s.mu.Unlock()
	slog.Info("lsp: project ready", "symbols", proj.Table().Total(), "diagnostics", len(proj.Diags()))
	return nil
}

func (s *Server) didOpen(ctx *glsp.Context, params *protocol.DidOpenTextDocumentParams) error {
	uri := params.TextDocument.URI
	s.mu.Lock()
	s.docs[uri] = []byte(params.TextDocument.Text)
	s.mu.Unlock()
	s.analyzeAndPublish(ctx.Notify, uri)
	return nil
}

func (s *Server) didChange(ctx *glsp.Context, params *protocol.DidChangeTextDocumentParams) error {
	text, ok := wholeText(params.ContentChanges)
	if !ok {
		return nil
	}
	uri := params.TextDocument.URI
	s.mu.Lock()
	s.docs[uri] = []byte(text)
	if t := s.timers[uri]; t != nil {
		t.Stop()
	}
	notify := ctx.Notify
	s.timers[uri] = time.AfterFunc(debounceDelay, func() { s.analyzeAndPublish(notify, uri) })
	s.mu.Unlock()
	return nil
}

func (s *Server) didSave(ctx *glsp.Context, params *protocol.DidSaveTextDocumentParams) error {
	s.analyzeAndPublish(ctx.Notify, params.TextDocument.URI)
	return nil
}

func (s *Server) didClose(ctx *glsp.Context, params *protocol.DidCloseTextDocumentParams) error {
	uri := params.TextDocument.URI
	s.mu.Lock()
	delete(s.docs, uri)
	if t := s.timers[uri]; t != nil {
		t.Stop()
		delete(s.timers, uri)
	}
	s.mu.Unlock()
	// Clear diagnostics for the closed document.
	ctx.Notify(protocol.ServerTextDocumentPublishDiagnostics, protocol.PublishDiagnosticsParams{
		URI: uri, Diagnostics: []protocol.Diagnostic{},
	})
	return nil
}

// analyzeAndPublish re-analyzes the changed document from its buffer and then
// republishes diagnostics for every open document (an edit to a definition can
// change references in other open files).
func (s *Server) analyzeAndPublish(notify glsp.NotifyFunc, changedURI string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.proj == nil {
		return
	}
	if text, ok := s.docs[changedURI]; ok {
		_ = s.proj.UpdateSource(uriToPath(changedURI), text)
	}
	for uri, text := range s.docs {
		s.publish(notify, uri, text)
	}
}

// publish sends diagnostics for one open document, converting validator byte
// ranges to UTF-16 LSP ranges using that document's text.
func (s *Server) publish(notify glsp.NotifyFunc, uri string, text []byte) {
	path := uriToPath(uri)
	severity := protocol.DiagnosticSeverityError
	source := "pdxl"
	var diags []protocol.Diagnostic
	for _, d := range s.proj.FileDiags(path) {
		diags = append(diags, protocol.Diagnostic{
			Range: protocol.Range{
				Start: offsetToPosition(text, d.Start),
				End:   offsetToPosition(text, d.End),
			},
			Severity: &severity,
			Source:   &source,
			Message:  d.Msg,
		})
	}
	notify(protocol.ServerTextDocumentPublishDiagnostics, protocol.PublishDiagnosticsParams{
		URI: uri, Diagnostics: diags,
	})
}

// wholeText extracts the full document text from a Full-sync change event.
func wholeText(changes []any) (string, bool) {
	if len(changes) == 0 {
		return "", false
	}
	switch c := changes[len(changes)-1].(type) {
	case protocol.TextDocumentContentChangeEventWhole:
		return c.Text, true
	case protocol.TextDocumentContentChangeEvent:
		return c.Text, true
	}
	return "", false
}
