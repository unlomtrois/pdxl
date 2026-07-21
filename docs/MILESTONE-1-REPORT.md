# Milestone Report — Rust Port: Lexer

## Status
Complete (Milestone 0 + Milestone 1).

## Commit
`1094c018129a6316ba7742354f68f163f78b927b` on branch `rust-port-milestone-0-1`
(reference Go commit: `5d02a979cb162ba5d89c7e705618de322884bd79`).

## Implemented
- **Milestone 0** — repository reconnaissance + parity harness:
  - Cloned the Go repo; read README, CLAUDE.md, ARCHITECTURE.md, Makefile, go.mod,
    and the lexer/parser/cache/files/validate layers.
  - Recorded the Go baseline (test results + lexer/parser/cache benchmarks,
    environment, commit SHA) in `rust/docs/BASELINE.md`.
  - Created the `rust/` Cargo workspace (edition 2024).
  - Added `scripts/parity.sh` (Go tests + Rust tests + differential lexer dump).
  - Added `rust/README.md` (oracle strategy, milestone status, how to run parity).
- **Milestone 1** — source model + lexer:
  - `pdxl-source`: `TextRange { start: u32, end: u32 }` (half-open byte range).
  - `pdxl-lexer`: faithful port of `internal/lexer` with exact token-kind and
    byte-range parity, including a port of Go's `utf8.DecodeRune`.
  - Deterministic token-dump tooling on both sides + a differential test.

## Architecture
- **Crates added:** `pdxl-source`, `pdxl-lexer`.
- **Public types:**
  - `pdxl_source::TextRange` — `Copy`, `Eq`, `Hash`; `new/from_usize/len/is_empty/
    as_range/slice`.
  - `pdxl_lexer::TokenKind` — one variant per Go `Tag`; `as_str()` returns the exact
    `Tag.String()` names so dumps are directly comparable.
  - `pdxl_lexer::Token { kind, range }` — no owned strings; `value(src)` slices.
  - `pdxl_lexer::Lexer<'src>` — `init` (skips BOM), `next_token` (yields every token
    incl. invalid). `tokenize(src)` mirrors Go's `Tokenize` (skips invalid/eof).
- **Dependency choices:** none beyond `pdxl-source`. Go's `utf8.DecodeRune` is
  reproduced byte-for-byte (the `first[256]` table + `acceptRanges`) rather than
  pulling a crate, because the required contract — *any* invalid UTF-8 →
  `RuneError` with size **1** — differs from both Rust std and `bstr` (maximal-
  subpart substitution), and the reported size drives token offsets. `no unsafe`
  is enforced via `unsafe_code = "forbid"` in both crates.

## Parity
- **Fixtures compared (11):** all 8 `testdata/*.txt`, `testdata/ck3/
  scripted_trigger_macro.txt`, `testdata/lint/advance_for_lint.txt`, and the
  Rust-side `stress.txt` (malformed UTF-8 `0xFF`/`0x80`/truncated `0xC3`, lone
  `!`/`?`, bare `$`/`@`, unterminated string, BC date, dot floats, scope chains).
- **Method:** for each fixture, `go run ./tools/lexdump` and the Rust lexer each
  emit `<kind>\t<start>\t<end>` per token (including invalid tokens); the streams
  are asserted byte-identical. Since a token's text is `source[start..end]`,
  identical offsets over identical source ⇒ identical slices (also checked
  independently in a Rust unit test).
- **Passing cases:** 11/11 byte-identical.
- **Mismatches:** none.

## Tests
- **Go:** `go test ./...` — green (unchanged; only the additive `tools/lexdump`
  package was introduced).
- **Rust:** `cargo test --workspace` — 43 lexer unit tests (ported 1:1 from
  `internal/lexer/lexer_test.go`) + 5 rune-decoder tests + 2 `pdxl-source` tests,
  all passing.
- **Differential:** `tests/parity.rs` — 11 fixtures, 0 mismatches. Self-skips with
  a warning if `go` is not on PATH.
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets --all-features`
  clean.

## Benchmarks
| Case | Go | Rust | Difference |
|------|----|------|------------|
| LexLarge (international_organization.txt, 24401 B) | ~205 MB/s, 18432 B/op, 768 allocs/op | ~280 MB/s, 0 allocs/token | Rust ~1.37× faster, no per-token allocs |
| advance.txt (211 B) | ~164 MB/s | ~251 MB/s | Rust faster |
| modifier_types.txt (1617 B) | ~185 MB/s | ~260 MB/s | Rust faster |

Methodologies differ (Go `testing.B` vs a simple Rust throughput loop), so treat
these as order-of-magnitude parity, not a tuned comparison. No regression.

## Deviations from Go
- Go returns a heap `*Token` from `Next()`; Rust returns `Option<Token>` by value
  (no per-token allocation). Observable token kinds/ranges are identical.
- `tools/lexdump` (Go) is **new, additive** tooling — it does not alter existing
  Go behavior. It dumps every token from raw `Next()` (including invalid), unlike
  `lexer.Tokenize`, which skips invalid/eof.
- Offsets are `u32` in Rust (Go uses `int`); fine for sub-4 GiB script files and
  matches the target type in the porting spec.

## Risks discovered
- **Rune-decoder parity is load-bearing.** Any future swap to a crate-based UTF-8
  decoder could silently shift offsets on malformed input. The differential
  `stress.txt` guards this; keep it.
- **`comment` and `eof` token kinds are defined but never produced** by either
  implementation (comments are consumed in `skip_whitespace`; EOF is `None`). They
  exist for kind-name parity only.
- **Differential test depends on the `go` toolchain** at `cargo test` time. It
  skips gracefully when absent, so a Rust-only CI would not actually verify
  parity — parity must run where Go is available.

## Files changed
- Added: `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/README.md`,
  `rust/docs/BASELINE.md`, `rust/docs/MILESTONE-1-REPORT.md`,
  `rust/crates/pdxl-source/**`, `rust/crates/pdxl-lexer/**`
  (`src/lib.rs`, `src/rune.rs`, `src/tests.rs`, `src/bin/lexdump.rs`,
  `examples/lexbench.rs`, `tests/parity.rs`, `testdata/stress.txt`),
  `tools/lexdump/main.go`, `scripts/parity.sh`.
- Modified: `.gitignore` (ignore `rust/target`, stray `lexdump` binary).
- Unchanged: all existing Go source under `cmd/`, `internal/`.

## Next milestone
**Milestone 2 — Parser v3 (flat node pool). Do not begin yet.** Recommendations:
1. Add a `pdxl-syntax` crate depending on `pdxl-lexer`. Port the flat layout
   exactly: `NodeId(u32)`, `Node { kind, range, operator: TokenKind, child_start,
   child_end }`, `SyntaxTree { source: Arc<[u8]>, nodes: Box<[Node]>, child_ids:
   Box<[NodeId]> }`. Node 0 is the `File` root; children live in a separate index
   array; scalar text is recovered from source ranges.
2. Preserve invariants: parsing always returns a tree; syntax errors accumulate as
   `Diagnostic { offset, msg, severity }`; valid input allocates no diagnostics
   buffer; traversal is allocation-free by default. Port the Pratt binding powers
   (`.`/`:`/`|` at bp 80) and `synchronize()` recovery verbatim.
3. Extend the parity harness with a structured tree dump (node kind, source range,
   operator, immediate child IDs, diagnostics+offsets) compared against the Go
   `*_test.go` goldens and the existing `testdata/*.golden` renders. Add a Go
   `tools/parsedump` analogous to `tools/lexdump`.
4. Reuse `TextRange` and `TokenKind`; do not redesign the grammar, introduce Rowan,
   or add incremental parsing — parity first.
