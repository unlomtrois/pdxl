// Package validate provides semantic indexing of PDXScript across a whole
// project. Phase 0 collects definitions (scripted triggers, traits, events,
// ...) into a SymbolTable; reference resolution is a future phase.
package validate

import (
	"os"
	"regexp"
	"sort"

	"pdxl/internal/cache"
	"pdxl/internal/files"
	v3 "pdxl/internal/parser/v3"
)

// SymbolKind identifies the type of a defined symbol.
type SymbolKind uint8

const (
	KindScriptedTrigger SymbolKind = iota
	KindScriptedEffect
	KindTrait
	KindEvent
	KindDecision
	KindOnAction
)

// String returns the kind name used in reports.
func (k SymbolKind) String() string {
	switch k {
	case KindScriptedTrigger:
		return "scripted_trigger"
	case KindScriptedEffect:
		return "scripted_effect"
	case KindTrait:
		return "trait"
	case KindEvent:
		return "event"
	case KindDecision:
		return "decision"
	case KindOnAction:
		return "on_action"
	default:
		return "unknown"
	}
}

// Kinds lists every SymbolKind in a stable order, for iteration in reports.
var Kinds = []SymbolKind{
	KindScriptedTrigger, KindScriptedEffect, KindTrait,
	KindEvent, KindDecision, KindOnAction,
}

// Symbol is a single definition found in the project.
type Symbol struct {
	Name   string
	Kind   SymbolKind
	File   string   // FileSet RelPath where it was defined
	Offset int      // byte offset of the definition (for diagnostics)
	Params []string // sorted, deduped $PARAM$ names found in the body (macros)
}

// Duplicate records a redefinition of an already-defined symbol.
type Duplicate struct {
	Kind  SymbolKind
	Name  string
	First Symbol // the previously registered definition
	File  string // the file that redefined it
}

// SymbolTable holds all collected definitions.
type SymbolTable struct {
	byKind     map[SymbolKind]map[string]Symbol
	Duplicates []Duplicate
}

func newSymbolTable() *SymbolTable {
	return &SymbolTable{byKind: make(map[SymbolKind]map[string]Symbol)}
}

func (t *SymbolTable) add(s Symbol) {
	m := t.byKind[s.Kind]
	if m == nil {
		m = make(map[string]Symbol)
		t.byKind[s.Kind] = m
	}
	if first, ok := m[s.Name]; ok {
		t.Duplicates = append(t.Duplicates, Duplicate{Kind: s.Kind, Name: s.Name, First: first, File: s.File})
		return
	}
	m[s.Name] = s
}

// Count returns the number of symbols of the given kind.
func (t *SymbolTable) Count(kind SymbolKind) int { return len(t.byKind[kind]) }

// Total returns the total number of symbols across all kinds.
func (t *SymbolTable) Total() int {
	n := 0
	for _, m := range t.byKind {
		n += len(m)
	}
	return n
}

// Lookup returns the symbol of the given kind and name, if present.
func (t *SymbolTable) Lookup(kind SymbolKind, name string) (Symbol, bool) {
	s, ok := t.byKind[kind][name]
	return s, ok
}

var macroParamRe = regexp.MustCompile(`\$(\w+)\$`)

// Build walks every winning file in fs, parses it (via store when non-nil),
// and collects definitions into a SymbolTable.
func Build(fs *files.FileSet, store *cache.Store) (*SymbolTable, error) {
	tbl := newSymbolTable()
	walkErr := fs.Walk(func(e files.FileEntry) error {
		rule, ok := ruleFor(e.RelPath)
		if !ok {
			return nil
		}
		tree, err := parseEntry(e.FullPath, store)
		if err != nil {
			return err
		}
		harvest(tbl, tree, rule, e.RelPath)
		return nil
	})
	return tbl, walkErr
}

// parseEntry returns the parse tree for path, using the cache when available.
func parseEntry(path string, store *cache.Store) (*v3.Tree, error) {
	info, err := os.Stat(path)
	if err != nil {
		return nil, err
	}
	if store != nil {
		if tree, _, _ := store.Get(path, info); tree != nil {
			return tree, nil
		}
	}
	src, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	tree, diags := v3.Parse(path, src)
	if store != nil {
		_ = store.Put(path, info, src, tree, diags)
	}
	return tree, nil
}

// harvest collects definitions from one file's tree into tbl.
func harvest(tbl *SymbolTable, tree *v3.Tree, rule defRule, relPath string) {
	root := tree.Root()
	for _, node := range tree.Children(root) {
		if node.Kind != v3.KindField {
			continue
		}
		children := tree.Children(node)
		if len(children) != 2 {
			continue
		}
		key, value := children[0], children[1]
		// A definition has a block body; this skips metadata like `namespace = x`.
		if value.Kind != v3.KindBlock && value.Kind != v3.KindTaggedBlock {
			continue
		}
		seen := make(map[string]struct{})
		collectParams(tree, value, seen)
		tbl.add(Symbol{
			Name:   key.Value(tree.Src),
			Kind:   rule.kind,
			File:   relPath,
			Offset: int(node.SrcStart),
			Params: sortedKeys(seen),
		})
	}
}

// collectParams walks the subtree rooted at n, recording every $PARAM$ name
// found in scalar/tagged-block text into seen. Block nodes carry no source
// span themselves, so params are gathered from their leaf descendants.
func collectParams(tree *v3.Tree, n v3.Node, seen map[string]struct{}) {
	if n.Kind == v3.KindScalar || n.Kind == v3.KindTaggedBlock {
		for _, m := range macroParamRe.FindAllStringSubmatch(n.Value(tree.Src), -1) {
			seen[m[1]] = struct{}{}
		}
	}
	for _, child := range tree.Children(n) {
		collectParams(tree, child, seen)
	}
}

func sortedKeys(set map[string]struct{}) []string {
	if len(set) == 0 {
		return nil
	}
	out := make([]string, 0, len(set))
	for k := range set {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}
