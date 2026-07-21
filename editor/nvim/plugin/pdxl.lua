-- Register pdxl's filetype detection as soon as the plugin loads, so
-- PDXScript / `.gui` / localization files get the right filetype even before
-- `require('pdxl').setup()` runs. Starting the language server itself needs
-- setup() (for game_path etc.); this only handles detection.
if vim.g.loaded_pdxl then
  return
end
vim.g.loaded_pdxl = true

require('pdxl').register_filetypes()
