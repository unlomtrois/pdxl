package main

import (
	"fmt"
	"log/slog"
	"os"
	"strings"
	"time"

	"github.com/schollz/progressbar/v3"
	"github.com/spf13/cobra"
	"pdxl/internal/cache"
	"pdxl/internal/files"
	v3 "pdxl/internal/parser/v3"
)

var indexCmd = &cobra.Command{
	Use:   "index",
	Short: "Scan game and mod files and report statistics",
	Args:  cobra.NoArgs,
	RunE:  runIndex,
}

var indexGame string
var indexMod string
var indexDry bool
var indexProtonPrefix string

func init() {
	indexCmd.Flags().StringVar(&indexGame, "game", "", "path to vanilla game directory (default: game_path from config)")
	indexCmd.Flags().StringVar(&indexMod, "mod", "", "path to mod directory or .mod file (default: mod_path from config)")
	indexCmd.Flags().BoolVar(&indexDry, "dry", false, "scan only, do not parse")
	indexCmd.Flags().StringVar(&indexProtonPrefix, "proton-prefix", "", "Proton/Wine prefix to resolve Windows paths (e.g. ~/.steam/steamapps/compatdata/1158310/pfx)")
	rootCmd.AddCommand(indexCmd)
}

func runIndex(cmd *cobra.Command, _ []string) error {
	gameDir := indexGame
	if gameDir == "" {
		gameDir = cfg.GamePath
	}
	modArg := indexMod
	if modArg == "" {
		modArg = cfg.ModPath
	}

	fs, err := buildProjectFileSet(gameDir, modArg, indexProtonPrefix)
	if err != nil {
		return err
	}

	st := fs.Stats()
	fmt.Printf("vanilla  %5d files\n", st.Vanilla)
	fmt.Printf("mod      %5d files\n", st.Mod)
	fmt.Printf("total    %5d files", st.Total)
	if st.Shadowed > 0 || st.Replaced > 0 {
		fmt.Printf("  (%d shadowed, %d replaced)", st.Shadowed, st.Replaced)
	}
	fmt.Println()

	if indexDry {
		return nil
	}

	return parseAll(fs, st.Total)
}

// buildProjectFileSet resolves a game directory and a mod (.mod file or plain
// directory), applies overlay/ignore rules, and scans both into a FileSet.
// gameDir and modArg may be empty; at least one must be set.
func buildProjectFileSet(gameDir, modArg, protonPrefix string) (*files.FileSet, error) {
	if gameDir == "" && modArg == "" {
		return nil, fmt.Errorf("provide --game and/or --mod (or set game_path/mod_path in pdxl.toml)")
	}

	// Resolve mod: .mod file or plain directory.
	var modDir string
	var mod files.Mod
	if modArg != "" {
		info, err := os.Stat(modArg)
		if err != nil {
			return nil, fmt.Errorf("mod: %w", err)
		}
		if !info.IsDir() && strings.HasSuffix(strings.ToLower(modArg), ".mod") {
			mod, err = files.ParseMod(modArg)
			if err != nil {
				return nil, fmt.Errorf("parsing .mod file: %w", err)
			}
			if files.IsWindowsAbsolute(mod.Path) {
				if protonPrefix == "" {
					return nil, fmt.Errorf("mod path %q is a Windows absolute path — provide --proton-prefix or use --mod <dir>", mod.Path)
				}
				modDir = files.ResolveWindowsPath(mod.Path, protonPrefix)
			} else {
				modDir = mod.Path
			}
		} else {
			modDir = modArg
		}
	}

	fs := &files.FileSet{}
	fs.SetIgnore(cfg.Scan.IgnoreDirs, cfg.Scan.IgnoreFiles)
	if len(mod.ReplacePaths) > 0 {
		fs.SetReplacePaths(mod.ReplacePaths)
	}
	if gameDir != "" {
		if err := fs.Add(gameDir, files.FileKindVanilla); err != nil {
			return nil, fmt.Errorf("scanning game dir: %w", err)
		}
	}
	if modDir != "" {
		if err := fs.Add(modDir, files.FileKindMod); err != nil {
			return nil, fmt.Errorf("scanning mod dir: %w", err)
		}
	}
	return fs, nil
}

// parseAll parses every winning entry in the FileSet and reports how many
// files contained diagnostics, plus the total diagnostic count. total is the
// number of winning entries, used to size the progress bar.
func parseAll(fs *files.FileSet, total int) error {
	var store *cache.Store
	if cfg.Cache.Enabled {
		store, _ = cache.NewStore(cfg.Cache.Dir, cfg.Cache.LRUCap)
	}

	bar := progressbar.NewOptions(total,
		progressbar.OptionSetDescription("parsing"),
		progressbar.OptionSetWriter(os.Stderr),
		progressbar.OptionShowCount(),
		progressbar.OptionThrottle(65*time.Millisecond),
		progressbar.OptionClearOnFinish(),
	)

	var parsed, filesWithErrors, totalDiags int
	walkErr := fs.Walk(func(e files.FileEntry) error {
		_ = bar.Add(1)
		path := e.FullPath
		info, err := os.Stat(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "%s: %v\n", path, err)
			return nil
		}

		var tree *v3.Tree
		var diags []v3.Diagnostic
		if store != nil {
			tree, diags, _ = store.Get(path, info)
		}
		if tree == nil {
			src, err := os.ReadFile(path)
			if err != nil {
				fmt.Fprintf(os.Stderr, "%s: %v\n", path, err)
				return nil
			}
			tree, diags = v3.Parse(path, src)
			if store != nil {
				_ = store.Put(path, info, src, tree, diags)
			}
		}

		parsed++
		if len(diags) > 0 {
			filesWithErrors++
			totalDiags += len(diags)
			slog.Debug("diagnostics", "path", path, "count", len(diags))
		}
		return nil
	})
	if walkErr != nil {
		return walkErr
	}
	_ = bar.Finish()

	fmt.Printf("parsed   %5d files  (%d with errors, %d diagnostics total)\n",
		parsed, filesWithErrors, totalDiags)
	return nil
}
