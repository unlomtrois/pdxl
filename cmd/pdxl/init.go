package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"pdxl/internal/config"
)

var initCmd = &cobra.Command{
	Use:   "init",
	Short: "Create a pdxl.toml config file in the current directory",
	Args:  cobra.NoArgs,
	RunE:  runInit,
}

var initGame string
var initForce bool

func init() {
	initCmd.Flags().StringVar(&initGame, "game", "", "target game (e.g. ck3)")
	initCmd.Flags().BoolVar(&initForce, "force", false, "overwrite existing pdxl.toml")
	rootCmd.AddCommand(initCmd)
}

func runInit(_ *cobra.Command, _ []string) error {
	path := config.DefaultPath

	if _, err := os.Stat(path); err == nil && !initForce {
		return fmt.Errorf("%s already exists (use --force to overwrite)", path)
	}

	cfg := config.Default()
	cfg.Game = initGame

	if err := config.Write(path, cfg); err != nil {
		return fmt.Errorf("writing %s: %w", path, err)
	}

	fmt.Printf("created %s\n", path)
	if initGame == "" {
		fmt.Println("tip: set game = \"ck3\" (or another game) in pdxl.toml")
	}
	return nil
}
