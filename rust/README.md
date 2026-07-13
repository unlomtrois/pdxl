# pdxl — Rust port

This directory contains the incremental Rust port of `pdxl`. The work proceeds
**subsystem by subsystem**; the Go implementation in the repository root remains
the reference until each subsystem reaches documented parity.

## The Go implementation is the oracle

Every ported subsystem is validated against the existing Go code through
**differential testing**: the same fixtures are fed to both implementations and
their normalized outputs are compared byte-for-byte. The Go code is not modified
behaviorally during the port — only additive tooling (the dump tools under
`tools/`) is introduced.

A subsystem is "done" only once it has equivalent fixtures, differential tests,
acceptable performance, and documented behavioral parity. Architectural
improvements (Rowan, Salsa, incremental parsing, zero-copy on-disk formats) are
explicitly **out of scope** until parity exists — the first goal is to reproduce
behavior exactly.

## Internal model

Like the Go code, the Rust port uses **zero-based, half-open byte ranges**
(`[start, end)`) everywhere internally — see `pdxl-src::TextRange`. UTF-16
conversion happens only at the LSP protocol boundary (a later milestone), never
in the lexer, syntax tree, or analysis layers.

## Milestone status

| Milestone | Subsystem | Status |
|-----------|-----------|--------|
| 0 | Reconnaissance + parity harness | Done |
| 1 | Source model + lexer (`pdxl-src`, `pdxl-lexer`) | Done — byte-for-byte parity |
| 2 | Parser v3, flat node pool (`pdxl-ast`, `pdxl-parser`) | Done — exact node-layout & diagnostic parity |
| 3 | FileSet & overlay resolution (`pdxl-path`, `pdxl-fileset`, `pdxl-moddesc`) | Done — exact entry-order, stats & descriptor parity |
| 4 | Syntax cache (`pdxl-cache`) | Done — Go semantics + mandated hardening (versioned entries, atomic writes, race-free L1) |
| 5+ | Analysis, CK3 rules, CLI, LSP | Not started |

## Crates

Dependency direction (production crates only; each depends only on those above):

```
pdxl-src        TextRange: zero-based, half-open byte ranges
pdxl-path       Go-compatible lexical path helpers (Clean/Join, overlay keys)
pdxl-lexer      byte-offset tokenizer                      (dep: src)
pdxl-ast        flat node-pool syntax tree, data only      (dep: src, lexer)
pdxl-parser     recursive-descent parser → pdxl-ast trees  (dep: src, lexer, ast)
pdxl-fileset    scanning + mod-overlay resolution          (dep: path)
pdxl-moddesc    .mod descriptor parsing                    (dep: parser, path)
pdxl-cache      two-level parse cache (LRU + disk)         (dep: ast, parser, path;
                                                            serde/postcard/sha2)
```

Test / parity infrastructure (never a production dependency):

- `pdxl-testutil` — shared test helpers (temp trees, repo-root discovery, Go
  toolchain probe). Dev-dependency only.
- `pdxl-parity` — the Go-oracle differential harness: canonical dump formats,
  the `lexdump` / `parsedump` / `filesetdump` binaries (each with a same-format
  Go twin under `tools/`), and the differential tests. Depends on all production
  crates; nothing depends on it. When the Go oracle retires, this crate can
  shrink or disappear without touching anything else.

Notable splits: `pdxl-ast` holds the tree *data model* apart from the parsing
*algorithm* so future tree consumers (the syntax cache, semantic analysis) can
depend on the stable layout without the parser. `pdxl-moddesc` is separate from
`pdxl-fileset` so the scanner stays parser-free — descriptor parsing is the only
place the project model touches `pdxl-parser`.

The Proton/`drive_c` path translation from Go's `internal/files` is intentionally
not ported (mods are referenced by local folder path).

Crates are added only when their milestone begins.

## Running parity tests

From the repository root:

```sh
# Go oracle tests + Rust tests + lexer/parser differential comparisons
scripts/parity.sh

# Individual differential dump comparisons
scripts/parity.sh --lex
scripts/parity.sh --parse
```

From this directory (Rust only — the differential tests self-skip if `go` is not
on PATH):

```sh
cargo test --workspace          # unit + differential parity tests
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

The differential tests live in `crates/pdxl-parity/tests/`. Each runs the
corresponding Go dump tool (`go run ./tools/<name>`) over the shared fixtures in
`testdata/` (plus Rust-side stress fixtures with malformed UTF-8 under
`crates/pdxl-lexer/testdata/`) and asserts byte-identical output. A token's or
node's source slice is `source[start..end]` by construction, so matching offsets
over identical source bytes is a complete comparison.
