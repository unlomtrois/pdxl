# pdxl

A toolkit for working with Paradox Interactive scripting files — the format used by games like EU5, CK3, and Victoria 3.

Existing tools are either outdated or single-purpose CI validators. pdxl aims to be a foundation for editor tooling and AI-assisted modding workflows.

> Early experiment. Expect breaking changes.

## Status

| Component | Status |
|-----------|--------|
| Lexer | Working — ~135 MB/s |
| Parser | Working — three implementations for benchmarking (see below) |
| Cache | Planned |
| Validator | Planned |
| LSP server | Planned |
| MCP server | Planned |

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
pdxl parse common/international_organizations/foo.txt
pdxl parse --json common/international_organizations/foo.txt
```

## Parser implementations

Three parser variants live in `internal/parser/` for benchmarking comparison:

| Package | Design | ~Throughput | ~Allocs/op |
|---------|--------|-------------|------------|
| `v1` | participle (reference baseline) | 2 MB/s | 19 000 |
| `v2` | hand-written Pratt, pointer-tree AST | 104 MB/s | 2 300 |
| `v3` | hand-written Pratt, flat node pool | 115 MB/s | 880 |

The CLI uses `v2`. Run `make bench-parser` to reproduce.

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
