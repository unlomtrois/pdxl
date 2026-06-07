package main

import (
	"github.com/spf13/cobra"
	"github.com/tliron/commonlog"
	_ "github.com/tliron/commonlog/simple"

	"pdxl/internal/lsp"
)

var lspGame string

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
	rootCmd.AddCommand(lspCmd)
}

func runLSP(_ *cobra.Command, _ []string) error {
	// glsp logs via commonlog; send it to stderr so stdout stays the LSP channel.
	commonlog.Configure(1, nil)
	srv := lsp.NewServer(lsp.Options{Config: cfg, GamePath: lspGame})
	return srv.Run()
}
