# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
make test         # run all tests
make lint         # run golangci-lint
make build        # build binary to bin/pdxl
make install      # install to $GOPATH/bin
make bench        # run all benchmarks (lexer + all parsers + cache)
make bench-lexer  # lexer benchmarks only
make bench-parser # v1/v2/v3 parser benchmarks side-by-side
make bench-cache  # cache L1/L2 read and write benchmarks
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
cmd/pdxl/               — CLI: lex, parse, lint, init subcommands
internal/lexer/         — tokenizer (internal; not public API yet)
internal/parser/
  v1/                   — participle-based reference parser (benchmarking baseline only)
  v2/                   — hand-written recursive descent + Pratt, pointer-tree AST
  v3/                   — same algorithm, flat node-pool AST (fastest; preferred for new tools)
internal/cache/         — two-level parse cache: in-memory LRU + on-disk gob store
internal/config/        — TOML config loader (pdxl.toml)
internal/testutil/      — shared TestdataDir() and DiffLines() used by all test packages
testdata/               — shared .txt fixture files and .golden expected-output files
testdata/lint/          — intentionally broken fixtures for lint/recovery tests
testdata/ck3/           — CK3-specific fixtures (macros, scripted triggers)
pkg/mod/                — reserved for future public packages
```

### Lexer

- `Lexer` holds `source []byte` and `pos int` (byte offset, not rune index).
- `Token{Start, End int, Tag Tag}` — byte offsets only; no string copy. Extract via `token.GetValue(source)`.
- `advance()` uses `utf8.DecodeRune` — multi-byte safe. Invalid UTF-8 is silently treated as an identifier char.
- `yes`/`no` keyword detection happens in `Next()` after `lexIdentifier()` returns.
- UTF-8 BOM (`\xEF\xBB\xBF`) is skipped in `Init()`.
- `$IDENT$` is lexed as `TagMacroParam` (a single atom); bare `$` falls back to `TagDollar`.
- `lexer.Tokenize(src []byte) []Token` is the public helper used by `v3.newParser`.

### Parser v3 — flat node-pool AST with error recovery

Preferred base for all new tools (linter, LSP, validator). ~2× fewer allocations than v2. The `parse` command uses v3.

All nodes live in a single `Tree.Nodes []Node` slice. Parent→child relationships go through `Tree.Index []uint32` — a node's children are `Tree.Index[node.ChildStart:node.ChildEnd]`, each element being an index into `Tree.Nodes`. This eliminates heap pointers inside nodes.

```go
tree, diags := v3.Parse("file.pdx", src)
// tree is always non-nil; diags non-empty means errors were found but parsing continued
root := tree.Root()                    // tree.Nodes[0], always KindFile
refs := tree.ChildRefs(root)          // []uint32, no alloc
for _, idx := range refs {
    child := tree.Nodes[idx]
    fmt.Println(child.Value(tree.Src)) // materializes string from src[SrcStart:SrcEnd]
}
```

`Node.Op` (a `lexer.Tag`) holds the operator for `KindField` nodes. `Node.OpString()` maps it back to a source symbol (`"="`, `"?="`, etc.).

**Error recovery** uses synchronization: on any unexpected token, the parser records a `Diagnostic{Filename, Offset, Msg, Severity}` and calls `synchronize()`, which skips tokens until reaching a `}`, the start of a plausible new item (atom followed by operator or `{`), or EOF. Recovery is zero-cost on valid input: `p.diags` stays nil and adds no allocations.

`parseBlockItems` receives the opening `{` token and reports `"unclosed block (missing '}'; an inner block may have stolen the closing brace)"` when EOF is reached. When a block is unclosed, subsequent fields are absorbed into it — the parser cannot distinguish block-level from outer-level items without indentation heuristics.

**Typed definitions** (`scripted_trigger NAME = { ... }`) parse as two sibling nodes under KindFile: a bare KindScalar for the type keyword, then a KindField for the assignment. This is valid syntactically; semantic distinction belongs in a future validator layer.

### Parser v2 — pointer-tree AST

Used only for benchmarking comparison. Key API:
- `Field.KeyParts []lexer.Token`, `Field.Operator lexer.Tag` — materialize with `field.Key(src)` and `OperatorString(field.Operator)`.
- `Scalar.Parts []lexer.Token` — materialize with `scalar.Value(src)`.

`v3.Parse` returns `(*Tree, []Diagnostic)`; `v2.ParseBytes` returns `(*File, error)`. They are independent.

### Cache layer (`internal/cache`)

`Store` is the two-level cache. `NewStore(dir, lruCap)` creates the disk directory and an in-memory LRU when `lruCap > 0`. Also writes `.pdxl/.gitignore` on first use.

```go
store, _ := cache.NewStore(cfg.Cache.Dir, cfg.Cache.LRUCap)
info, _ := os.Stat(path)
tree, diags, _ := store.Get(path, info)   // nil on miss
if tree == nil {
    tree, diags = v3.Parse(path, src)
    store.Put(path, info, src, tree, diags)
}
```

**Invalidation:** L1 checks mtime; L2 always verifies SHA-256 (mtime alone is unreliable on coarse-resolution filesystems). A same-content/different-mtime file refreshes the stored mtime. A changed hash is a full miss.

**Disk format:** gob-encoded `diskEntry{ModTime int64, SHA256 [32]byte, SrcGzip []byte, Nodes []v3.Node, Index []uint32, Diags []v3.Diagnostic}`. Entry filename is `sha256(filepath.Clean(path)).bin`. Source is stored gzip-compressed so hot L2 paths skip the original file entirely.

**Performance baseline (i5-11400H):** L1 hit ~23 ns / 0 allocs; L2 disk read ~190 µs; disk write ~620 µs.

### Config (`internal/config`)

Config file is `pdxl.toml` in the project root (created by `pdxl init`). Missing file is not an error — defaults are used.

```toml
game = "ck3"

[cache]
enabled = true
dir     = ".pdxl/cache"
lru_cap = 256

[lint]
context = 0
```

`config.Load(path)` starts from `Default()` and overlays the file, so partial configs inherit defaults. Override the path with `pdxl --config /path/to/pdxl.toml`.

### CLI subcommands

| Command | Description |
|---------|-------------|
| `pdxl init [--game ck3]` | Create `pdxl.toml` with defaults; `--force` to overwrite |
| `pdxl lint <files> [--context N] [--no-cache]` | Structural diagnostics; `--context` prints N source lines around each |
| `pdxl parse <file> [--tree\|--json]` | Print AST; `--tree` shows labelled node tree with box-drawing chars |
| `pdxl lex <file>` | Dump token stream |

All subcommands accept `--verbose` / `-v` (debug logging via slog) and `--config`.

### Testutil and golden tests

`internal/testutil.TestdataDir()` returns the absolute path to the project-level `testdata/` directory using `runtime.Caller()` — safe to call from any sub-package without path hacks.

All parser packages share the same `testdata/*.txt` fixtures. Each has its own `fixture_test.go` that renders the parsed AST to a string and diffs against `testdata/*.golden`. Run with `-update` to regenerate goldens.

### Token naming

Tag constants use `snake_case` (e.g. `literal_boolean`, `l_brace`). The linter's `var-naming` rule is disabled for this.

### Commit style

Use git-flow prefixes: `feat(scope):`, `fix(scope):`, `refactor(scope):`, `chore(scope):`, etc.
