package main

import (
	"log/slog"
	"os"

	"github.com/spf13/cobra"
	"pdxl/internal/config"
)

var rootCmd = &cobra.Command{
	Use:   "pdxl",
	Short: "Paradox scripting language toolkit",
}

var verbose bool
var configPath string
var cfg config.Config

func init() {
	rootCmd.PersistentFlags().BoolVarP(&verbose, "verbose", "v", false, "enable verbose logging")
	rootCmd.PersistentFlags().StringVar(&configPath, "config", "", "path to config file (default: pdxl.toml)")
	cobra.OnInitialize(initLogging, initConfig)
}

func initLogging() {
	level := slog.LevelWarn
	if verbose {
		level = slog.LevelDebug
	}
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: level})))
}

func initConfig() {
	path := configPath
	if path == "" {
		path = config.DefaultPath
	}
	var err error
	cfg, err = config.Load(path)
	if err != nil {
		slog.Warn("config load error", "path", path, "err", err)
	}
	if cfg.Game != "" {
		slog.Debug("config loaded", "game", cfg.Game, "path", path)
	}
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		os.Exit(1)
	}
}
