package v2

// Fixture-based golden tests.
//
// Each .txt in testdata/ is parsed; the pretty-printed AST is compared
// against the corresponding .golden file.
//
// Regenerate goldens after a deliberate change:
//
//	go test ./internal/parser/... -update
//
// Fixture inventory (see testdata/ at project root):
//
//	advance                  — simple fields, potential trigger
//	government_reform        — modifier block, negative numbers (-0.25)
//	international_organizations — tagged block (rgb {…}), identifier lists
//	international_organization  — scope chains, ?= operator
//	modifier_types           — no-whitespace assignment, game_data block
//	parliament_types         — colon-prefixed type refs
//	special_statuses         — pipe paths (define:Name|CONSTANT)
//	subject_type             — negative float, scope:actor pattern

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

			ast, err := ParseBytes(fixturePath, src)
			if err != nil {
				t.Fatalf("parse error: %v", err)
			}

			got := renderFile(ast)
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

// renderFile pretty-prints a parsed File to a string for golden comparison.
func renderFile(f *File) string {
	var b bytes.Buffer
	for _, item := range f.Items {
		renderItem(&b, item, 0)
	}
	return b.String()
}

func renderItem(b *bytes.Buffer, item *Item, depth int) {
	if item.Field != nil {
		renderField(b, item.Field, depth)
	} else if item.Scalar != nil {
		fmt.Fprintf(b, "%s%s\n", indent(depth), item.Scalar.Value())
	}
}

func renderField(b *bytes.Buffer, f *Field, depth int) {
	pfx := indent(depth)
	switch v := f.Value.(type) {
	case *Scalar:
		fmt.Fprintf(b, "%s%s %s %s\n", pfx, f.Key(), f.Operator, v.Value())
	case *TaggedBlock:
		fmt.Fprintf(b, "%s%s %s %s {\n", pfx, f.Key(), f.Operator, v.Tag)
		for _, item := range v.Items {
			renderItem(b, item, depth+1)
		}
		fmt.Fprintf(b, "%s}\n", pfx)
	case *Block:
		fmt.Fprintf(b, "%s%s %s {\n", pfx, f.Key(), f.Operator)
		for _, item := range v.Items {
			renderItem(b, item, depth+1)
		}
		fmt.Fprintf(b, "%s}\n", pfx)
	}
}

func indent(depth int) string {
	return strings.Repeat("  ", depth)
}

