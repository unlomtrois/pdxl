package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"pdxl/internal/parser"
)

var parseJSON bool

var parseCmd = &cobra.Command{
	Use:   "parse <file>",
	Short: "Parse a Paradox scripting file and print the AST",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filename := args[0]
		data, err := os.ReadFile(filename)
		if err != nil {
			return fmt.Errorf("reading %s: %w", filename, err)
		}
		ast, err := parser.ParseBytes(filename, data)
		if err != nil {
			return fmt.Errorf("parse error: %w", err)
		}
		if parseJSON {
			enc := json.NewEncoder(os.Stdout)
			enc.SetIndent("", "  ")
			return enc.Encode(ast)
		}
		printFile(ast, 0)
		return nil
	},
}

func init() {
	parseCmd.Flags().BoolVar(&parseJSON, "json", false, "output AST as JSON")
	rootCmd.AddCommand(parseCmd)
}

// ── pretty printer ────────────────────────────────────────────────────────────

func printFile(f *parser.File, depth int) {
	for _, item := range f.Items {
		printItem(item, depth)
	}
}

func printItem(item *parser.Item, depth int) {
	indent := indentStr(depth)
	if item.Field != nil {
		printField(item.Field, depth)
	} else if item.Scalar != nil {
		fmt.Printf("%s%s\n", indent, item.Scalar.Value())
	}
}

func printField(f *parser.Field, depth int) {
	indent := indentStr(depth)
	switch v := f.Value.(type) {
	case *parser.Scalar:
		fmt.Printf("%s%s %s %s\n", indent, f.Key(), f.Operator, v.Value())
	case *parser.TaggedBlock:
		fmt.Printf("%s%s %s %s {\n", indent, f.Key(), f.Operator, v.Tag)
		for _, item := range v.Items {
			printItem(item, depth+1)
		}
		fmt.Printf("%s}\n", indent)
	case *parser.Block:
		fmt.Printf("%s%s %s {\n", indent, f.Key(), f.Operator)
		for _, item := range v.Items {
			printItem(item, depth+1)
		}
		fmt.Printf("%s}\n", indent)
	}
}

func indentStr(depth int) string {
	const tab = "  "
	s := ""
	for range depth {
		s += tab
	}
	return s
}
