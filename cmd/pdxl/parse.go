package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"
	parser "pdxl/internal/parser/v2"
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
		printFile(ast, data, 0)
		return nil
	},
}

func init() {
	parseCmd.Flags().BoolVar(&parseJSON, "json", false, "output AST as JSON")
	rootCmd.AddCommand(parseCmd)
}

// ── pretty printer ────────────────────────────────────────────────────────────

func printFile(f *parser.File, src []byte, depth int) {
	for _, item := range f.Items {
		printItem(item, src, depth)
	}
}

func printItem(item *parser.Item, src []byte, depth int) {
	ind := indentStr(depth)
	if item.Field != nil {
		printField(item.Field, src, depth)
	} else if item.Scalar != nil {
		fmt.Printf("%s%s\n", ind, item.Scalar.Value(src))
	}
}

func printField(f *parser.Field, src []byte, depth int) {
	ind := indentStr(depth)
	switch v := f.Value.(type) {
	case *parser.Scalar:
		fmt.Printf("%s%s %s %s\n", ind, f.Key(src), parser.OperatorString(f.Operator), v.Value(src))
	case *parser.TaggedBlock:
		fmt.Printf("%s%s %s %s {\n", ind, f.Key(src), parser.OperatorString(f.Operator), string(v.Tag.GetValue(src)))
		for _, item := range v.Items {
			printItem(item, src, depth+1)
		}
		fmt.Printf("%s}\n", ind)
	case *parser.Block:
		fmt.Printf("%s%s %s {\n", ind, f.Key(src), parser.OperatorString(f.Operator))
		for _, item := range v.Items {
			printItem(item, src, depth+1)
		}
		fmt.Printf("%s}\n", ind)
	}
}

func indentStr(depth int) string {
	return strings.Repeat("\t", depth)
}
