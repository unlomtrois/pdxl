# pdxl

A toolkit for working with Paradox Interactive scripting files — the format used by games like EU5, CK3, and Victoria 3.

Existing tools are either outdated or single-purpose CI validators. pdxl aims to be a foundation for editor tooling and AI-assisted modding workflows.

> Early experiment. Expect breaking changes.

## Status

| Component | Status |
|-----------|--------|
| Lexer | Working |
| Parser | Planned |
| Validator | Planned |
| LSP server | Planned |
| VS Code extension | Planned |
| MCP server | Planned |

## Install

```sh
go install pdxl/cmd/pdxl@latest
```

Or build from source:

```sh
git clone https://github.com/unlomtrois/pdxl
cd pdxl
make build
```

## Usage

```sh
pdxl <file.txt>
```

Prints each token with its file position:

```
common/characters/my_char.txt:1:1: identifier
common/characters/my_char.txt:1:5: colon
...
```

## Development

Requires Go 1.25+. Nix users: `nix-shell` provides the full toolchain.

```sh
make test   # run tests
make lint   # run golangci-lint
make build  # build binary to bin/
```

## License

MIT
