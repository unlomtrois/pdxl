# Milestone Report — Rust Port: Parser v3

## Status
Complete.

## Branch
`rust-port-milestone-0-1` (continues the lexer branch).

## Commits
- `94478d7` feat(rust): port parser v3 + flat node pool with exact Go parity (M2)
- (report commit follows)

## Reference Go commit
`5d02a979cb162ba5d89c7e705618de322884bd79`

## Implemented
- **Syntax-tree model** (`pdxl-syntax`): flat node pool + separate child-index
  array, byte-offset ranges, `Arc<[u8]>` shared source.
- **Parser behavior**: direct port of `internal/parser/v3/parser.go`
  (`parseFile/Item/Field/Value/BlockItems`, Pratt scope chains, comparator-as-
  value, unary minus, tagged/plain blocks).
- **Recovery**: `synchronize()` and the verbatim unclosed-block diagnostic.
- **Render/debug tooling**: `render_tree` (golden format) and `validate_tree`
  (structural invariants).
- **Differential harness**: `tools/parsedump` (Go) + Rust `parsedump`, structured
  dump comparison, golden comparison, fuzz/termination tests.

## Architecture
- **Crate layout**: `crates/pdxl-syntax/src/{lib,node,diagnostic,parser,render,
  dump,validate}.rs`, `src/bin/parsedump.rs`, `examples/parsebench.rs`,
  `tests/{fixtures,parity,recovery}.rs`. Depends only on `pdxl-source` and
  `pdxl-lexer`.
- **Public types**: `NodeId`, `NodeKind`, `Node`, `SyntaxTree`, `Parse`,
  `Diagnostic`, `Severity`; `parse`, `render_tree`, `validate_tree`, `dump_json`.
- **Ownership model**: `SyntaxTree` owns `source: Arc<[u8]>`, `nodes: Box<[Node]>`,
  `child_ids: Box<[NodeId]>`. No self-referential lifetimes; `node_text` borrows
  from the owned source. `Diagnostic.filename` is `Arc<str>`, shared not copied.
- **Node / child-index representation**: a node stores `[child_start, child_end)`
  into `child_ids`; each `child_ids` entry indexes `nodes`. Non-field nodes carry
  `TokenKind::Invalid` as operator (compact); the dump normalizes both Go and Rust
  to `"invalid"`.
- **Allocation-free traversal**: `child_ids(id) -> &[NodeId]` and
  `children(id) -> impl ExactSizeIterator<Item = NodeId>` borrow; no per-traversal
  `Vec`. Builder vectors are shrunk with `into_boxed_slice`.

## Parity
- **Fixtures compared**: 11 — all 8 `testdata/*.txt`, `testdata/ck3/
  scripted_trigger_macro.txt`, `testdata/lint/advance_for_lint.txt` (malformed),
  and `rust/crates/pdxl-lexer/testdata/stress.txt` (malformed UTF-8).
- **Exact node-layout matches**: 11/11 byte-identical structured dumps — node
  count, ids, **allocation order**, kinds, start/end offsets, normalized operator,
  child ranges, and the complete child-index array all match the Go oracle.
- **Exact diagnostics matches**: order, offset, severity, and message identical
  (verified on the malformed lint fixture, which produces real diagnostics, and on
  the unit recovery tests).
- **Golden-test matches**: 8/8 `testdata/*.golden` reproduced byte-for-byte by
  `render_tree`.
- **Lexer regression result**: M1 lexer differential re-run — 11 fixtures still
  byte-identical. No regression.

## Tests
### Go
`go test ./...` — green, unchanged. Only the additive `tools/parsedump` package
was introduced; production parser behavior is untouched.

### Rust
`cargo test --workspace` — green. `pdxl-syntax`: 35 unit tests (ported from
`parser_test.go`: fields, blocks, tagged blocks, scalar lists, key/value scope
chains, pipe chains, unary negatives, dates/BC dates, quoted strings, booleans,
script values, inline math, macro params, comparator-as-value, missing operator/
value, unclosed/nested-malformed blocks, multiple errors, typed definitions,
arbitrary-input termination) + 1 doc test. `cargo fmt --check` and
`cargo clippy --workspace --all-targets --all-features -- -D warnings` clean. No
`unsafe` (forbidden at crate level).

### Differential
`tests/parity.rs` — 11/11 fixtures byte-identical to the Go oracle
(`go run ./tools/parsedump`). `scripts/parity.sh --parse` reproduces this
standalone. Self-skips if `go` is unavailable.

### Invariant validation
`validate_tree` runs on every fixture (valid and malformed) inside the
differential and golden tests, and on 256 seeded fuzz inputs, deeply nested
unclosed blocks (depth 500), whitespace/comment-only inputs, and stray
delimiters in `tests/recovery.rs`. Checks: non-empty pool; root id 0 is a `File`;
well-formed ranges within source; child ranges index `child_ids`; child ids index
`nodes`; fields have exactly two children with a scalar key; scalars have no
children.

## Benchmarks
| Case | Go parser v3 | Rust parser | Difference |
|------|--------------|-------------|------------|
| international_organization.txt (24401 B) | ~176 MB/s, 863 allocs/op | ~282 MB/s | Rust ~1.6× faster |
| advance.txt (211 B) | ~104 MB/s | ~187 MB/s | Rust faster |
| subject_type.txt (3301 B) | ~125 MB/s | ~217 MB/s | Rust faster |

Methodology caveats: Go uses `testing.B` (`make bench-parser`); Rust uses a simple
throughput loop (`cargo run --release --example parsebench`). Both include
tokenization (Go's `Parse` calls `lexer.Tokenize`; Rust's `parse` calls
`tokenize`). Node/child counts are identical to Go (626 nodes / 625 child-index
entries for the large fixture), confirmed by the structured dump. The Go
"863 allocs/op" counts pool growth + child slices + diagnostics, not nodes —
the flat pool stores nodes inline with no per-node heap allocation. Well within
the "no regression larger than 2×" gate (it is faster).

## Deviations from Go
- The parser returns `Parse { tree, diagnostics }` (always a tree), not a
  `Result`. Matches Go's `(*Tree, []Diagnostic)` contract.
- `invalidIdx` (`^uint32(0)`) is modeled as `Option<NodeId>` for parse-function
  return values — this changes no allocation order or node id.
- Non-field nodes store `TokenKind::Invalid` as operator (Go leaves the zero tag,
  which stringifies as `identifier`). Both are normalized to `"invalid"` in the
  structured dump, so node-level parity is exact.
- `tools/parsedump` (Go) is new, additive tooling; it does not change production
  parser behavior.

## Bugs or ambiguities discovered
- None in the Go parser. One quirk preserved deliberately: in `parseValue`, a
  unary `-` immediately followed by a non-atom (e.g. `-}`) absorbs that token's
  end into the scalar range. This is existing Go behavior, reproduced exactly; it
  is not exercised by the fixtures but is covered by the fuzz/termination tests.
- The Go golden renderer has no case for a bare `TaggedBlock` or `File` at item
  level (it emits nothing). Reproduced as-is in `render_tree`.

## Files changed
- Added: `rust/crates/pdxl-syntax/**` (`src/{lib,node,diagnostic,parser,render,
  dump,validate,tests}.rs`, `src/bin/parsedump.rs`, `examples/parsebench.rs`,
  `tests/{fixtures,parity,recovery}.rs`, `Cargo.toml`), `tools/parsedump/main.go`,
  `rust/docs/MILESTONE-2-REPORT.md`.
- Modified: `rust/Cargo.toml` (+`pdxl-syntax`, +`pdxl-lexer` workspace dep),
  `rust/Cargo.lock`, `rust/README.md`, `rust/docs/BASELINE.md`, `scripts/parity.sh`.
- Unchanged: all existing Go source under `cmd/`, `internal/`.

## Risks for later milestones
- **Serialization layout (cache, M4).** `Node` is `Copy`/`repr`-friendly and the
  pool is contiguous, which suits a versioned binary format — but persisting must
  include a syntax-version key (the Go cache caveat). Do *not* rely on
  `Arc<[u8]>` identity across a serialize/deserialize boundary.
- **Source ownership.** The tree shares `Arc<[u8]>`; a future overlay/cache layer
  must keep the exact source bytes a tree was parsed from (offsets are meaningless
  against different bytes). `node_text` borrows from that shared buffer.
- **Recursive traversal depth.** Both the parser and any recursive consumer use
  one native stack frame per block level. Go has growable stacks; Rust does not.
  Deeply nested input (thousands of levels) could overflow a small-stack thread.
  The fuzz test caps depth at 500; a future iterative traversal or explicit depth
  guard may be warranted for untrusted input.
- **Malformed-tree consumers.** Downstream (facts/symbols) must tolerate partial
  trees and accumulated diagnostics; `validate_tree` guarantees structural
  soundness but not semantic completeness.
- **Cache compatibility.** Rust will not read Go `gob`; a fresh versioned format
  is expected (M4), keyed by source content + syntax/format version.

## Recommendation for Milestone 3 (FileSet & overlay) — do not begin
Port `internal/files` into a `pdxl-files` crate depending on `pdxl-source` (not on
`pdxl-syntax`, except that `.mod` parsing uses the parser — keep that dependency
explicit and minimal). Preserve: normalized lowercase, forward-slash overlay
keys; vanilla-first/mod-last load order with later roots shadowing earlier;
`replace_path` prefix dropping; ignored dirs/files; `.mod` descriptor parsing via
the parser; Windows/Proton path resolution (`ResolveWindowsPath`); deterministic
winner order (insertion order). Build fixture-based tests over temporary
directories, and add a `FileSet` walk dump (relative path, kind, full path) for
differential comparison against a new additive Go `tools/filesetdump`. Keep
deterministic iteration wherever Go behavior depends on file order. No caches,
analysis, CLI, or LSP work in that milestone.
