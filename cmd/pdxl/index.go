package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"
	"pdxl/internal/files"
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

	if gameDir == "" && modArg == "" {
		return fmt.Errorf("provide --game and/or --mod (or set game_path/mod_path in pdxl.toml)")
	}

	// Resolve mod: .mod file or plain directory.
	var modDir string
	var mod files.Mod
	if modArg != "" {
		info, err := os.Stat(modArg)
		if err != nil {
			return fmt.Errorf("mod: %w", err)
		}
		if !info.IsDir() && strings.HasSuffix(strings.ToLower(modArg), ".mod") {
			mod, err = files.ParseMod(modArg)
			if err != nil {
				return fmt.Errorf("parsing .mod file: %w", err)
			}
			if files.IsWindowsAbsolute(mod.Path) {
				if indexProtonPrefix == "" {
					return fmt.Errorf("mod path %q is a Windows absolute path — provide --proton-prefix or use --mod <dir>", mod.Path)
				}
				modDir = files.ResolveWindowsPath(mod.Path, indexProtonPrefix)
			} else {
				modDir = mod.Path
			}
		} else {
			modDir = modArg
		}
	}

	var fs files.FileSet
	if len(mod.ReplacePaths) > 0 {
		fs.SetReplacePaths(mod.ReplacePaths)
	}
	if gameDir != "" {
		if err := fs.Add(gameDir, files.FileKindVanilla); err != nil {
			return fmt.Errorf("scanning game dir: %w", err)
		}
	}
	if modDir != "" {
		if err := fs.Add(modDir, files.FileKindMod); err != nil {
			return fmt.Errorf("scanning mod dir: %w", err)
		}
	}

	st := fs.Stats()
	fmt.Printf("vanilla  %5d files\n", st.Vanilla)
	fmt.Printf("mod      %5d files\n", st.Mod)
	fmt.Printf("total    %5d files", st.Total)
	if st.Shadowed > 0 || st.Replaced > 0 {
		fmt.Printf("  (%d shadowed, %d replaced)", st.Shadowed, st.Replaced)
	}
	fmt.Println()

	return nil
}
