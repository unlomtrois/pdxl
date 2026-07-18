# pdxl — Zed extension

PDXScript (Paradox / Crusader Kings III) language support for [Zed](https://zed.dev),
powered by the `pdxl` language server.

## Features

Backed by `pdxl lsp` (the same server the VS Code extension uses):

- Diagnostics for unresolved references (mod-scoped)
- Go to definition, Find All References
- Hover (symbol info, built-in effect/trigger/scope-link docs)
- Completion (structural fields, effects, triggers, scope links)
- Inlay hints (best-effort scope annotations, localization text)
- Format Document
- Syntax highlighting via [tree-sitter-paradox](https://github.com/acture/tree-sitter-paradox)

**Not available in Zed:** the reference-count *CodeLens* (Zed does not support
CodeLens as a general LSP feature). Use **Find All References** instead.

## Setup

1. Build and install the server so `pdxl` is on your `PATH`:
   ```sh
   cd ../../rust && cargo build --release -p pdxl-cli
   # copy target/release/pdxl onto PATH, or point binary.path at it (below)
   ```
2. Install this extension: Zed → Extensions → **Install Dev Extension** → select
   this `editor/zed/` directory.
3. Point the server at your vanilla game files in Zed `settings.json`:
   ```json
   {
     "lsp": {
       "pdxl": {
         "initialization_options": {
           "gamePath": "/path/to/Crusader Kings III/game"
         }
       }
     }
   }
   ```

Open a CK3 mod folder in Zed — the mod directory is taken from the workspace
root automatically. `.txt` files are treated as PDXScript.

### Richer highlighting (semantic tokens)

The server provides schema-aware highlighting via LSP semantic tokens, which is
far more precise than the tree-sitter grammar alone (it distinguishes keys,
values, numbers, strings, booleans, comments, and macro params). Enable it per
language in `settings.json`:

```json
{
  "languages": {
    "PDXScript": { "semantic_tokens": "full" }
  }
}
```

`"full"` makes the server tokens the only color source (recommended — the
bundled tree-sitter grammar is weak at highlighting and is used only for
structure: indentation, folding, bracket matching). Use `"combined"` if you
want the grammar's coloring layered underneath the server tokens.

## Settings

Under `lsp.pdxl` in Zed `settings.json`:

| Key | Purpose | Default |
|-----|---------|---------|
| `binary.path` | path to the `pdxl` executable | `pdxl` on `PATH` |
| `binary.arguments` | server arguments (must include the `lsp` subcommand) | `["lsp", "--log-level", "info"]` |
| `initialization_options.gamePath` | vanilla CK3 `game/` directory | (unset) |
