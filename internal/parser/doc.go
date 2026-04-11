// Package parser contains three parser implementations for benchmarking comparison.
//
//   - v1: participle-based (reference baseline)
//   - v2: hand-written recursive descent + Pratt, pointer-tree AST
//   - v3: hand-written recursive descent + Pratt, flat node pool (no pointers in Node)
//
// Run `make bench-parser` to compare. The CLI (cmd/pdxl) uses v2.
package parser
