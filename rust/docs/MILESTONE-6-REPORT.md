# Milestone Report — Rust Port: Whole-Project Analysis

## Status
Complete.

## Branch
`rust-port-milestone-6` (based on `5be76ee`, the M5 facts extraction).

## Reference Go commit
`5d02a979cb162ba5d89c7e705618de322884bd79`

## Scope
Ports the remaining half of `internal/validate`: `SymbolTable`, `resolveRefs`,
`mergeAndResolve`, `gatherFacts`/`Analyze`, and the incremental `Project`. With
this, the entire Go pipeline below the CLI/LSP layer exists in Rust:
FileSet → parse → facts → symbol table → unresolved-reference diagnostics,
with single-file incremental updates.

## Implemented
- **`pdxl-analysis` additions** (pure, no new deps):
  - `SymbolTable` — `by_kind` maps with **first-writer-wins** merge, a
    `Duplicate` record per redefinition, alias **gap-fill** (`add_alias` never
    shadows or duplicate-tracks), `count`/`total`/`lookup`.
  - `RefDiag` + `resolve_refs` — `unknown <kind> "<name>"` messages (Go's
    `unknown %s %q`).
  - `merge_and_resolve(order, facts)` — defs+refs in walk order first
    (duplicate "first" stability), aliases second, then resolution. Pure and
    in-memory, exactly like Go.
- **`pdxl-project`** (new crate; deps: analysis, fileset, parser, path):
  - `analyze(fs, schema)` — Go's `Analyze(fs, nil, nil)`: walk winners, read,
    parse, extract, merge, resolve.
  - `Project` — owns walk order + facts map + table + diags; `update(path)`
    (re-read one file from disk), `update_source(path, buffer)` (unsaved
    editor buffer; disk untouched), both re-parse **only that file** and
    rebuild the table from in-memory facts; `table()`, `diags()`,
    `file_diags()` (loc-prefix match, Go parity), `facts_at()`,
    `references(kind, name)` (walk order), `rel_to_full()`; `key_for` compares
    cleaned absolute paths (no symlink resolution, like Go `filepath.Abs`).
  - The schema is an explicit constructor parameter (Go hardcodes the CK3
    registry); `pdxl-project` itself is game-agnostic.

## Deliberate deviations from Go
- **No `FactStore`** in the gather path (M5 decision, benchmark-backed).
- **No AST-cache wiring** in `gather_facts` yet: the end-to-end measurement
  below shows the cold path outrunning Go's cached path; `pdxl-cache` remains
  available for the LSP milestone if warm-start profiling asks for it.
- `Project` stores its `Schema` by value; Go reads package-level rule maps.

## Parity
- **Oracle**: additive `tools/projectdump` (same `--root path:kind` flag style
  as `filesetdump`) running `validate.Analyze(fs, nil, nil)`; Rust twin
  `projectdump` + `pdxl_parity::dump_project`.
- **Project differential** (`pdxl-parity/tests/project.rs`): 4 scenarios
  byte-identical — (1) cross-file/kind resolution with aliases, quoted refs,
  on_action lists and weighted blocks; (2) duplicates with stable "first"
  ordering across three files; (3) **overlay interplay**: a mod file shadowing
  a vanilla file removes both its definitions *and* its aliases, produces no
  false duplicates, and surfaces the newly unresolved refs; (4) skip rules end
  to end (macros/scopes/concat never diagnose). Compared fields: counts per
  kind (stable order) + total, duplicates (kind/name/first_file/file) in merge
  order, unresolved diagnostics (file/start/end/loc/msg) in walk order.
- **All previous suites re-run green**: lexer 11/11, parser 11/11, fileset 5/5,
  descriptors 6/6, facts 13×8, golden trees 8/8.

## Tests
- Go: `go test ./...` green (only additive `tools/projectdump`).
- Rust: `cargo test --workspace` — 43 suites green. New: 15 tests in
  `pdxl-project/tests/project.rs` — all 7 `resolve_test.go` cases and all 5
  `project_test.go` cases ported 1:1, plus: untracked-file errors,
  `references`/`rel_to_full`, and the property test
  **`incremental_equals_fresh_analysis`** (after a single-file edit, table
  totals, per-kind counts, duplicates, and diagnostics equal a from-scratch
  analysis of the edited corpus — the invariant the whole design rests on).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features
  -- -D warnings` clean; no `unsafe`.

## End-to-end measurement (synthesized corpus: 3,500 files, 800 traits, 1,200 events, ~51k refs)
| | cold, no caches |
|---|---|
| Go `pdxl check --no-cache` | 116 ms |
| Rust `projectdump` (single-threaded) | **75 ms** |

Identical symbol totals (5,450). Both are well under a second cold, re-confirming
the M5 no-FactStore decision at the whole-pipeline level.

## Bugs or ambiguities discovered
- None new. One behavior worth knowing (present in Go, faithfully ported):
  `file_diags` matches by `loc` string prefix (`<full path>:`), so two tracked
  files where one path is a prefix of another *plus* a colon could in theory
  cross-match; not reachable with real paths since the prefix ends at `:`
  followed by a line number.

## Files changed
- Added: `rust/crates/pdxl-project/**`, `rust/crates/pdxl-analysis/src/{table,
  resolve}.rs`, `rust/crates/pdxl-parity/{src/project_dump.rs,
  src/bin/projectdump.rs, tests/project.rs}`, `tools/projectdump/main.go`,
  `rust/docs/MILESTONE-6-REPORT.md`.
- Modified: `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/README.md`,
  `pdxl-analysis/src/lib.rs`, `pdxl-parity/{Cargo.toml, src/lib.rs}`.
- Unchanged behaviorally: all Go production code.

## Risks for later milestones
- **`SymbolTable` iteration**: `by_kind` is a `HashMap`; anything that ever
  *iterates* symbols (a future CLI listing) must sort explicitly — only
  counts/lookups are exposed today, so no nondeterminism can leak.
- **`Project` is single-threaded by design** (`&mut self` updates). The LSP
  layer must own exactly one behind its own lock, as Go's server does; the
  Go server's non-reentrant-mutex discipline translates to "don't hold the
  lock across await points" in an async Rust server.
- **Adding/removing files** still requires a fresh FileSet scan + `Project::new`
  (Go parity). The watch/LSP layer should rebuild on directory events.

## Recommendation for next milestones — do not begin
The analysis engine is now complete end to end. Two directions, per the
original roadmap:
- **M7 (CLI)**: port commands incrementally over the existing crates — `lex`,
  `parse`, `lint`, `check` first (all are thin wrappers now; `check` ≈
  `projectdump` with human output + exit codes). Snapshot-test CLI output
  against the Go binary where machine-readable.
- **M8 (LSP)**: the original port motivation. `AnalysisHost` owning one
  `Project` behind a lock, serving immutable snapshots; document the
  cancellation/concurrency model before picking `tower-lsp-server` vs
  `lsp-server`; UTF-16 conversion enters here and only here. Features one at a
  time: init/shutdown → didChange+diagnostics (200 ms debounce, mod-scoped) →
  go-to-definition → references.
