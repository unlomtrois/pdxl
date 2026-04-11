package parser

// Fixture-based tests: parse each .txt in testdata/, pretty-print the AST,
// and compare against the corresponding .golden file.
//
// To regenerate all golden files after a deliberate AST/printer change:
//
//	go test ./internal/parser/... -update
//
// Each fixture is named for the PDXScript construct it exercises:
//
//	advance.txt               — simple key=value block, potential trigger
//	government_reform.txt     — modifier block, negative numbers (-0.25)
//	international_organizations.txt — tagged block (rgb {...}), identifier lists
//	international_organization.txt  — scope chains (c:GEN, scope:actor ?= {...})
//	modifier_types.txt        — no-whitespace assignment (key=value), game_data block
//	parliament_types.txt      — colon-prefixed type refs (parliament_type:foo)
//	special_statuses.txt      — pipe in path (define:Name|CONSTANT), color tagged block
//	subject_type.txt          — negative float (-0.1), scope:actor = {...} pattern

import (
	"bytes"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

var update = flag.Bool("update", false, "regenerate golden files instead of comparing")

func TestFixtures(t *testing.T) {
	fixtures, err := filepath.Glob("testdata/*.txt")
	if err != nil {
		t.Fatal(err)
	}
	if len(fixtures) == 0 {
		t.Fatal("no fixture files found in testdata/")
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
			goldenPath := filepath.Join("testdata", name+".golden")

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
					name, diffLines(got, string(want)))
			}
		})
	}
}

// renderFile pretty-prints a parsed File to a string — same logic as the CLI
// printer but returns a string so tests can compare without side effects.
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

// diffLines produces a simple line-by-line diff for test failure messages.
func diffLines(got, want string) string {
	gotLines := strings.Split(got, "\n")
	wantLines := strings.Split(want, "\n")
	var b bytes.Buffer
	max := len(gotLines)
	if len(wantLines) > max {
		max = len(wantLines)
	}
	shown := 0
	for i := 0; i < max && shown < 20; i++ {
		g, w := "", ""
		if i < len(gotLines) {
			g = gotLines[i]
		}
		if i < len(wantLines) {
			w = wantLines[i]
		}
		if g != w {
			fmt.Fprintf(&b, "line %d:\n  got:  %q\n  want: %q\n", i+1, g, w)
			shown++
		}
	}
	if shown == 0 {
		b.WriteString("(no line differences found — possibly trailing newline)")
	}
	return b.String()
}
