package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"
	v3 "pdxl/internal/parser/v3"
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
		tree, diags := v3.Parse(filename, data)
		for _, d := range diags {
			fmt.Fprintf(os.Stderr, "%s\n", d.String())
		}
		if parseJSON {
			enc := json.NewEncoder(os.Stdout)
			enc.SetIndent("", "  ")
			return enc.Encode(tree)
		}
		printTree(tree, 0)
		return nil
	},
}

func init() {
	parseCmd.Flags().BoolVar(&parseJSON, "json", false, "output AST as JSON")
	rootCmd.AddCommand(parseCmd)
}

func printTree(tree *v3.Tree, depth int) {
	root := tree.Root()
	for _, idx := range tree.ChildRefs(root) {
		printNode(tree, tree.Nodes[idx], depth)
	}
}

func printNode(tree *v3.Tree, n v3.Node, depth int) {
	ind := strings.Repeat("\t", depth)
	switch n.Kind {
	case v3.KindField:
		children := tree.Children(n)
		key := children[0].Value(tree.Src)
		op := n.OpString()
		val := children[1]
		switch val.Kind {
		case v3.KindScalar:
			fmt.Printf("%s%s %s %s\n", ind, key, op, val.Value(tree.Src))
		case v3.KindTaggedBlock:
			fmt.Printf("%s%s %s %s {\n", ind, key, op, val.Value(tree.Src))
			for _, idx := range tree.ChildRefs(val) {
				printNode(tree, tree.Nodes[idx], depth+1)
			}
			fmt.Printf("%s}\n", ind)
		case v3.KindBlock:
			fmt.Printf("%s%s %s {\n", ind, key, op)
			for _, idx := range tree.ChildRefs(val) {
				printNode(tree, tree.Nodes[idx], depth+1)
			}
			fmt.Printf("%s}\n", ind)
		}
	case v3.KindScalar:
		fmt.Printf("%s%s\n", ind, n.Value(tree.Src))
	case v3.KindBlock:
		fmt.Printf("%s{\n", ind)
		for _, idx := range tree.ChildRefs(n) {
			printNode(tree, tree.Nodes[idx], depth+1)
		}
		fmt.Printf("%s}\n", ind)
	}
}
