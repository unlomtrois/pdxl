//! Zed extension for pdxl — launches the `pdxl lsp` server for PDXScript
//! (`.txt`) files. The server contract mirrors the VS Code extension
//! (`editor/vscode`): run `pdxl lsp --log-level <level>` over stdio, take the
//! game directory from `initializationOptions.gamePath`, and the mod directory
//! from the workspace root (which Zed provides as `root_uri` automatically).
//!
//! User configuration lives in Zed settings under `lsp.pdxl`:
//! - `binary.path` / `binary.arguments` — override the executable and its args
//!   (defaults: `pdxl` on PATH, `lsp --log-level info`).
//! - `initialization_options.gamePath` — the vanilla CK3 `game/` directory.

use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

struct PdxlExtension;

impl zed::Extension for PdxlExtension {
    fn new() -> Self {
        PdxlExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree("pdxl", worktree)
            .ok()
            .and_then(|settings| settings.binary);

        // Explicit path from settings, else `pdxl` on PATH.
        let command = binary
            .as_ref()
            .and_then(|b| b.path.clone())
            .or_else(|| worktree.which("pdxl"))
            .ok_or_else(|| {
                "pdxl language server not found. Install it and put `pdxl` on \
                 your PATH, or set `lsp.pdxl.binary.path` in your Zed settings."
                    .to_string()
            })?;

        // Setting `binary.path` alone makes Zed pass `arguments` as an empty
        // list (not absent), so fall back to the default unless the user
        // supplied non-empty args. Custom args must include the `lsp`
        // subcommand themselves.
        let args = binary
            .and_then(|b| b.arguments)
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| vec!["lsp".into(), "--log-level".into(), "info".into()]);

        Ok(zed::Command {
            command,
            args,
            env: worktree.shell_env(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        // Forwarded verbatim to the server as `initializationOptions`; the
        // server reads `.gamePath` from here.
        Ok(LspSettings::for_worktree("pdxl", worktree)
            .ok()
            .and_then(|settings| settings.initialization_options))
    }
}

zed::register_extension!(PdxlExtension);
