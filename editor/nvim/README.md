# pdxl.nvim

Neovim integration for the [pdxl](../../README.md) language server: diagnostics,
go-to-definition, find-references, hover, completion, inlay scope hints,
semantic tokens, CodeLens, and formatting for CK3 PDXScript, the Jomini `.gui`
dialect, and Paradox localization `.yml`.

Neovim ships a built-in LSP client, so this is thin: filetype detection plus a
`vim.lsp.config` wiring of `pdxl lsp`. No build step.

## Requirements

- **Neovim 0.11+** (native `vim.lsp.config` / `vim.lsp.enable`).
- The **`pdxl`** binary on your `PATH` (or point `cmd` at it). Build it with
  `cargo build --release -p pdxl-cli` and put `target/release/pdxl` on `PATH`,
  or grab a binary from the GitHub Releases.

## Install

### lazy.nvim

The plugin lives in the `editor/nvim` subdirectory of the repo, so use lazy's
`rtp` field to make that the runtime path root:

```lua
{
  'unlomtrois/pdxl',
  rtp = 'editor/nvim',
  config = function()
    require('pdxl').setup({
      game_path = '~/.local/share/Steam/steamapps/common/Crusader Kings III/game',
    })
  end,
}
```

### Manual (any setup)

Clone the repo and add the subdirectory to your runtime path, then call setup:

```lua
vim.opt.runtimepath:append('/path/to/pdxl/editor/nvim')
require('pdxl').setup({ game_path = '/…/Crusader Kings III/game' })
```

### Copy-paste (no plugin, no dependency)

Drop this straight into your `init.lua` — it reproduces what the plugin does:

```lua
vim.filetype.add({
  extension = { gui = 'pdxgui' },
  pattern = {
    ['.*/[Cc]ommon/.*%.txt']    = 'pdxscript',
    ['.*/events/.*%.txt']       = 'pdxscript',
    ['.*/history/.*%.txt']      = 'pdxscript',
    ['.*/gfx/.*%.txt']          = 'pdxscript',
    ['.*/localization/.*%.yml'] = 'pdxloc',
  },
})

vim.lsp.config('pdxl', {
  cmd = { 'pdxl', 'lsp', '--log-level', 'info' },
  filetypes = { 'pdxscript', 'pdxgui', 'pdxloc' },
  root_markers = { 'descriptor.mod', '.metadata', '.git' },
  init_options = { gamePath = vim.fn.expand('~/…/Crusader Kings III/game') },
})
vim.lsp.enable('pdxl')
```

## Configuration

`require('pdxl').setup({ … })` accepts:

| Option | Default | Meaning |
| --- | --- | --- |
| `game_path` | `nil` | Vanilla game dir, forwarded as `initializationOptions.gamePath` so references resolve against base-game definitions. `~` is expanded. |
| `cmd` | `{ 'pdxl', 'lsp', '--log-level', <log_level> }` | Full launch command. Override to point at a specific binary. |
| `log_level` | `'info'` | `--log-level` when `cmd` is not overridden. |
| `filetypes` | `{ 'pdxscript', 'pdxgui', 'pdxloc' }` | Filetypes the server attaches to. |
| `root_markers` | `{ 'descriptor.mod', '.metadata', '.git' }` | Markers identifying the mod root (becomes the server's mod directory). |
| `auto_enable` | `true` | Call `vim.lsp.enable` so the server autostarts. |
| `server_name` | `'pdxl'` | Config/client name shown in `:checkhealth vim.lsp`. |

## How filetype detection works

PDXScript lives in generic `.txt` files, so mapping `.txt` to a filetype
globally would hijack every text file. Instead the plugin claims `pdxscript`
only for `.txt` under the CK3 tree directories (`common/`, `events/`,
`history/`, `gfx/`) — the same routing pdxl's FileSet uses. `.gui` becomes
`pdxgui`; localization `.yml` (under `localization/`) becomes `pdxloc`. Random
project `.txt`/`.yml` files stay untouched.

If detection misses a file, check `:echo &filetype`. You can extend the
patterns with your own `vim.filetype.add({ pattern = { … } })`.

## Notes

- Syntax highlighting comes from the server's **semantic tokens** (Neovim 0.10+
  applies them automatically). There is no separate Tree-sitter grammar yet, so
  highlighting appears once the server attaches.
- The reference-count **CodeLens** renders via `vim.lsp.codelens` — run
  `:lua vim.lsp.codelens.refresh()` (or set up an autocmd) if you don't see it.
- Inlay **scope hints**: `:lua vim.lsp.inlay_hint.enable(true)`.
