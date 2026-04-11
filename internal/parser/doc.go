// Package parser contains three parser implementations for benchmarking comparison.
//
//   - v1: participle-based (~2 MB/s, 19012 allocs/op)
//   - v2: hand-written pointer tree, Pratt (~104 MB/s, 2293 allocs/op)
//   - v3: flat node pool with index indirection (~115 MB/s, 880 allocs/op)
//
// The CLI (cmd/pdxl) uses v2 for its pointer-tree API.
package parser
