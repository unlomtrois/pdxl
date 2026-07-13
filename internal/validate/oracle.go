package validate

import v3 "pdxl/internal/parser/v3"

// ExtractFileFacts exposes extractFacts for differential parity tooling
// (tools/factsdump). It is additive: a plain re-export with no behavior of its
// own, existing only so the Rust port can be oracle-checked against the exact
// production extraction logic. Not intended as public API.
func ExtractFileFacts(tree *v3.Tree, relPath, fullPath string) FileFacts {
	return extractFacts(tree, relPath, fullPath)
}
