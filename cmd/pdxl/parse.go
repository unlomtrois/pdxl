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
var parseTree bool

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
		switch {
		case parseJSON:
			enc := json.NewEncoder(os.Stdout)
			enc.SetIndent("", "  ")
			return enc.Encode(tree)
		case parseTree:
			printNodeTree(tree)
		default:
			printFlat(tree, 0)
		}
		return nil
	},
}

func init() {
	parseCmd.Flags().BoolVar(&parseJSON, "json", false, "output AST as JSON")
	parseCmd.Flags().BoolVar(&parseTree, "tree", false, "output AST as a labelled node tree")
	rootCmd.AddCommand(parseCmd)
}

// ── flat printer (default) ────────────────────────────────────────────────────

func printFlat(tree *v3.Tree, depth int) {
	root := tree.Root()
	for _, idx := range tree.ChildRefs(root) {
		printFlatNode(tree, tree.Nodes[idx], depth)
	}
}

func printFlatNode(tree *v3.Tree, n v3.Node, depth int) {
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
				printFlatNode(tree, tree.Nodes[idx], depth+1)
			}
			fmt.Printf("%s}\n", ind)
		case v3.KindBlock:
			fmt.Printf("%s%s %s {\n", ind, key, op)
			for _, idx := range tree.ChildRefs(val) {
				printFlatNode(tree, tree.Nodes[idx], depth+1)
			}
			fmt.Printf("%s}\n", ind)
		}
	case v3.KindScalar:
		fmt.Printf("%s%s\n", ind, n.Value(tree.Src))
	case v3.KindBlock:
		fmt.Printf("%s{\n", ind)
		for _, idx := range tree.ChildRefs(n) {
			printFlatNode(tree, tree.Nodes[idx], depth+1)
		}
		fmt.Printf("%s}\n", ind)
	}
}

// ── tree printer (--tree) ─────────────────────────────────────────────────────

func printNodeTree(tree *v3.Tree) {
	root := tree.Root()
	fmt.Println("Root (KindFile)")
	refs := tree.ChildRefs(root)
	for i, idx := range refs {
		last := i == len(refs)-1
		printTreeNode(tree, tree.Nodes[idx], "", last)
	}
}

func printTreeNode(tree *v3.Tree, n v3.Node, prefix string, last bool) {
	branch := "├── "
	childPrefix := prefix + "│   "
	if last {
		branch = "└── "
		childPrefix = prefix + "    "
	}

	switch n.Kind {
	case v3.KindScalar:
		fmt.Printf("%s%sKindScalar  %q\n", prefix, branch, n.Value(tree.Src))

	case v3.KindField:
		children := tree.Children(n)
		key := children[0].Value(tree.Src)
		op := n.OpString()
		val := children[1]
		fmt.Printf("%s%sKindField   key=%q  op=%q\n", prefix, branch, key, op)
		printTreeNode(tree, val, childPrefix, true)

	case v3.KindBlock:
		fmt.Printf("%s%sKindBlock\n", prefix, branch)
		refs := tree.ChildRefs(n)
		for i, idx := range refs {
			printTreeNode(tree, tree.Nodes[idx], childPrefix, i == len(refs)-1)
		}

	case v3.KindTaggedBlock:
		fmt.Printf("%s%sKindTaggedBlock  tag=%q\n", prefix, branch, n.Value(tree.Src))
		refs := tree.ChildRefs(n)
		for i, idx := range refs {
			printTreeNode(tree, tree.Nodes[idx], childPrefix, i == len(refs)-1)
		}
	}
}
