# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
make test         # run all tests
make lint         # run golangci-lint
make build        # build binary to bin/pdxl
make bench        # run all benchmarks (lexer + all parsers)
make bench-lexer  # lexer benchmarks only
make bench-parser # v1/v2/v3 parser benchmarks side-by-side
```

To run a single test:
```sh
go test ./internal/lexer/... -run TestUTF8Identifier
```

To regenerate golden fixture files after a deliberate output change:
```sh
go test ./internal/parser/v2/... -update
go test ./internal/parser/v3/... -update
```

Always use `make test` to verify changes, never raw `go test` or `go build`.

Nix users: activate the toolchain with `nix-shell` before running make commands.

## Architecture

pdxl is a toolkit for parsing Paradox Interactive scripting files (PDXScript, used in EU5, CK3, Victoria 3, etc.). The grammar is the same across all games; only semantics differ.

```
cmd/pdxl/               — CLI: `lex`, `parse`, and `lint` subcommands
internal/lexer/         — tokenizer (internal; not public API yet)
internal/parser/
  v1/                   — participle-based reference parser (benchmarking baseline only)
  v2/                   — hand-written recursive descent + Pratt, pointer-tree AST
  v3/                   — same algorithm, flat node-pool AST (fastest; preferred for new tools)
internal/testutil/      — shared TestdataDir() and DiffLines() used by all test packages
testdata/               — shared .txt fixture files and .golden expected-output files
pkg/mod/                — reserved for future public packages
```

### Lexer

- `Lexer` holds `source []byte` and `pos int` (byte offset, not rune index).
- `Token{Start, End int, Tag Tag}` — byte offsets only; no string copy. Extract via `token.GetValue(source)`.
- `advance()` uses `utf8.DecodeRune` — multi-byte safe. Invalid UTF-8 is silently treated as an identifier char.
- `yes`/`no` keyword detection happens in `Next()` after `lexIdentifier()` returns.
- UTF-8 BOM (`\xEF\xBB\xBF`) is skipped in `Init()`.

### Parser v2 — pointer-tree AST

Used by the CLI (`cmd/pdxl/parse.go`). Ergonomic for visitor/linter code.

Key AST types after the token rewrite:
- `Field.KeyParts []lexer.Token`, `Field.Operator lexer.Tag` — no string copies; materialize with `field.Key(src)` and `OperatorString(field.Operator)`.
- `Scalar.Parts []lexer.Token` — materialize with `scalar.Value(src)`.
- `TaggedBlock.Tag lexer.Token` — materialize with `string(tag.GetValue(src))`.

Grammar resolved with LL(2) lookahead: atom+operator → Field; atom+`{` → TaggedBlock; `{` → Block; else → Scalar. Scope chains (`:` `.` `|`) parsed via Pratt with binding power 80.

### Parser v3 — flat node-pool AST with error recovery

Preferred base for performance-sensitive tools (linter, LSP). ~2× fewer allocations than v2.

All nodes live in a single `Tree.Nodes []Node` slice. Parent→child relationships go through `Tree.Index []uint32` — a node's children are `Tree.Index[node.ChildStart:node.ChildEnd]`, each element being an index into `Tree.Nodes`. This eliminates heap pointers inside nodes.

```go
tree, diags := v3.Parse("file.pdx", src)
// tree is always non-nil; diags non-empty means errors were found but parsing continued
root := tree.Root()                         // tree.Nodes[0], always KindFile
refs := tree.ChildRefs(root)               // []uint32, no alloc
for _, idx := range refs {
    child := tree.Nodes[idx]
    fmt.Println(child.Value(src))          // materializes string from src[SrcStart:SrcEnd]
}
```

`Node.Op` (a `lexer.Tag`) holds the operator for `KindField` nodes. `Node.OpString()` maps it back to a source symbol (`"="`, `"?="`, etc.).

**Error recovery** uses synchronization: on any unexpected token, the parser records a `Diagnostic{Filename, Offset, Msg, Severity}` and calls `synchronize()`, which skips tokens until reaching a `}`, the start of a plausible new item (atom followed by operator or `{`), or EOF. The tree is always returned — callers get a partial AST even for broken files. Recovery is zero-cost on valid input: `p.diags` stays nil and adds no allocations.

`parseBlockItems` receives the opening `{` token so it can report `"unclosed block"` at the brace's byte offset when EOF is reached before `}`. This is the only diagnostic currently emitted; it is surfaced by `pdxl lint`.

When a block is unclosed, subsequent fields are absorbed into it — the parser cannot distinguish block-level from outer-level items without indentation heuristics.

`v3.Parse` returns `(*Tree, []Diagnostic)`; `v2.ParseBytes` returns `(*File, error)`. They are independent — v3 does not use v2 types.

### Testutil and golden tests

`internal/testutil.TestdataDir()` returns the absolute path to the project-level `testdata/` directory using `runtime.Caller()` — safe to call from any sub-package without path hacks.

All parser packages share the same `testdata/*.txt` fixtures. Each has its own `fixture_test.go` that renders the parsed AST to a string and diffs against `testdata/*.golden`. Run with `-update` to regenerate goldens.

### Token naming

Tag constants use `snake_case` (e.g. `literal_boolean`, `l_brace`). The linter's `var-naming` rule is disabled for this.

### Commit style

Use git-flow prefixes: `feat(scope):`, `fix(scope):`, `refactor(scope):`, `chore(scope):`, etc.
