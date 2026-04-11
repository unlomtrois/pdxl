# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
make test    # run all tests
make lint    # run golangci-lint
make build   # build binary to bin/pdxl
```

To run a single test:
```sh
go test ./internal/lexer/... -run TestUTF8Identifier
```

Always use `make test` to verify changes, never raw `go test` or `go build`.

Nix users: activate the toolchain with `nix-shell` before running make commands.

## Architecture

pdxl is a toolkit for parsing Paradox Interactive scripting files (used in EU5, CK3, Victoria 3, etc.). The project is in early development — currently only a lexer exists.

```
cmd/pdxl/           — CLI binary: reads a file, prints tokens with positions
internal/lexer/     — lexer package (not public API yet)
  lexer.go          — Lexer struct, Next(), and all scanning helpers
  token.go          — Token struct, Tag enum, position utilities
  lexer_test.go     — tests; use testTokenize() helper for tag sequence assertions
pkg/mod/            — reserved for future public packages
```

### Lexer design

- `Lexer` holds `source []byte` and `pos int` (byte offset, not rune index).
- `Token.Start`/`Token.End` are byte offsets; `GetValue(source)` slices correctly for UTF-8.
- `advance()` returns `(rune, int)` via `utf8.DecodeRune` — supports multi-byte characters. Invalid UTF-8 bytes produce `utf8.RuneError` (> 127), silently treated as identifier chars (no validation).
- `peek()` returns `rune` without consuming.
- `Next()` dispatch: outer `switch {}` separates digit-start, `isIdentifierStart` (ASCII letters, `_`, or any non-ASCII rune), and a nested `switch c` for single-character operators.
- `yes`/`no` keyword detection happens in `Next()` after `lexIdentifier()` returns, using `string(source[startPos:pos])`.
- UTF-8 BOM (`\xEF\xBB\xBF`) is detected in `Init()` and skipped via initial `pos` offset.

### Token naming

Tag constants use `snake_case` (e.g. `literal_boolean`, `l_brace`). The linter's `var-naming` rule is disabled to allow this.

### Commit style

Use git-flow prefixes: `feat(scope):`, `fix(scope):`, `chore(scope):`, etc.
