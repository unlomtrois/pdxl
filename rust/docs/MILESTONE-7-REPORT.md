# Milestone Report — Rust Port: CLI (lex, parse, check)

## Status
Complete (incremental CLI scope: `lex`, `parse`, `check`; `lint`/`index`/
`cache`/`watch` remain future work, per the roadmap's port-incrementally rule).

## Branch
`rust-port-milestone-7` (based on `8155ece`).

## Reference Go commit
`5d02a979cb162ba5d89c7e705618de322884bd79` (+ two lockstep bug fixes below).

## Implemented
- **`pdxl-cli`** (new crate, `clap`): binary `pdxl` with
  - `lex <file> [--tags] [--show-pos]` — Go output byte-identical, including
    the `%-17s` tag column and `basename:line:col: invalid "…"` lines;
  - `parse <file> [--tree]` — flat printer and box-drawing tree printer, Go
    output byte-identical; diagnostics to stderr as `file:line:col: msg`;
  - `check [file] --game --mod [--no-cache]` — report format (`%-18s %6d`
    counts, duplicates, unresolved), exit codes, single-file mode, `.mod`
    resolution with replace_paths, `[scan]` ignore defaults = Go
    `config.Default()`.
- **AST cache wiring** as opt-in API: `pdxl_project::analyze_with(fs, schema,
  Option<&Store>)` (mirrors Go `parseEntry`); correctness covered by
  `analyze_with_cache_matches_uncached`.

## Lockstep bug fixes (user-directed + found during the milestone)
1. **`ParseMod` Unix absolute paths** (user-directed): Go only special-cased
   Windows absolute `path=` values; the Linux launcher writes absolute Unix
   paths, so real descriptors (the user's actual `T4N.mod`) failed with a
   mangled joined path. Fixed in Go (`IsWindowsAbsolute(raw) ||
   filepath.IsAbs(raw)`) and Rust (`|| raw.starts_with('/')`) with tests both
   sides + a new descriptor-differential case (7/7). The real `T4N.mod` now
   loads in both implementations.
2. **Go `pdxl parse` panicked on any malformed file** (found by the snapshot
   harness): `Diagnostic.String()` passes a nil source to `FormatPosition`,
   whose line/col derivation indexes the source → index-out-of-range for any
   diagnostic past offset 0. Fixed minimally at the `cmd/pdxl/parse.go` call
   site (format against the in-scope `data`), matching the v3 doc comment's
   stated intent. `Diagnostic.String()` itself remains a footgun for other
   callers (`watch`?) — recommended deeper fix upstream.

## Measured decision: `check` does NOT use the AST cache
The M6 recalibration hypothesized the AST cache would pay for warm `check`.
Measured on the real corpus (vanilla + T4N, ~195 MB): warm AST-cached run
≈ 4.1 s vs 4.0 s cold — no benefit, plus 212 MB of disk. Cause: both paths
read + SHA-256 every file, and decoding stored trees costs as much as parsing;
Go's 0.32 s warm path was entirely its tiny-entry **FactStore**, not the AST
cache. The CLI therefore runs cache-free (`--no-cache` accepted as a
compatibility no-op); `analyze_with` stays as a tested opt-in API. If a fast
warm `check` is ever wanted, the right tool is a facts cache (tiny entries,
keyed content + `ANALYSIS_VERSION` + rel_path) — not the AST cache.

## Parity
- **CLI snapshots** (`pdxl-cli/tests/cli.rs`): 21 lex/parse invocations over 4
  fixtures (incl. the malformed one) + 3 `check` scenarios (project report
  with duplicates/unresolved via a .mod descriptor exercising replace_path and
  the fixed absolute path; single-file mode; clean project) — stdout
  byte-identical, exit codes matching. Determinism test for repeated runs.
- **All differential suites green**: lexer 11/11, parser 11/11, fileset 5/5,
  descriptors 7/7 (new Unix-absolute case), facts 13×8, project 4/4.
- Real corpus: Rust `pdxl check` on vanilla + real `T4N.mod` reproduces the
  known report (24,232 symbols, 23 duplicates, 1 unresolved, exit 1).

## Tests
Go `go test ./...` green (files fix + new `TestParseModUnixAbsolutePath`).
Rust: 45 suites green; fmt + clippy `-D warnings` clean; no `unsafe`.
New dependency: `clap` (derive) for the user-facing binary only.

## Deviations from Go (all documented in-code)
- No `pdxl.toml` loading (built-in defaults equal `config.Default()`).
- No Proton path resolution (M3 decision).
- `parse --json` not ported (its shape is Go's internal struct encoding).
- `check` runs cache-free (measured; see above).

## Files changed
- Added: `rust/crates/pdxl-cli/**`, `rust/docs/MILESTONE-7-REPORT.md`.
- Modified: `internal/files/files.go` (+`filepath.IsAbs` branch),
  `internal/files/files_test.go` (+test), `cmd/pdxl/parse.go` (panic fix),
  `rust/crates/pdxl-moddesc/{src/lib.rs, tests/descriptor.rs}`,
  `rust/crates/pdxl-project/src/lib.rs` (+`analyze_with`),
  `rust/crates/pdxl-parity/tests/fileset.rs` (+descriptor case),
  `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/README.md`.

## Recommendation for Milestone 8 (LSP) — do not begin
The original port motivation. `AnalysisHost` owning one `Project` behind a
lock, immutable snapshots for request handlers; decide `tower-lsp-server` vs
`lsp-server` after documenting the cancellation/concurrency model; UTF-16
conversion enters at the protocol boundary only. Feature order: init/shutdown
→ didChange + debounced mod-scoped diagnostics → go-to-definition →
references. The 4 s cold project build (≈2 s threaded) happens once per
session in `initialized`, matching Go's async build.
