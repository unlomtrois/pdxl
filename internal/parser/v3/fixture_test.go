package v3

// Fixture-based golden tests for the flat-node-pool parser (Option A).
// Golden files are shared with v2 — output format is identical.
//
// Regenerate goldens:
//
//	go test ./internal/parser/... -update

import (
	"bytes"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pdxl/internal/testutil"
)

var update = flag.Bool("update", false, "regenerate golden files instead of comparing")

func TestFixtures(t *testing.T) {
	td := testutil.TestdataDir()
	fixtures, err := filepath.Glob(filepath.Join(td, "*.txt"))
	if err != nil {
		t.Fatal(err)
	}
	if len(fixtures) == 0 {
		t.Fatalf("no fixture files found in %s", td)
	}

	for _, fixturePath := range fixtures {
		name := strings.TrimSuffix(filepath.Base(fixturePath), ".txt")
		t.Run(name, func(t *testing.T) {
			src, err := os.ReadFile(fixturePath)
			if err != nil {
				t.Fatalf("reading fixture: %v", err)
			}

			tree, diags := Parse(fixturePath, src)
			if len(diags) > 0 {
				t.Fatalf("unexpected parse diagnostics: %v", diags)
			}

			got := renderTree(tree)
			goldenPath := filepath.Join(td, name+".golden")

			if *update {
				if err := os.WriteFile(goldenPath, []byte(got), 0644); err != nil {
					t.Fatalf("writing golden: %v", err)
				}
				t.Logf("updated %s", goldenPath)
				return
			}

			want, err := os.ReadFile(goldenPath)
			if err != nil {
				t.Fatalf("reading golden (run with -update to generate): %v", err)
			}

			if got != string(want) {
				t.Errorf("output mismatch for %s\ndiff (got vs want):\n%s",
					name, testutil.DiffLines(got, string(want)))
			}
		})
	}
}

// renderTree pretty-prints a Tree to a string matching the golden format.
func renderTree(tree *Tree) string {
	var b bytes.Buffer
	root := tree.Root()
	for _, child := range tree.Children(root) {
		renderNode(&b, tree, child, 0)
	}
	return b.String()
}

func renderNode(b *bytes.Buffer, tree *Tree, n Node, depth int) {
	pfx := strings.Repeat("  ", depth)
	switch n.Kind {
	case KindField:
		children := tree.Children(n)
		key := children[0].Value(tree.Src)
		op := n.OpString()
		val := children[1]
		switch val.Kind {
		case KindScalar:
			fmt.Fprintf(b, "%s%s %s %s\n", pfx, key, op, val.Value(tree.Src))
		case KindTaggedBlock:
			fmt.Fprintf(b, "%s%s %s %s {\n", pfx, key, op, val.Value(tree.Src))
			for _, item := range tree.Children(val) {
				renderNode(b, tree, item, depth+1)
			}
			fmt.Fprintf(b, "%s}\n", pfx)
		case KindBlock:
			fmt.Fprintf(b, "%s%s %s {\n", pfx, key, op)
			for _, item := range tree.Children(val) {
				renderNode(b, tree, item, depth+1)
			}
			fmt.Fprintf(b, "%s}\n", pfx)
		}
	case KindScalar:
		fmt.Fprintf(b, "%s%s\n", pfx, n.Value(tree.Src))
	case KindBlock:
		// bare block at top level (unusual)
		fmt.Fprintf(b, "%s{\n", pfx)
		for _, item := range tree.Children(n) {
			renderNode(b, tree, item, depth+1)
		}
		fmt.Fprintf(b, "%s}\n", pfx)
	}
}

