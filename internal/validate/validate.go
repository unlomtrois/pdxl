// Package validate provides semantic indexing of PDXScript across a whole
// project. Phase 0 collects definitions (scripted triggers, traits, events,
// ...) into a SymbolTable; reference resolution is a future phase.
package validate

import (
	"log/slog"
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
	KindCharacter
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
	case KindCharacter:
		return "character"
	default:
		return "unknown"
	}
}

// Kinds lists every SymbolKind in a stable order, for iteration in reports.
var Kinds = []SymbolKind{
	KindScriptedTrigger, KindScriptedEffect, KindTrait,
	KindEvent, KindDecision, KindOnAction, KindCharacter,
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

// addAlias registers an additional resolvable name for a kind without
// duplicate tracking. Used for names that legitimately repeat across many
// definitions, e.g. CK3 trait groups (`group = education_martial`).
func (t *SymbolTable) addAlias(kind SymbolKind, name string, sym Symbol) {
	m := t.byKind[kind]
	if m == nil {
		m = make(map[string]Symbol)
		t.byKind[kind] = m
	}
	if _, ok := m[name]; !ok {
		m[name] = sym
	}
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

// Analyze walks every winning file in fs once, building the symbol table and
// resolving references in memory. Per-file facts come from fc when non-nil
// (unchanged files skip parsing); on a miss the file is parsed via ast and the
// facts are cached. Pass nil for either store to disable that cache.
func Analyze(fs *files.FileSet, ast *cache.Store, fc *FactStore) (*SymbolTable, []RefDiag, error) {
	order, facts, err := gatherFacts(fs, ast, fc)
	if err != nil {
		return nil, nil, err
	}
	tbl, diags := mergeAndResolve(order, facts)
	return tbl, diags, nil
}

// fileKey identifies a project file by both its FileSet RelPath (drives the def
// rule and stable duplicate ordering) and its on-disk FullPath.
type fileKey struct{ rel, full string }

// gatherFacts walks fs once and returns the per-file facts keyed by RelPath,
// plus the walk order. Facts come from fc when non-nil (unchanged files skip
// parsing); on a miss the file is parsed via ast and the facts are cached.
func gatherFacts(fs *files.FileSet, ast *cache.Store, fc *FactStore) ([]fileKey, map[string]FileFacts, error) {
	order := make([]fileKey, 0)
	facts := make(map[string]FileFacts)
	var nHits int

	walkErr := fs.Walk(func(e files.FileEntry) error {
		info, err := os.Stat(e.FullPath)
		if err != nil {
			return err
		}
		f, ok := FileFacts{}, false
		if fc != nil {
			f, ok = fc.Get(e.FullPath, info)
		}
		if ok {
			nHits++
		} else {
			tree, err := parseEntry(e.FullPath, ast)
			if err != nil {
				return err
			}
			f = extractFacts(tree, e.RelPath, e.FullPath)
			if fc != nil {
				_ = fc.Put(e.FullPath, info, tree.Src, f)
			}
		}
		order = append(order, fileKey{rel: e.RelPath, full: e.FullPath})
		facts[e.RelPath] = f
		return nil
	})
	if walkErr != nil {
		return nil, nil, walkErr
	}
	slog.Debug("validate: gathered facts",
		"files", len(order), "fact_hits", nHits, "fact_misses", len(order)-nHits)
	return order, facts, nil
}

// mergeAndResolve builds the symbol table from the gathered facts (in walk
// order, so duplicate "first" is stable) and resolves all references. Pure
// in-memory; no disk I/O.
func mergeAndResolve(order []fileKey, facts map[string]FileFacts) (*SymbolTable, []RefDiag) {
	tbl := newSymbolTable()
	var refs []Ref
	// Definitions first (duplicate-tracked), then aliases (gap-fill only).
	for _, k := range order {
		for _, d := range facts[k.rel].Defs {
			tbl.add(d)
		}
		refs = append(refs, facts[k.rel].Refs...)
	}
	for _, k := range order {
		for _, a := range facts[k.rel].Aliases {
			tbl.addAlias(a.Kind, a.Name, a)
		}
	}
	diags := resolveRefs(tbl, refs)
	slog.Debug("validate: resolved references",
		"symbols", tbl.Total(), "duplicates", len(tbl.Duplicates), "unresolved", len(diags))
	return tbl, diags
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

// directFieldValue returns the scalar value of a direct-child `key = value`
// field in block, or "" if absent or non-scalar.
func directFieldValue(tree *v3.Tree, block v3.Node, key string) string {
	if n, ok := directFieldNode(tree, block, key); ok && n.Kind == v3.KindScalar {
		return n.Value(tree.Src)
	}
	return ""
}

// directFieldNode returns the value node of a direct-child `key = value` field
// in block, or (zero, false) if absent.
func directFieldNode(tree *v3.Tree, block v3.Node, key string) (v3.Node, bool) {
	for _, child := range tree.Children(block) {
		if child.Kind != v3.KindField {
			continue
		}
		kids := tree.Children(child)
		if len(kids) == 2 && kids[0].Value(tree.Src) == key {
			return kids[1], true
		}
	}
	return v3.Node{}, false
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
