package main

import (
	"bytes"
	"fmt"
	"log/slog"
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
var contextLines int

func init() {
	lintCmd.Flags().BoolVar(&noCacheLint, "no-cache", false, "disable parse cache")
	lintCmd.Flags().IntVar(&contextLines, "context", 0, "lines of source context to print around each diagnostic (0 = off)")
	rootCmd.AddCommand(lintCmd)
}

func runLint(cmd *cobra.Command, args []string) error {
	if !cmd.Flags().Changed("context") {
		contextLines = cfg.Lint.Context
	}

	var store *cache.Store
	if !noCacheLint && cfg.Cache.Enabled {
		store, _ = cache.NewStore(cfg.Cache.Dir, cfg.Cache.LRUCap)
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
			if tree != nil {
				slog.Debug("cache hit", "path", path)
			}
		}

		if tree == nil {
			slog.Debug("cache miss, parsing", "path", path)
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
			if contextLines > 0 {
				printContext(tree.Src, d.Offset, contextLines)
			}
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

// printContext prints up to n lines before and after the line containing offset.
func printContext(src []byte, offset, n int) {
	lines := bytes.Split(src, []byte("\n"))

	// find which line the offset falls on (0-based)
	diagLine := 0
	pos := 0
	for i, line := range lines {
		pos += len(line) + 1 // +1 for the '\n'
		if pos > offset {
			diagLine = i
			break
		}
	}

	first := diagLine - n
	if first < 0 {
		first = 0
	}
	last := diagLine + n
	if last >= len(lines) {
		last = len(lines) - 1
	}

	for i := first; i <= last; i++ {
		marker := "  "
		if i == diagLine {
			marker = "> "
		}
		fmt.Printf("  %s%4d | %s\n", marker, i+1, lines[i])
	}
	fmt.Println()
}
