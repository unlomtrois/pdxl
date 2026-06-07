package validate

import (
	"fmt"
	"strings"

	"pdxl/internal/cache"
	"pdxl/internal/files"
	"pdxl/internal/lexer"
	v3 "pdxl/internal/parser/v3"
)

// RefDiag is an unresolved-reference diagnostic.
type RefDiag struct {
	Loc string // "file:line:col"
	Msg string
}

func (d RefDiag) String() string { return d.Loc + ": " + d.Msg }

// Resolve walks every file in fs and reports references (per ck3RefRules) whose
// scalar value does not resolve to a defined symbol in tbl. Build must have run
// first to populate tbl.
func Resolve(tbl *SymbolTable, fs *files.FileSet, store *cache.Store) ([]RefDiag, error) {
	var diags []RefDiag
	walkErr := fs.Walk(func(e files.FileEntry) error {
		tree, err := parseEntry(e.FullPath, store)
		if err != nil {
			return err
		}
		resolveNode(tbl, tree, tree.Root(), e.FullPath, &diags)
		return nil
	})
	return diags, walkErr
}

// resolveNode recursively checks references in the subtree rooted at n.
func resolveNode(tbl *SymbolTable, tree *v3.Tree, n v3.Node, path string, diags *[]RefDiag) {
	if n.Kind == v3.KindField {
		children := tree.Children(n)
		if len(children) == 2 {
			key := children[0].Value(tree.Src)
			value := children[1]
			if kind, ok := ck3RefRules[key]; ok && value.Kind == v3.KindScalar {
				val := strings.Trim(value.Value(tree.Src), `"`) // names may be quoted
				// A '$' immediately after the value means it is the prefix of a
				// macro-interpolated identifier (e.g. education_$EDUCATION$_5),
				// which the lexer splits; only the prefix is captured here.
				concatMacro := int(value.SrcEnd) < len(tree.Src) && tree.Src[value.SrcEnd] == '$'
				if !concatMacro && !skipRefValue(val) {
					if _, found := tbl.Lookup(kind, val); !found {
						tok := lexer.Token{Start: int(value.SrcStart), End: int(value.SrcEnd)}
						*diags = append(*diags, RefDiag{
							Loc: tok.FormatPosition(path, tree.Src),
							Msg: fmt.Sprintf("unknown %s %q", kind, val),
						})
					}
				}
			}
		}
	}
	for _, child := range tree.Children(n) {
		resolveNode(tbl, tree, child, path, diags)
	}
}

// scopeKeywords are relative-scope references that may hold a trait at runtime;
// `has_trait = prev` etc. cannot be resolved without scope tracking.
var scopeKeywords = map[string]struct{}{
	"root": {}, "this": {}, "prev": {},
	"prevprev": {}, "prevprevprev": {}, "prevprevprevprev": {},
}

// skipRefValue reports whether a reference value should not be resolved:
// macro parameters ($X$), scope/data-function chains (foo:bar), relative-scope
// keywords (prev/root/...), and empties cannot be checked against the symbol
// table without deeper (scope) analysis.
func skipRefValue(val string) bool {
	if val == "" || strings.ContainsAny(val, "$:") {
		return true
	}
	_, isScope := scopeKeywords[val]
	return isScope
}
