# pdxl — Rust port

This directory contains the incremental Rust port of `pdxl`. The work proceeds
**subsystem by subsystem**; the Go implementation in the repository root remains
the reference until each subsystem reaches documented parity.

## The Go implementation is the oracle

Every ported subsystem is validated against the existing Go code through
**differential testing**: the same fixtures are fed to both implementations and
their normalized outputs are compared byte-for-byte. The Go code is not modified
behaviorally during the port — only additive tooling (e.g. `tools/lexdump`, a
deterministic token-dump used by the parity tests) is introduced.

A subsystem is "done" only once it has equivalent fixtures, differential tests,
acceptable performance, and documented behavioral parity. Architectural
improvements (Rowan, Salsa, incremental parsing, zero-copy on-disk formats) are
explicitly **out of scope** until parity exists — the first goal is to reproduce
behavior exactly.

## Internal model

Like the Go code, the Rust port uses **zero-based, half-open byte ranges**
(`[start, end)`) everywhere internally — see `pdxl-source::TextRange`. UTF-16
conversion happens only at the LSP protocol boundary (a later milestone), never
in the lexer, syntax tree, or analysis layers.

## Milestone status

| Milestone | Subsystem | Status |
|-----------|-----------|--------|
| 0 | Reconnaissance + parity harness | Done |
| 1 | Source model + lexer (`pdxl-source`, `pdxl-lexer`) | Done — byte-for-byte parity |
| 2 | Parser v3 (flat node pool) | Not started |
| 3+ | FileSet, cache, analysis, CK3 rules, CLI, LSP | Not started |

## Crates

- `pdxl-source` — `TextRange` and source-offset primitives.
- `pdxl-lexer` — byte-offset tokenizer (port of `internal/lexer`), plus a
  `lexdump` binary that emits a deterministic `<kind>\t<start>\t<end>` token dump.

Crates are added only when their milestone begins.

## Running parity tests

From the repository root:

```sh
# Go oracle tests + Rust tests + lexer differential comparison
scripts/parity.sh

# Just the lexer differential dump comparison
scripts/parity.sh --lex
```

From this directory (Rust only — the differential test self-skips if `go` is not
on PATH):

```sh
cargo test --workspace          # unit + differential parity tests
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

The lexer differential test (`crates/pdxl-lexer/tests/parity.rs`) runs
`go run ./tools/lexdump` over every shared fixture in `testdata/` (plus Rust-side
stress fixtures with malformed UTF-8) and asserts the Rust lexer produces an
identical token stream — same kind, same byte offsets. The token's source slice
is `source[start..end]` by construction, so matching offsets over identical
source bytes is a complete token-stream comparison.
