package main

import (
	"fmt"

	"github.com/spf13/cobra"
	"pdxl/internal/cache"
	"pdxl/internal/validate"
)

var checkCmd = &cobra.Command{
	Use:   "check",
	Short: "Index project definitions (scripted triggers, traits, events, ...)",
	Args:  cobra.NoArgs,
	RunE:  runCheck,
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

func runCheck(_ *cobra.Command, _ []string) error {
	gameDir := checkGame
	if gameDir == "" {
		gameDir = cfg.GamePath
	}
	modArg := checkMod
	if modArg == "" {
		modArg = cfg.ModPath
	}

	fs, err := buildProjectFileSet(gameDir, modArg, checkProtonPrefix)
	if err != nil {
		return err
	}

	var store *cache.Store
	if !noCacheCheck && cfg.Cache.Enabled {
		store, _ = cache.NewStore(cfg.Cache.Dir, cfg.Cache.LRUCap)
	}

	tbl, err := validate.Build(fs, store)
	if err != nil {
		return err
	}

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
	return nil
}
