# pdxl

A toolkit for working with Paradox Interactive scripting files — the format used by games like EU5, CK3, and Victoria 3.

Existing tools are either outdated or single-purpose CI validators. pdxl aims to be a foundation for editor tooling and AI-assisted modding workflows.

> Early experiment. Expect breaking changes.

## Status

| Component | Status |
|-----------|--------|
| Lexer | Working — ~135 MB/s; full CK3 syntax (`@` script values & inline math, dates, paths, …) |
| Parser | Working — three implementations for benchmarking (see below) |
| File scanning | Working — mod-overlay resolution (`replace_path`, `.mod`/Proton paths) |
| Cache | Working — two-level parse cache (in-memory LRU + on-disk) + per-file symbol cache |
| Validator | Working — cross-file definition indexing + reference resolution (CK3) |
| LSP server | Planned |
| MCP server | Planned |

On the full CK3 game plus a total-conversion mod (~3,500 files), `pdxl` parses
with zero diagnostics and a warm `pdxl check` runs in under a second.

## Install

```sh
go install github.com/unlomtrois/pdxl/cmd/pdxl@latest
```

Or build from source:

```sh
git clone https://github.com/unlomtrois/pdxl
cd pdxl
make build
```

## Usage

**Lex a file** — print tokens with positions:

```sh
pdxl lex common/characters/my_char.txt
```

**Parse a file** — print the AST:

```sh
pdxl parse --tree common/international_organizations/foo.txt
pdxl parse --json common/international_organizations/foo.txt
```

**Lint** — structural diagnostics for files or directories:

```sh
pdxl lint common/traits/00_traits.txt
pdxl lint common/                       # recurses; --context N for source lines
```

**Check a whole project** — index definitions and resolve cross-file references
(undefined traits, events, on_actions). Game and mod are overlaid with Paradox
`replace_path`/load-order semantics:

```sh
pdxl check --game /path/to/ck3/game --mod /path/to/MyMod.mod
```

`pdxl index` scans and parses the whole project (with a progress bar) and reports
file/diagnostic counts. `pdxl cache size [--detailed]` / `pdxl cache clear`
inspect the on-disk caches.

Project defaults (game path, mod path, ignored files) live in `pdxl.toml`; run
`pdxl init` to create one.

## Parser implementations

Three parser variants live in `internal/parser/` for benchmarking comparison:

| Package | Design | ~Throughput | ~Allocs/op |
|---------|--------|-------------|------------|
| `v1` | participle (reference baseline) | 2 MB/s | 19 000 |
| `v2` | hand-written Pratt, pointer-tree AST | 104 MB/s | 2 300 |
| `v3` | hand-written Pratt, flat node pool | 115 MB/s | 880 |

The CLI and validator use `v3` (preferred for new tools). Run `make bench-parser`
to reproduce.

## Development

Requires Go 1.25+. Nix users: `nix-shell` provides the full toolchain.

```sh
make test          # run all tests
make lint          # run golangci-lint
make build         # build binary to bin/pdxl
make bench         # benchmark lexer + all parser variants
make bench-lexer   # lexer only
make bench-parser  # parser v1/v2/v3 comparison
```

## License

MIT
