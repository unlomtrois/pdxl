# Port Baseline (Milestone 0)

Reference measurements taken before/at the start of the Rust port. The Go
implementation is the oracle; these numbers anchor parity and performance
comparisons for every later milestone.

## Environment

| | |
|---|---|
| Go | go1.26.4 linux/amd64 |
| Rust | rustc/cargo 1.96.0 (edition 2024) |
| OS | Linux 6.17 (Ubuntu 24.04 base), x86_64 |
| CPU | 11th Gen Intel Core i5-11400H @ 2.70GHz, 12 logical cores |
| Reference commit | `5d02a979cb162ba5d89c7e705618de322884bd79` |

## Go test suite

`go test ./...` — all green: `internal/{cache,config,files,lexer,lsp,validate}`,
`internal/parser/{v1,v2,v3}`. (`cmd/pdxl`, `internal/parser`, `internal/testutil`,
`tools/lexdump` have no test files.)

## Fixtures (`testdata/`)

| File | Bytes |
|---|---|
| advance.txt | 211 |
| international_organizations.txt | 525 |
| parliament_types.txt | 581 |
| government_reform.txt | 580 |
| modifier_types.txt | 1617 |
| special_statuses.txt | 2799 |
| subject_type.txt | 3301 |
| international_organization.txt | 24401 |

Plus `testdata/ck3/scripted_trigger_macro.txt`, `testdata/lint/advance_for_lint.txt`,
and the parser `*.golden` AST renders (used by Milestone 2, not the lexer).

## Lexer benchmark — Go oracle (`make bench-lexer`, count=3)

| Case | ns/op | MB/s | B/op | allocs/op |
|---|---|---|---|---|
| advance.txt | ~1284 | ~164 | 480 | 20 |
| government_reform.txt | ~3700 | ~156 | 1416 | 59 |
| international_organization.txt | ~119000 | ~205 | 18432 | 768 |
| modifier_types.txt | ~8720 | ~185 | 2688 | 112 |
| special_statuses.txt | ~15400 | ~182 | 4344 | 181 |
| subject_type.txt | ~21000 | ~157 | 6120 | 255 |
| **LexLarge** (international_organization.txt) | ~119300 | **~205** | 18432 | 768 |

Go allocates one heap `*Token` per token (hence the allocs/op scaling with token
count).

## Parser benchmark — Go oracle (`make bench-parser`, count=1), largest fixture

| Variant | ns/op | MB/s | allocs/op |
|---|---|---|---|
| v1 (participle) | ~11.9M | ~2.0 | 19105 |
| v2 (pointer tree) | ~155k | ~157 | 1917 |
| v3 (flat node pool) | ~138k | **~177** | 863 |

v3 is the parser the port targets in Milestone 2.

## Cache benchmark — Go oracle (`make bench-cache`, count=1)

| Case | ns/op | MB/s | allocs/op |
|---|---|---|---|
| CacheWriteDisk | ~606k | ~40 | 63 |
| CacheReadDisk | ~184k | ~133 | 325 |
| CacheReadL1 | ~25 | — | 0 |

## Lexer benchmark — Rust port (`cargo run --release --example lexbench`)

Simple throughput harness (not Criterion); methodology differs from Go's
`testing.B`, but the comparison is fair. Zero heap allocations per token.

| Case | ns/op | MB/s |
|---|---|---|
| advance.txt | ~803 | ~251 |
| international_organization.txt | ~82200 | ~283 |
| **LexLarge** | ~83100 | **~280** |

**Result:** the Rust lexer is ~1.37× the Go lexer's throughput on the large
fixture (~280 vs ~205 MB/s) and allocates nothing per token. No performance
regression; optimization is explicitly out of scope for this milestone.

## Parser benchmark — Rust port (`cargo run --release --example parsebench`)

Simple throughput harness (not Criterion); like the Go `Parse`, it includes
tokenization. The flat node pool is stored inline (no per-node heap allocation;
the pool/child-index/diagnostics vectors still allocate as they grow).

| Case | Go v3 | Rust | Difference |
|---|---|---|---|
| ParseLarge (international_organization.txt) | ~176 MB/s, 863 allocs/op | ~282 MB/s | Rust ~1.6× faster |
| advance.txt | ~104 MB/s | ~187 MB/s | Rust faster |
| subject_type.txt | ~125 MB/s | ~217 MB/s | Rust faster |

Node/child counts are identical to Go (verified by the structured differential
dump): the large fixture yields 626 nodes / 625 child-index entries. The Go
"863 allocs/op" counts pool growth + child slices + diagnostics, not nodes.
No regression; parser optimization is out of scope for this milestone.
