# Plan: Cache Layer

## Goal

Eliminate redundant parsing on incremental runs. A mod directory with 10 000 files at 115 MB/s still takes ~1 s to parse from scratch. The cache turns re-runs with no file changes into pure deserialization — which should be 5–10× faster than re-parsing.

Tiger's fatal flaw was no caching at all: every validation run re-read 50 GB from disk. We fix this in Phase 1, before validation exists, so it's never a problem.

---

## What gets cached

Each cached entry maps a **source file** to its **pre-lexed token slice + v3 Tree** (or just v2 AST for now). The cache key is the file's canonical path; validity is checked via mtime + SHA-256.

```
.pdxl/cache/<sha256-of-path>.bin   ← gob-encoded CacheEntry
```

```go
type CacheEntry struct {
    ModTime int64    // unix nano of source file at cache-write time
    SHA256  [32]byte // content hash
    Tokens  []lexer.Token
    Nodes   []parser.Node   // v3 flat pool
    Index   []uint32
}
```

On cache hit: deserialize and return. On miss or stale: re-parse, write entry, return.

---

## Invalidation

1. `stat()` the source file → get mtime
2. If mtime matches cached mtime → cache hit (fast path, no read)
3. If mtime differs → read file, SHA-256 → if hash matches → update mtime in cache, return (handles touch without edit)
4. If hash differs → re-parse, write new entry

No TTL. No global invalidation sweep. Each file is self-contained.

---

## Serialization format

`encoding/gob` for correctness and zero dependencies. If gob proves slow under profiling, swap to `vmihailenco/msgpack` — the interface stays the same behind a `CacheStore` interface.

Gob can encode `[]Token`, `[]Node`, `[]uint32` directly since they are all fixed-size value types with no pointers. The `Src []byte` is **not** cached — the caller re-reads the source file anyway for mtime/hash checking.

---

## API

```go
// internal/cache/cache.go

type Store struct {
    Dir string // e.g. ".pdxl/cache"
}

func NewStore(dir string) (*Store, error)

// Get returns a cached Tree for src, or nil if the cache is cold/stale.
// src is the raw file bytes (already read by the caller for hashing).
func (s *Store) Get(path string, modTime int64, src []byte) (*parser.Tree, []lexer.Token, error)

// Put writes a cache entry. Called after a successful parse.
func (s *Store) Put(path string, modTime int64, src []byte, tree *parser.Tree, tokens []lexer.Token) error
```

The caller pattern:

```go
tree, tokens, err := store.Get(path, info.ModTime().UnixNano(), src)
if tree == nil {
    tokens = lexer.Tokenize(src)
    tree, err = parser.ParseTokens(path, tokens, src)
    store.Put(path, info.ModTime().UnixNano(), src, tree, tokens)
}
```

This also motivates the `ParseTokens(path, tokens, src)` entry point discussed after the parser PR — the cache layer is where pre-lexed tokens are handed in directly.

---

## Files to create

| File | Purpose |
|---|---|
| `internal/cache/cache.go` | `Store`, `Get`, `Put`, `CacheEntry` |
| `internal/cache/cache_test.go` | round-trip test, stale-on-mtime, stale-on-hash, concurrent read |
| `internal/lexer/lexer.go` addition | `Tokenize(src []byte) []lexer.Token` — pre-lex helper |
| `internal/parser/v3/parser.go` addition | `ParseTokens(filename string, tokens []lexer.Token, src []byte) (*Tree, error)` |

## Files to update

| File | Change |
|---|---|
| `cmd/pdxl/parse.go` | Accept `--cache` flag; wire `Store` |
| `Makefile` | `bench-cache` target |

---

## Verification

```sh
make test                          # all existing tests pass
go test ./internal/cache/... -v    # cache-specific tests
# cold run
time bin/pdxl parse testdata/international_organization.txt
# warm run (should be significantly faster on a large directory)
time bin/pdxl parse testdata/international_organization.txt --cache
```

---

## Out of scope for this iteration

- Cache size limit / eviction (LRU) — not needed until we have 10 000 files
- Shared cache across processes — file locking; defer until LSP integration
- Dependency graph invalidation — needed once cross-file validation exists (Phase 2)
