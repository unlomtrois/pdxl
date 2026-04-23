package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"pdxl/internal/lexer"
	v3 "pdxl/internal/parser/v3"
)

var lintCmd = &cobra.Command{
	Use:   "lint <file> [<file>...]",
	Short: "Check Paradox scripting files for structural errors",
	Args:  cobra.MinimumNArgs(1),
	RunE:  runLint,
}

func init() {
	rootCmd.AddCommand(lintCmd)
}

func runLint(_ *cobra.Command, args []string) error {
	hasErrors := false
	for _, path := range args {
		src, err := os.ReadFile(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "%s: %v\n", path, err)
			hasErrors = true
			continue
		}
		_, diags := v3.Parse(path, src)
		for _, d := range diags {
			tok := lexer.Token{Start: d.Offset, End: d.Offset}
			fmt.Printf("%s: %s\n", tok.FormatPosition(d.Filename, src), d.Msg)
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
