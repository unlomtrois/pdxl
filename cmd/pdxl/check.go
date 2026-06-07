package main

import (
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/spf13/cobra"
	"pdxl/internal/cache"
	"pdxl/internal/files"
	"pdxl/internal/validate"
)

var checkCmd = &cobra.Command{
	Use:   "check [file]",
	Short: "Index project definitions and resolve references across game+mod",
	Long: "Index project definitions (scripted triggers, traits, events, ...) and " +
		"resolve cross-file references.\n\n" +
		"With no argument, reports counts, duplicates, and all unresolved references.\n" +
		"With a file argument, reports only that file's unresolved references, " +
		"resolved against the whole-project symbol table.",
	Args: cobra.MaximumNArgs(1),
	RunE: runCheck,
}

var checkGame string
var checkMod string
var checkProtonPrefix string
var noCacheCheck bool

func init() {
	checkCmd.Flags().StringVar(&checkGame, "game", "", "path to vanilla game directory (default: game_path from config)")
	checkCmd.Flags().StringVar(&checkMod, "mod", "", "path to mod directory or .mod file (default: mod_path from config)")
	checkCmd.Flags().StringVar(&checkProtonPrefix, "proton-prefix", "", "Proton/Wine prefix to resolve Windows paths")
	checkCmd.Flags().BoolVar(&noCacheCheck, "no-cache", false, "disable parse cache")
	rootCmd.AddCommand(checkCmd)
}

func runCheck(_ *cobra.Command, args []string) error {
	gameDir := checkGame
	if gameDir == "" {
		gameDir = cfg.GamePath
	}
	modArg := checkMod
	if modArg == "" {
		modArg = cfg.ModPath
	}

	slog.Debug("check: building project", "game", gameDir, "mod", modArg)
	fs, err := buildProjectFileSet(gameDir, modArg, checkProtonPrefix)
	if err != nil {
		return err
	}
	st := fs.Stats()
	slog.Debug("check: scanned project", "vanilla", st.Vanilla, "mod", st.Mod, "total", st.Total)

	var store *cache.Store
	var fc *validate.FactStore
	if !noCacheCheck && cfg.Cache.Enabled {
		store, _ = cache.NewStore(cfg.Cache.Dir, cfg.Cache.LRUCap)
		fc, _ = validate.NewFactStore(filepath.Join(cfg.Cache.Dir, "symbols"))
	} else {
		slog.Debug("check: cache disabled")
	}

	start := time.Now()
	tbl, refDiags, err := validate.Analyze(fs, store, fc)
	if err != nil {
		return err
	}
	slog.Debug("check: analysis complete", "duration", time.Since(start).Round(time.Millisecond))

	if len(args) == 1 {
		return reportFile(fs, refDiags, args[0])
	}
	return reportProject(tbl, refDiags)
}

// reportProject prints whole-project counts, duplicates, and unresolved refs.
func reportProject(tbl *validate.SymbolTable, refDiags []validate.RefDiag) error {
	for _, kind := range validate.Kinds {
		fmt.Printf("%-18s %6d\n", kind, tbl.Count(kind))
	}
	fmt.Printf("%-18s %6d\n", "total", tbl.Total())

	if len(tbl.Duplicates) > 0 {
		fmt.Printf("\n%d duplicate definitions:\n", len(tbl.Duplicates))
		for _, d := range tbl.Duplicates {
			fmt.Printf("  %s %q redefined in %s (first in %s)\n", d.Kind, d.Name, d.File, d.First.File)
		}
	}

	if len(refDiags) > 0 {
		fmt.Printf("\n%d unresolved references:\n", len(refDiags))
		for _, d := range refDiags {
			fmt.Printf("  %s\n", d)
		}
		os.Exit(1)
	}
	return nil
}

// reportFile prints only target's unresolved references, resolved against the
// whole-project table. target is matched to its project file by absolute path.
func reportFile(fs *files.FileSet, refDiags []validate.RefDiag, target string) error {
	fullPath, ok := projectPathOf(fs, target)
	if !ok {
		return fmt.Errorf("%s is not part of the scanned game/mod project", target)
	}
	prefix := fullPath + ":"
	n := 0
	for _, d := range refDiags {
		if strings.HasPrefix(d.Loc, prefix) {
			fmt.Printf("%s\n", d)
			n++
		}
	}
	slog.Debug("check: filtered to file", "file", fullPath, "unresolved", n, "project_unresolved", len(refDiags))
	if n > 0 {
		os.Exit(1)
	}
	return nil
}

// projectPathOf finds the FileSet entry matching target (by absolute path) and
// returns the FullPath used in diagnostics.
func projectPathOf(fs *files.FileSet, target string) (string, bool) {
	abs, err := filepath.Abs(target)
	if err != nil {
		return "", false
	}
	abs = filepath.Clean(abs)
	var found string
	_ = fs.Walk(func(e files.FileEntry) error {
		if ep, err := filepath.Abs(e.FullPath); err == nil && filepath.Clean(ep) == abs {
			found = e.FullPath
		}
		return nil
	})
	return found, found != ""
}
