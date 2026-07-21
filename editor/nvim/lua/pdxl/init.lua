--- pdxl.nvim — Neovim integration for the pdxl language server.
---
--- Neovim ships a built-in LSP client, so this is just filetype detection plus
--- a `vim.lsp.config`/`vim.lsp.enable` wiring of `pdxl lsp`. Requires Neovim
--- 0.11+ (native `vim.lsp.config`).
---
--- Usage:
---   require('pdxl').setup({ game_path = '/…/Crusader Kings III/game' })

local M = {}

--- Path-scoped filetype detection.
---
--- PDXScript lives in generic `.txt` files, so it is claimed *only* inside a
--- CK3 mod/game tree (the same directories pdxl's FileSet scans) rather than
--- hijacking every `.txt`. `.gui` and localization `.yml` route to their own
--- filetypes. `vim.filetype.add` is additive and idempotent, so calling this
--- more than once is harmless.
function M.register_filetypes()
  vim.filetype.add({
    extension = {
      gui = 'pdxgui',
    },
    pattern = {
      ['.*/[Cc]ommon/.*%.txt'] = 'pdxscript',
      ['.*/events/.*%.txt'] = 'pdxscript',
      ['.*/history/.*%.txt'] = 'pdxscript',
      ['.*/gfx/.*%.txt'] = 'pdxscript',
      ['.*/localization/.*%.yml'] = 'pdxloc',
    },
  })
end

local defaults = {
  --- The `vim.lsp.config` name (also the client name in `:LspInfo`).
  server_name = 'pdxl',
  --- Full launch command. When nil, defaults to
  --- `{ 'pdxl', 'lsp', '--log-level', <log_level> }`. Override to point at a
  --- specific binary (e.g. `{ '/path/to/pdxl', 'lsp' }`).
  cmd = nil,
  --- `--log-level` passed to the server when `cmd` is not overridden.
  log_level = 'info',
  --- Vanilla game directory, forwarded as `initializationOptions.gamePath` so
  --- references resolve against base-game definitions. `~` is expanded.
  game_path = nil,
  --- Filetypes the server attaches to (script, interface, localization).
  filetypes = { 'pdxscript', 'pdxgui', 'pdxloc' },
  --- Markers identifying a mod root (becomes the server's mod directory).
  root_markers = { 'descriptor.mod', '.metadata', '.git' },
  --- Call `vim.lsp.enable` so the server autostarts on matching filetypes.
  auto_enable = true,
}

--- Registers filetypes and configures + enables the pdxl language server.
--- @param opts table|nil see `defaults` above
function M.setup(opts)
  opts = vim.tbl_deep_extend('force', defaults, opts or {})

  M.register_filetypes()

  if vim.fn.has('nvim-0.11') == 0 then
    vim.notify(
      'pdxl.nvim requires Neovim 0.11+ (vim.lsp.config / vim.lsp.enable)',
      vim.log.levels.ERROR
    )
    return
  end

  local cmd = opts.cmd or { 'pdxl', 'lsp', '--log-level', opts.log_level }

  local init_options = nil
  if opts.game_path and opts.game_path ~= '' then
    init_options = { gamePath = vim.fn.expand(opts.game_path) }
  end

  vim.lsp.config(opts.server_name, {
    cmd = cmd,
    filetypes = opts.filetypes,
    root_markers = opts.root_markers,
    init_options = init_options,
  })

  if opts.auto_enable then
    vim.lsp.enable(opts.server_name)
  end
end

return M
