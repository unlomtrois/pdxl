// Package lsp implements a Language Server for PDXScript, exposing the
// cross-file validator (internal/validate) as live editor diagnostics.
//
// Milestone 1: publish unresolved-reference diagnostics for open documents,
// updated incrementally as the user edits. The whole-project symbol table is
// held in memory (validate.Project) and a single file edit re-analyzes only
// that file.
package lsp

import (
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
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

	initGame string // game path from initialize (for deferred build)
	initMod  string // mod dir from initialize

	mu        sync.Mutex
	notify    glsp.NotifyFunc        // set from initialized; used when re-publishing async
	proj      *validate.Project
	docs      map[string][]byte      // document URI -> current text
	timers    map[string]*time.Timer // debounce timer per URI
	published map[string]struct{}    // full paths that currently have diagnostics shown
}

// NewServer creates a server with the given options.
func NewServer(opts Options) *Server {
	return &Server{
		opts:      opts,
		docs:      make(map[string][]byte),
		timers:    make(map[string]*time.Timer),
		published: make(map[string]struct{}),
	}
}

// Run serves the LSP over stdio until the connection closes.
func (s *Server) Run() error {
	slog.Info("lsp: server starting")
	handler := protocol.Handler{
		Initialize:             s.initialize,
		Initialized:            s.initialized,
		Shutdown:               func(*glsp.Context) error { return nil },
		SetTrace:               func(*glsp.Context, *protocol.SetTraceParams) error { return nil },
		TextDocumentDidOpen:    s.didOpen,
		TextDocumentDidChange:  s.didChange,
		TextDocumentDidSave:    s.didSave,
		TextDocumentDidClose:   s.didClose,
		TextDocumentDefinition: s.definition,
	}
	srv := glspserv.NewServer(&handler, "pdxl", false)
	err := srv.RunStdio()
	slog.Info("lsp: server stopped")
	return err
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

	clientName, clientVersion := "", ""
	if params.ClientInfo != nil {
		clientName = params.ClientInfo.Name
		if params.ClientInfo.Version != nil {
			clientVersion = *params.ClientInfo.Version
		}
	}
	slog.Info("lsp: initialize",
		"game", game,
		"mod", modDir,
		"clientName", clientName,
		"clientVersion", clientVersion,
	)

	// Store for the async build in initialized; we must respond to initialize
	// quickly per the LSP spec (~10s timeout on some clients).
	s.initGame = game
	s.initMod = modDir

	syncFull := protocol.TextDocumentSyncKindFull
	name := "pdxl"
	return protocol.InitializeResult{
		Capabilities: protocol.ServerCapabilities{
			TextDocumentSync:   syncFull,
			DefinitionProvider: true,
		},
		ServerInfo: &protocol.InitializeResultServerInfo{Name: name},
	}, nil
}

// initialized is called by the client after a successful initialize
// handshake. Builds the project asynchronously (not in initialize) so the
// client doesn't time out on large game corpora. After the build completes,
// re-publishes diagnostics for any documents that were opened while the
// project was building.
func (s *Server) initialized(ctx *glsp.Context, _ *protocol.InitializedParams) error {
	s.mu.Lock()
	s.notify = ctx.Notify
	s.mu.Unlock()

	go func() {
		slog.Info("lsp: building project", "game", s.initGame, "mod", s.initMod)
		if err := s.buildProject(s.initGame, s.initMod); err != nil {
			slog.Error("lsp: failed to build project", "err", err)
			return
		}

		s.mu.Lock()
		defer s.mu.Unlock()
		slog.Info("lsp: project ready",
			"symbols", s.proj.Table().Total(),
			"diagnostics", len(s.proj.Diags()),
			"openDocs", len(s.docs),
		)

		// Re-analyze any documents opened while the project was still building
		// (didOpen would have skipped them) so their buffers override disk.
		for uri, text := range s.docs {
			if err := s.proj.UpdateSource(uriToPath(uri), text); err != nil {
				slog.Warn("lsp: post-build UpdateSource failed", "uri", uri, "err", err)
				continue
			}
		}
		// Publish diagnostics for every mod file, opened or not.
		s.publishProjectDiagnostics(s.notify)
	}()
	return nil
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
	return nil
}

func (s *Server) didOpen(ctx *glsp.Context, params *protocol.DidOpenTextDocumentParams) error {
	uri := params.TextDocument.URI
	slog.Debug("lsp: didOpen", "uri", uri)
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
	slog.Debug("lsp: didClose", "uri", uri)
	s.mu.Lock()
	defer s.mu.Unlock()
	delete(s.docs, uri)
	if t := s.timers[uri]; t != nil {
		t.Stop()
		delete(s.timers, uri)
	}
	// Re-analyze from disk: the closed buffer may have differed from disk, so the
	// file reverts to its on-disk diagnostics rather than being force-cleared.
	if s.proj != nil {
		if err := s.proj.Update(uriToPath(uri)); err != nil {
			slog.Debug("lsp: didClose Update failed", "uri", uri, "err", err)
		}
	}
	s.publishProjectDiagnostics(ctx.Notify)
	return nil
}

// analyzeAndPublish re-analyzes the changed document from its buffer and then
// republishes diagnostics for every mod file (an edit to a definition can change
// references in other files, opened or not).
func (s *Server) analyzeAndPublish(notify glsp.NotifyFunc, changedURI string) {
	slog.Debug("lsp: analyzing", "uri", changedURI)
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.proj == nil {
		slog.Debug("lsp: skipping analysis, project not ready", "uri", changedURI)
		return
	}
	if text, ok := s.docs[changedURI]; ok {
		if err := s.proj.UpdateSource(uriToPath(changedURI), text); err != nil {
			slog.Warn("lsp: UpdateSource failed", "uri", changedURI, "err", err)
		}
	}
	s.publishProjectDiagnostics(notify)
	slog.Debug("lsp: analysis complete", "changed", changedURI, "openDocs", len(s.docs))
}

// publishProjectDiagnostics publishes unresolved-reference diagnostics for every
// mod file that has them, clears files that no longer do, and tracks the current
// set in s.published. Vanilla files are analyzed but never flagged. Must be called
// with s.mu held.
func (s *Server) publishProjectDiagnostics(notify glsp.NotifyFunc) {
	if s.proj == nil {
		return
	}
	// Group diagnostics by file, keeping only mod files.
	byFile := make(map[string][]validate.RefDiag)
	for _, d := range s.proj.Diags() {
		if !s.underModRoot(d.File) {
			continue
		}
		byFile[d.File] = append(byFile[d.File], d)
	}

	totalDiags := 0
	for file, fileDiags := range byFile {
		text, err := s.readFileLocked(file)
		if err != nil {
			slog.Warn("lsp: failed to read file for diagnostics", "path", file, "err", err)
			continue
		}
		diags := toLSPDiagnostics(fileDiags, text)
		notify(protocol.ServerTextDocumentPublishDiagnostics, protocol.PublishDiagnosticsParams{
			URI: pathToURI(file), Diagnostics: diags,
		})
		totalDiags += len(diags)
	}

	// Clear files that had diagnostics last cycle but no longer do.
	for file := range s.published {
		if _, ok := byFile[file]; ok {
			continue
		}
		notify(protocol.ServerTextDocumentPublishDiagnostics, protocol.PublishDiagnosticsParams{
			URI: pathToURI(file), Diagnostics: []protocol.Diagnostic{},
		})
	}

	// Record the new set of files with diagnostics.
	next := make(map[string]struct{}, len(byFile))
	for file := range byFile {
		next[file] = struct{}{}
	}
	s.published = next
	slog.Debug("lsp: published project diagnostics", "files", len(byFile), "diags", totalDiags)
}

// underModRoot reports whether fullPath lives under the mod root. An empty mod
// root (no workspace / tests) treats every file as in-scope.
func (s *Server) underModRoot(fullPath string) bool {
	if s.initMod == "" {
		return true
	}
	root := filepath.Clean(s.initMod)
	p := filepath.Clean(fullPath)
	if p == root {
		return true
	}
	return strings.HasPrefix(p, root+string(filepath.Separator))
}

// toLSPDiagnostics converts validator byte ranges to UTF-16 LSP diagnostics
// using the file's text.
func toLSPDiagnostics(refDiags []validate.RefDiag, text []byte) []protocol.Diagnostic {
	severity := protocol.DiagnosticSeverityError
	source := "pdxl"
	var diags []protocol.Diagnostic
	for _, d := range refDiags {
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
	return diags
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

// definition handles textDocument/definition requests. It finds the reference
// at the cursor position, looks up its definition in the symbol table, and
// returns the definition location.
func (s *Server) definition(_ *glsp.Context, params *protocol.DefinitionParams) (any, error) {
	path := uriToPath(params.TextDocument.URI)
	slog.Debug("lsp: definition request", "uri", params.TextDocument.URI,
		"line", params.Position.Line, "char", params.Position.Character)

	s.mu.Lock()
	if s.proj == nil {
		s.mu.Unlock()
		slog.Debug("lsp: definition skipped, project not ready")
		return nil, nil
	}
	facts, ok := s.proj.FactsAt(path)
	s.mu.Unlock()
	if !ok {
		slog.Debug("lsp: definition, file not in project", "path", path)
		return nil, nil
	}

	// Get the source text (prefer in-memory buffer).
	src, err := s.readFile(path)
	if err != nil {
		slog.Warn("lsp: definition, failed to read source", "path", path, "err", err)
		return nil, nil
	}

	off := positionToOffset(src, params.Position)

	// Find the reference that spans the cursor position.
	var ref *validate.Ref
	for i := range facts.Refs {
		r := &facts.Refs[i]
		if r.Start <= off && off < r.End {
			ref = r
			break
		}
	}
	if ref == nil {
		slog.Debug("lsp: definition, no reference at position",
			"path", path, "offset", off, "totalRefs", len(facts.Refs))
		return nil, nil
	}

	slog.Debug("lsp: definition, found reference",
		"kind", ref.Kind.String(), "name", ref.Name,
		"range", fmt.Sprintf("%d-%d", ref.Start, ref.End))

	// Look up the definition.
	s.mu.Lock()
	sym, found := s.proj.Table().Lookup(ref.Kind, ref.Name)
	s.mu.Unlock()
	if !found {
		slog.Debug("lsp: definition, unresolved",
			"kind", ref.Kind.String(), "name", ref.Name)
		return nil, nil
	}

	// Resolve the definition's relative path to a full path.
	defFull, ok := s.proj.RelToFull(sym.File)
	if !ok {
		slog.Warn("lsp: definition, failed to resolve rel path",
			"relPath", sym.File)
		return nil, nil
	}

	// Read the definition file to convert byte offsets to LSP positions.
	defSrc, err := s.readFile(defFull)
	if err != nil {
		slog.Warn("lsp: definition, failed to read definition file",
			"path", defFull, "err", err)
		return nil, nil
	}

	slog.Debug("lsp: definition, resolved",
		"name", sym.Name, "kind", sym.Kind.String(),
		"file", sym.File, "offset", sym.Offset)

	return protocol.Location{
		URI: pathToURI(defFull),
		Range: protocol.Range{
			Start: offsetToPosition(defSrc, sym.Offset),
			End:   offsetToPosition(defSrc, sym.EndOffset),
		},
	}, nil
}

// readFile returns the content of the file at path, preferring the in-memory
// editor buffer when available, falling back to disk.
func (s *Server) readFile(path string) ([]byte, error) {
	s.mu.Lock()
	text, ok := s.docs[pathToURI(path)]
	s.mu.Unlock()
	if ok {
		return text, nil
	}
	return os.ReadFile(path)
}

// readFileLocked is readFile for callers that already hold s.mu (the mutex is
// not reentrant).
func (s *Server) readFileLocked(path string) ([]byte, error) {
	if text, ok := s.docs[pathToURI(path)]; ok {
		return text, nil
	}
	return os.ReadFile(path)
}
