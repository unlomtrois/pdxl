# Milestone Report — Rust Port: Per-File Semantic Facts

## Status
Complete.

## Branch
`rust-port-milestone-5` (based on `b53624a`, the M4 cache).

## Reference Go commit
`5d02a979cb162ba5d89c7e705618de322884bd79`

## Scope
Ports the *facts* half of `internal/validate`: the `FileFacts` model
(`Symbol`/`Ref`), the single-walk extraction (`extractFacts` + helpers), and
the CK3 registry (`schema_ck3.go`). The SymbolTable / `mergeAndResolve` /
`Project` half is Milestone 6.

## Implemented
- **`pdxl-analysis`** (generic engine; deps: `pdxl-src`, `pdxl-ast`):
  `SymbolKind` (7 variants, Go names/order), `Symbol`, `Ref`, `FileFacts`,
  `Schema`/`DefRule`, `extract_facts(tree, rel_path, full_path, &schema)`, and
  `ANALYSIS_VERSION` (for any future facts persistence).
- **`pdxl-ck3`** (rules as data; dep: `pdxl-analysis`): `schema()` — a
  transcription of `ck3DefRules` / `ck3RefRules` / `ck3BlockIDRefRules` /
  `ck3ListRefRules` / `ck3WeightedRefRules` / `OnActionDir`, plus the trait
  alias keys and scope keywords that Go hardcodes in `facts.go`/`resolve.go`.
- **`pdxl_src::line_col`** — Go's display-only `file:line:col` derivation
  (1-indexed; column counts **bytes**, exactly like `Token.getPosition`).
- **Behavior ported exactly**: block-body definition detection (skips
  `namespace = x`), `$PARAM$` collection over scalar/tagged-block leaves
  (sorted, deduped; hand-rolled scanner matching Go's `\$(\w+)\$` regex
  semantics — no regex dependency), trait `group`/`group_equivalence` aliases
  (including the `EndOffset == SrcStart` quirk), all four reference shapes
  (scalar, block-id, list, weighted), on_action gating, quote stripping, the
  macro-concatenation peek (`src[value.end] == '$'`), and `skipRefValue`
  (empty, `$`/`:`, scope keywords).

## Architecture note (generic vs game-specific)
Go hardcodes CK3 decisions inside its extraction functions (`rule.kind ==
KindTrait` triggers alias harvesting; `OnActionDir` gates lists). The Rust
engine is game-agnostic; every such decision arrives as **data** in a `Schema`
supplied by `pdxl-ck3`. Behavior is identical (oracle-checked); only ownership
moved. The schema stays deliberately small — deep validation is ck3-tiger's
territory.

## Deliberate deviation: `FactStore` not ported
Per the measured-simplification plan agreed for this milestone: complexity must
buy its way in with milliseconds. The decision benchmark
(`cargo run --release -p pdxl-parity --example factsbench`) cold-reads,
parses, and extracts a synthesized CK3-scale corpus:

| corpus | threads | cold read+parse+extract |
|---|---|---|
| 3,500 files / 14.9 MB | 1 | **87 ms** (170 MB/s) |
| 3,500 files / 14.9 MB | 12 | **13 ms** (1.14 GB/s) |

The Go `FactStore` exists to make warm runs "sub-second"; the Rust *cold* path
is ~10× under that bar single-threaded and ~75× under it threaded — and a
facts cache could never beat it meaningfully, since both paths must read and
hash every file anyway. The FactStore stays unwritten. `ANALYSIS_VERSION`
exists so a future cache (if a real corpus ever proves the need — the bench
accepts a root argument) has its version key ready. The M4 AST cache is
unaffected (its consumer story is the LSP warm start, to be evaluated in M6+).

## Parity
- **Oracle plumbing**: additive `internal/validate/oracle.go` exports
  `ExtractFileFacts` (a plain re-export of `extractFacts`; no behavior), and
  additive `tools/factsdump` emits the canonical dump — one parse, N relpath
  *personas* per invocation.
- **Facts differential** (`pdxl-parity/tests/facts.rs`): **13 fixtures × 8
  personas (one per def rule + on_action + no-match) — 104 extraction runs,
  byte-identical** to the Go oracle: defs (name/kind/file/offsets/params),
  aliases, refs (kind/name/byte range/`file:line:col`).
- **New stress fixtures** (`pdxl-parity/testdata/`): `facts_traits.txt`
  (groups, equivalences, params, namespace-metadata skip) and `facts_refs.txt`
  (every reference shape and every skip rule: quoted names, scope keywords,
  `scope:` chains, `$X$`, macro-concat prefixes, `100 = 0`, word-keyed config).
- **Previous suites re-run green**: lexer 11/11, parser 11/11, fileset 5/5,
  descriptors 6/6, golden trees 8/8.

## Tests
- Go: `go test ./...` green (only additive files).
- Rust: `cargo test --workspace` — 38 suites green. New: 15 extraction unit
  tests in `pdxl-ck3/tests/extract.rs` porting `validate_test.go`'s
  extraction-level assertions (definitions incl. namespace skip, characters,
  dotted IDs, sorted/deduped macro params, unknown dirs) plus focused coverage
  of every ref shape, every skip rule, alias quirks, `loc` format, and
  malformed-input (partial-tree) extraction; 1 new `line_col` test in
  `pdxl-src`.
- `cargo fmt --check` and `cargo clippy --workspace --all-targets
  --all-features -- -D warnings` clean; no `unsafe`.

## Bugs or ambiguities discovered
- None in the Go extraction logic. Quirks preserved deliberately: alias
  symbols carry `EndOffset == SrcStart`; the byte range of a quoted reference
  covers the quoted source text while `Name` is unquoted; `Loc` columns count
  bytes, not characters.

## Files changed
- Added: `rust/crates/pdxl-analysis/**`, `rust/crates/pdxl-ck3/**`,
  `rust/crates/pdxl-parity/{src/facts_dump.rs, src/bin/factsdump.rs,
  tests/facts.rs, testdata/facts_*.txt, examples/factsbench.rs}`,
  `internal/validate/oracle.go` (additive), `tools/factsdump/main.go`,
  `rust/docs/MILESTONE-5-REPORT.md`.
- Modified: `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/README.md`,
  `pdxl-src/src/lib.rs` (+`line_col`), `pdxl-parity/{Cargo.toml, src/lib.rs}`.
- Unchanged behaviorally: all Go production code.

## Risks for later milestones
- **Schema growth discipline**: adding CK3 rules changes what facts mean; bump
  `ANALYSIS_VERSION` and re-run the facts differential (the Go registry must
  change in lockstep while the oracle lives).
- **Recursive `extract_refs`** mirrors the parser's one-stack-frame-per-level
  shape; same depth caveat as M2 (fine for real script, noted for adversarial
  input).
- **M6 must preserve walk order**: duplicate-definition "first" stability
  depends on FileSet winner order feeding `mergeAndResolve` in order — already
  locked by the M3 differential.

## Recommendation for Milestone 6 (whole-project analysis) — do not begin
Port `SymbolTable` (first-writer-wins + `Duplicates`, alias gap-fill),
`resolveRefs` (`unknown <kind> "<name>"` diagnostics), `gatherFacts` (FileSet
walk → facts, no FactStore branch), and `Project` (`UpdateSource` re-parses one
file, replaces its facts, rebuilds the table from memory). Differential: an
additive Go `tools/projectdump` over a temp project tree comparing symbol
counts by kind, duplicates, and unresolved refs in order; reuse `factsbench`'s
synthesized corpus for an end-to-end `check` timing comparison against
`go run ./cmd/pdxl check`.
