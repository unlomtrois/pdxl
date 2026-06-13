package main

import (
	"log/slog"
	"os"

	"github.com/spf13/cobra"
	"github.com/tliron/commonlog"
	_ "github.com/tliron/commonlog/simple"

	"pdxl/internal/lsp"
)

var lspGame string
var lspLogLevel string

var lspCmd = &cobra.Command{
	Use:   "lsp",
	Short: "Run the PDXScript language server (LSP over stdio)",
	Long: "Run the language server over stdio for editor integration. The client " +
		"sends the mod directory as the workspace root; the vanilla game path comes " +
		"from --game, the client's initializationOptions (gamePath), or game_path in " +
		"pdxl.toml.",
	Args: cobra.NoArgs,
	RunE: runLSP,
}

func init() {
	lspCmd.Flags().StringVar(&lspGame, "game", "", "path to vanilla game directory (default: game_path from config)")
	lspCmd.Flags().StringVar(&lspLogLevel, "log-level", "", "log level: debug, info, warn, error (default: info, or debug with --verbose)")
	// LSP clients append a transport flag (e.g. vscode-languageclient sends
	// --stdio for TransportKind.stdio). We always use stdio, so accept and
	// ignore it rather than erroring on an unknown flag.
	lspCmd.Flags().Bool("stdio", false, "use stdio transport (accepted for LSP clients; always on)")
	_ = lspCmd.Flags().MarkHidden("stdio")
	rootCmd.AddCommand(lspCmd)
}

func runLSP(_ *cobra.Command, _ []string) error {
	// Adjust slog level: --log-level takes precedence over --verbose (which
	// was already processed by initLogging in root.go).
	if lspLogLevel != "" {
		level := slog.LevelInfo
		switch lspLogLevel {
		case "debug":
			level = slog.LevelDebug
		case "info":
			level = slog.LevelInfo
		case "warn":
			level = slog.LevelWarn
		case "error":
			level = slog.LevelError
		default:
			slog.Warn("lsp: unknown --log-level, using info", "value", lspLogLevel)
		}
		slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: level})))
		slog.Info("lsp: log level set", "level", lspLogLevel)
	}

	// glsp logs via commonlog; send it to stderr so stdout stays the LSP channel.
	commonlog.Configure(1, nil)
	srv := lsp.NewServer(lsp.Options{Config: cfg, GamePath: lspGame})
	return srv.Run()
}
