package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"pdxl/internal/cache"
	"pdxl/internal/lexer"
	v3 "pdxl/internal/parser/v3"
)

var lintCmd = &cobra.Command{
	Use:   "lint <file> [<file>...]",
	Short: "Check Paradox scripting files for structural errors",
	Args:  cobra.MinimumNArgs(1),
	RunE:  runLint,
}

var noCacheLint bool

func init() {
	lintCmd.Flags().BoolVar(&noCacheLint, "no-cache", false, "disable parse cache")
	rootCmd.AddCommand(lintCmd)
}

func runLint(_ *cobra.Command, args []string) error {
	var store *cache.Store
	if !noCacheLint {
		store, _ = cache.NewStore(".pdxl/cache", 256)
	}

	hasErrors := false
	for _, path := range args {
		info, err := os.Stat(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "%s: %v\n", path, err)
			hasErrors = true
			continue
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
				hasErrors = true
				continue
			}
			tree, diags = v3.Parse(path, src)
			if store != nil {
				_ = store.Put(path, info, src, tree, diags)
			}
		}

		for _, d := range diags {
			tok := lexer.Token{Start: d.Offset, End: d.Offset}
			fmt.Printf("%s: %s\n", tok.FormatPosition(d.Filename, tree.Src), d.Msg)
			if d.Severity == v3.SeverityError {
				hasErrors = true
			}
		}
	}
	if hasErrors {
		os.Exit(1)
	}
	return nil
}
