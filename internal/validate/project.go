package validate

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"pdxl/internal/cache"
	"pdxl/internal/files"
	v3 "pdxl/internal/parser/v3"
)

// Project holds a whole-project symbol table in memory and supports cheap
// incremental updates: when one file changes, only that file is re-parsed, then
// the table and diagnostics are rebuilt from the in-memory facts (no disk reads,
// no re-parsing of unchanged files). It is the foundation for a long-running
// validator (LSP server / watch loop). Not safe for concurrent use.
type Project struct {
	ast   *cache.Store
	fc    *FactStore
	order []fileKey            // walk order (rel + full)
	facts map[string]FileFacts // RelPath -> facts
	tbl   *SymbolTable
	diags []RefDiag
}

// NewProject gathers facts for every winning file in fs (via fc when non-nil)
// and builds the initial table and diagnostics.
func NewProject(fs *files.FileSet, ast *cache.Store, fc *FactStore) (*Project, error) {
	order, facts, err := gatherFacts(fs, ast, fc)
	if err != nil {
		return nil, err
	}
	p := &Project{ast: ast, fc: fc, order: order, facts: facts}
	p.rebuild()
	return p, nil
}

// rebuild recomputes the table and diagnostics from the in-memory facts.
func (p *Project) rebuild() {
	p.tbl, p.diags = mergeAndResolve(p.order, p.facts)
}

// Update re-extracts the single tracked file at fullPath from disk, replaces its
// facts (and refreshes the on-disk fact cache), then rebuilds the table and
// diagnostics in memory. No other file is re-read. fullPath must already be part
// of the project (adding/removing files needs a fresh FileSet scan).
func (p *Project) Update(fullPath string) error {
	key, ok := p.keyFor(fullPath)
	if !ok {
		return fmt.Errorf("%s is not part of the project", fullPath)
	}
	tree, err := parseEntry(key.full, p.ast)
	if err != nil {
		return err
	}
	p.facts[key.rel] = extractFacts(tree, key.rel, key.full)
	if p.fc != nil {
		if info, err := os.Stat(key.full); err == nil {
			_ = p.fc.Put(key.full, info, tree.Src, p.facts[key.rel])
		}
	}
	p.rebuild()
	return nil
}

// UpdateSource re-analyzes a tracked file from the given in-memory source
// (e.g. an unsaved editor buffer) instead of reading disk, then rebuilds the
// table and diagnostics in memory. No other file is re-read, and the on-disk
// caches are left untouched (the buffer may differ from disk).
func (p *Project) UpdateSource(fullPath string, src []byte) error {
	key, ok := p.keyFor(fullPath)
	if !ok {
		return fmt.Errorf("%s is not part of the project", fullPath)
	}
	tree, _ := v3.Parse(key.full, src)
	p.facts[key.rel] = extractFacts(tree, key.rel, key.full)
	p.rebuild()
	return nil
}

// Table returns the current whole-project symbol table.
func (p *Project) Table() *SymbolTable { return p.tbl }

// Diags returns all unresolved-reference diagnostics across the project.
func (p *Project) Diags() []RefDiag { return p.diags }

// FileDiags returns only the unresolved references located in fullPath.
func (p *Project) FileDiags(fullPath string) []RefDiag {
	key, ok := p.keyFor(fullPath)
	if !ok {
		return nil
	}
	prefix := key.full + ":"
	var out []RefDiag
	for _, d := range p.diags {
		if strings.HasPrefix(d.Loc, prefix) {
			out = append(out, d)
		}
	}
	return out
}

// keyFor finds the tracked fileKey whose on-disk path matches fullPath
// (compared as cleaned absolute paths).
func (p *Project) keyFor(fullPath string) (fileKey, bool) {
	target, err := filepath.Abs(fullPath)
	if err != nil {
		return fileKey{}, false
	}
	target = filepath.Clean(target)
	for _, k := range p.order {
		if abs, err := filepath.Abs(k.full); err == nil && filepath.Clean(abs) == target {
			return k, true
		}
	}
	return fileKey{}, false
}
