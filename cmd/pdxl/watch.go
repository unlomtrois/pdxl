package main

import (
	"encoding/json"
	"fmt"
	"io/fs"
	"log/slog"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/fsnotify/fsnotify"
	"github.com/spf13/cobra"
	"pdxl/internal/cache"
	"pdxl/internal/validate"
)

var watchGame string
var watchMod string
var watchProtonPrefix string
var watchAddr string

var watchCmd = &cobra.Command{
	Use:   "watch",
	Short: "Run a persistent validator: watch the mod dir and serve diagnostics over HTTP",
	Long: "Build the game+mod symbol table once, watch the mod directory for changes " +
		"(re-analyzing only changed files), and expose diagnostics over an HTTP API:\n\n" +
		"  GET /diagnostics            all unresolved references (JSON)\n" +
		"  GET /diagnostics?file=PATH  only that file's\n" +
		"  GET /health                 liveness check",
	Args: cobra.NoArgs,
	RunE: runWatch,
}

func init() {
	watchCmd.Flags().StringVar(&watchGame, "game", "", "path to vanilla game directory (default: game_path from config)")
	watchCmd.Flags().StringVar(&watchMod, "mod", "", "path to mod directory or .mod file (default: mod_path from config)")
	watchCmd.Flags().StringVar(&watchProtonPrefix, "proton-prefix", "", "Proton/Wine prefix to resolve Windows paths")
	watchCmd.Flags().StringVar(&watchAddr, "addr", "127.0.0.1:7777", "HTTP listen address")
	rootCmd.AddCommand(watchCmd)
}

// watcher holds the persistent Project behind a mutex (Project is not safe for
// concurrent use) and serves it over HTTP.
type watcher struct {
	mu   sync.Mutex
	proj *validate.Project
}

func runWatch(_ *cobra.Command, _ []string) error {
	gameDir := watchGame
	if gameDir == "" {
		gameDir = cfg.GamePath
	}
	modArg := watchMod
	if modArg == "" {
		modArg = cfg.ModPath
	}
	modDir, _, err := resolveMod(modArg, watchProtonPrefix)
	if err != nil {
		return err
	}
	if modDir == "" {
		return fmt.Errorf("watch needs a --mod directory to watch")
	}

	fset, err := buildProjectFileSet(gameDir, modArg, watchProtonPrefix)
	if err != nil {
		return err
	}
	var ast *cache.Store
	var fc *validate.FactStore
	if cfg.Cache.Enabled {
		ast, _ = cache.NewStore(cfg.Cache.Dir, cfg.Cache.LRUCap)
		fc, _ = validate.NewFactStore(filepath.Join(cfg.Cache.Dir, "symbols"))
	}
	proj, err := validate.NewProject(fset, ast, fc)
	if err != nil {
		return err
	}
	w := &watcher{proj: proj}
	slog.Info("watch: project ready", "symbols", proj.Table().Total(), "diagnostics", len(proj.Diags()))

	go w.watchDir(modDir)

	http.HandleFunc("/health", func(rw http.ResponseWriter, _ *http.Request) { _, _ = fmt.Fprintln(rw, "ok") })
	http.HandleFunc("/diagnostics", w.handleDiagnostics)
	slog.Info("watch: serving", "addr", watchAddr)
	fmt.Fprintf(os.Stderr, "pdxl watch listening on http://%s (GET /diagnostics)\n", watchAddr)
	return http.ListenAndServe(watchAddr, nil)
}

func (w *watcher) handleDiagnostics(rw http.ResponseWriter, r *http.Request) {
	w.mu.Lock()
	var diags []validate.RefDiag
	if file := r.URL.Query().Get("file"); file != "" {
		if abs, err := filepath.Abs(file); err == nil {
			diags = w.proj.FileDiags(abs)
		}
	} else {
		diags = w.proj.Diags()
	}
	w.mu.Unlock()

	if diags == nil {
		diags = []validate.RefDiag{} // encode [] not null
	}
	rw.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(rw).Encode(diags)
}

// watchDir adds every subdirectory of root to an fsnotify watcher and
// re-analyzes a tracked file whenever it is written, with light debouncing.
func (w *watcher) watchDir(root string) {
	fsw, err := fsnotify.NewWatcher()
	if err != nil {
		slog.Error("watch: cannot create watcher", "err", err)
		return
	}
	defer func() { _ = fsw.Close() }()

	_ = filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err == nil && d.IsDir() && !strings.HasPrefix(d.Name(), ".") {
			_ = fsw.Add(path)
		}
		return nil
	})

	timers := make(map[string]*time.Timer)
	for {
		ev, ok := <-fsw.Events
		if !ok {
			return
		}
		if !ev.Op.Has(fsnotify.Write) && !ev.Op.Has(fsnotify.Create) {
			continue
		}
		if !strings.EqualFold(filepath.Ext(ev.Name), ".txt") {
			continue
		}
		path := ev.Name
		if t := timers[path]; t != nil {
			t.Stop()
		}
		timers[path] = time.AfterFunc(100*time.Millisecond, func() { w.reload(path) })
	}
}

// reload re-analyzes one changed file from disk under the lock.
func (w *watcher) reload(path string) {
	w.mu.Lock()
	defer w.mu.Unlock()
	if err := w.proj.Update(path); err != nil {
		slog.Debug("watch: skip", "path", path, "err", err)
		return
	}
	slog.Info("watch: reloaded", "path", path, "diagnostics", len(w.proj.Diags()))
}
