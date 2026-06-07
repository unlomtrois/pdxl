package validate

import (
	"strings"

	"pdxl/internal/lexer"
	v3 "pdxl/internal/parser/v3"
)

// Ref is a reference to check against the symbol table. Name has any quotes
// stripped and Loc is the precomputed "file:line:col" of the value.
type Ref struct {
	Kind SymbolKind
	Name string
	Loc  string
}

// FileFacts is everything one file contributes to analysis: the definitions it
// declares, trait-group aliases, and the references that need resolving. It is
// deterministic from the file's content and path, so it can be cached per file.
type FileFacts struct {
	Defs    []Symbol // definitions (duplicate-tracked on merge)
	Aliases []Symbol // trait group / group_equivalence names (no dup tracking)
	Refs    []Ref    // filtered, resolvable references
}

// extractFacts walks a parsed file once, collecting its definitions, trait-group
// aliases, and references. relPath is the FileSet key (drives the def rule and
// the on_action gating). fullPath is the on-disk path used in reference Loc
// strings, so diagnostics point at a clickable file.
func extractFacts(tree *v3.Tree, relPath, fullPath string) FileFacts {
	var f FileFacts

	if rule, ok := ruleFor(relPath); ok {
		for _, node := range tree.Children(tree.Root()) {
			harvestDef(tree, node, rule, relPath, &f)
		}
	}

	onAction := strings.HasPrefix(relPath, OnActionDir)
	extractRefs(tree, tree.Root(), fullPath, onAction, &f.Refs)
	return f
}

// harvestDef records the definition (and any trait-group aliases) for a single
// top-level node, if it is a NAME = { ... } field.
func harvestDef(tree *v3.Tree, node v3.Node, rule defRule, relPath string, f *FileFacts) {
	if node.Kind != v3.KindField {
		return
	}
	children := tree.Children(node)
	if len(children) != 2 {
		return
	}
	key, value := children[0], children[1]
	// A definition has a block body; skips metadata like `namespace = x`.
	if value.Kind != v3.KindBlock && value.Kind != v3.KindTaggedBlock {
		return
	}
	seen := make(map[string]struct{})
	collectParams(tree, value, seen)
	f.Defs = append(f.Defs, Symbol{
		Name:   key.Value(tree.Src),
		Kind:   rule.kind,
		File:   relPath,
		Offset: int(node.SrcStart),
		Params: sortedKeys(seen),
	})

	// CK3 traits expose group / group_equivalence names as valid refs.
	if rule.kind == KindTrait {
		for _, gk := range []string{"group", "group_equivalence"} {
			if g := directFieldValue(tree, value, gk); g != "" {
				f.Aliases = append(f.Aliases, Symbol{Name: g, Kind: KindTrait, File: relPath, Offset: int(node.SrcStart)})
			}
		}
	}
}

// extractRefs recursively collects references from the subtree rooted at n.
// onAction enables list/weighted forms that apply only in on_action files.
func extractRefs(tree *v3.Tree, n v3.Node, path string, onAction bool, refs *[]Ref) {
	if n.Kind == v3.KindField {
		if children := tree.Children(n); len(children) == 2 {
			extractFieldRefs(tree, children[0].Value(tree.Src), children[1], path, onAction, refs)
		}
	}
	for _, child := range tree.Children(n) {
		extractRefs(tree, child, path, onAction, refs)
	}
}

// extractFieldRefs collects references from a single key = value field.
func extractFieldRefs(tree *v3.Tree, key string, value v3.Node, path string, onAction bool, refs *[]Ref) {
	// Scalar form: key = value.
	if kind, ok := ck3RefRules[key]; ok && value.Kind == v3.KindScalar {
		appendRef(tree, kind, value, path, refs)
	}
	// Block form carrying an id: key = { id = value ... }.
	if kind, ok := ck3BlockIDRefRules[key]; ok && value.Kind == v3.KindBlock {
		if idNode, ok := directFieldNode(tree, value, "id"); ok && idNode.Kind == v3.KindScalar {
			appendRef(tree, kind, idNode, path, refs)
		}
	}
	if !onAction || value.Kind != v3.KindBlock {
		return
	}
	// List form: key = { item item ... } — loose scalar items.
	if kind, ok := ck3ListRefRules[key]; ok {
		for _, item := range tree.Children(value) {
			if item.Kind == v3.KindScalar {
				appendRef(tree, kind, item, path, refs)
			}
		}
	}
	// Weighted form: key = { WEIGHT = id ... } — only numeric-keyed entries.
	if kind, ok := ck3WeightedRefRules[key]; ok {
		extractWeightedRefs(tree, kind, value, path, refs)
	}
}

// extractWeightedRefs collects event references from a weighted block like
// random_events = { 50 = ns.id ... }: only numeric-keyed entries are
// weight->event; word keys are config and a numeric value (0) means "no event".
func extractWeightedRefs(tree *v3.Tree, kind SymbolKind, block v3.Node, path string, refs *[]Ref) {
	for _, fld := range tree.Children(block) {
		if fld.Kind != v3.KindField {
			continue
		}
		kids := tree.Children(fld)
		if len(kids) != 2 || kids[1].Kind != v3.KindScalar {
			continue
		}
		if startsWithDigit(kids[0].Value(tree.Src)) && !startsWithDigit(kids[1].Value(tree.Src)) {
			appendRef(tree, kind, kids[1], path, refs)
		}
	}
}

// appendRef records a resolvable reference from a scalar value node, applying
// the quote-strip, macro-concatenation and scope/macro skips.
func appendRef(tree *v3.Tree, kind SymbolKind, value v3.Node, path string, refs *[]Ref) {
	val := strings.Trim(value.Value(tree.Src), `"`) // names may be quoted
	// A '$' immediately after the value means it is the prefix of a
	// macro-interpolated identifier (e.g. education_$EDUCATION$_5); the lexer
	// splits it, so only the prefix is captured.
	concatMacro := int(value.SrcEnd) < len(tree.Src) && tree.Src[value.SrcEnd] == '$'
	if concatMacro || skipRefValue(val) {
		return
	}
	tok := lexer.Token{Start: int(value.SrcStart), End: int(value.SrcEnd)}
	*refs = append(*refs, Ref{Kind: kind, Name: val, Loc: tok.FormatPosition(path, tree.Src)})
}
