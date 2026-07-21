# pdxl

A Rust toolkit and language server for Paradox Interactive scripting
(PDXScript, used by CK3, Victoria 3, EU5, …) and the Jomini interface dialect
(`.gui`). The grammar is shared across games; game semantics live in a
per-game schema crate (currently CK3).

## What it does

- **`pdxl lsp`** — the language server: live mod-scoped diagnostics,
  go-to-definition, find-references, hover (with game-doc and wiki-sourced
  documentation), completion (schema-driven for script, corpus-mined for
  gui), semantic highlighting, inlay hints, formatting. Editor integrations
  live under `editor/` (VS Code in `editor/vscode/`, Neovim in `editor/nvim/`,
  Zed in `editor/zed/`).
- **`pdxl check`** — one-shot project analysis: index every definition across
  game + mod (with Paradox mod-overlay semantics), resolve references, report
  duplicates, unresolved references, and gui datafunction errors.
- **`pdxl lex` / `pdxl parse` / `pdxl fmt`** — tokenizer, AST printer, and a
  comment-preserving formatter.

## Workspace layout

```
crates/
  pdxl-src        TextRange: zero-based, half-open byte ranges
  pdxl-path       lexical path helpers
  pdxl-lexer      byte-offset tokenizer
  pdxl-ast        flat node-pool syntax tree (data only)
  pdxl-parser     recursive-descent parser (+ .gui dialect entry)
  pdxl-fileset    directory scanning + mod-overlay resolution
  pdxl-moddesc    .mod descriptor parsing
  pdxl-cache      two-level parse cache
  pdxl-analysis   fact extraction engine (game-agnostic)
  pdxl-ck3        the CK3 schema: kinds, entities, doc tables, contexts
  pdxl-loc        Paradox localization .yml parser
  pdxl-gui        interface-script (.gui) analysis + datafunction typing
  pdxl-project    whole-project analysis + incremental updates
  pdxl-fmt        PDXScript formatter
  pdxl-cli        the pdxl binary
  pdxl-lsp        the language server
  pdxl-mcp        MCP server for agent-facing semantic queries
  pdxl-gamedocs   parsers for the game's doc dumps + gen-tables
  pdxl-testutil   shared test helpers
```

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Regression behavior is pinned by golden files, each living in the crate it
tests (`pdxl-lexer`, `pdxl-parser`, `pdxl-fileset`, `pdxl-ck3`, `pdxl-project`
under `tests/` + `testdata/goldens/`, plus `crates/pdxl-cli/tests/goldens`);
regenerate deliberately with `UPDATE_GOLDENS=1` and review the diff like code.

## History

pdxl began as a Go implementation, then was ported to Rust subsystem by
subsystem with byte-differential testing against the Go oracle (the port's
milestone reports live in `docs/`). The Go code has since been removed; the
last parity-verified outputs are pinned as the golden files above. Internally
everything uses zero-based, half-open byte ranges (`pdxl-src::TextRange`);
UTF-16 conversion happens only at the LSP boundary.
