package validate

import (
	"fmt"
	"strings"
)

// RefDiag is an unresolved-reference diagnostic.
type RefDiag struct {
	Loc string // "file:line:col"
	Msg string
}

func (d RefDiag) String() string { return d.Loc + ": " + d.Msg }

// resolveRefs checks each gathered reference against the completed table and
// returns a diagnostic for every one that does not resolve.
func resolveRefs(tbl *SymbolTable, refs []Ref) []RefDiag {
	var diags []RefDiag
	for _, r := range refs {
		if _, ok := tbl.Lookup(r.Kind, r.Name); !ok {
			diags = append(diags, RefDiag{
				Loc: r.Loc,
				Msg: fmt.Sprintf("unknown %s %q", r.Kind, r.Name),
			})
		}
	}
	return diags
}

// startsWithDigit reports whether s begins with an ASCII digit (a weight or
// config number; event IDs start with a namespace letter).
func startsWithDigit(s string) bool {
	return s != "" && s[0] >= '0' && s[0] <= '9'
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
