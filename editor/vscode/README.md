# pdxl — VS Code extension

Live diagnostics for Paradox script (CK3, …). A thin
[language client](https://github.com/microsoft/vscode-languageserver-node) that
launches the `pdxl lsp` server (a Go binary) over stdio and shows unresolved
cross-file references (undefined traits, events, on_actions) as you edit.

## Requirements

- The `pdxl` binary on your `PATH` (or set `pdxl.serverPath`). Build it from the
  repo root with `make install`.

## Settings

| Setting | Description |
|---------|-------------|
| `pdxl.serverPath` | Path to the `pdxl` executable. Default: `pdxl` (on PATH). |
| `pdxl.gamePath` | Vanilla game directory (e.g. `.../Crusader Kings III/game`), so references resolve against base-game definitions. |

Open your **mod folder** as the workspace; the vanilla game is overlaid
underneath using Paradox load-order semantics.

## How it attaches

PDXScript lives in generic `.txt` files, so the client attaches by directory
(`**/common/**/*.txt`, `**/events/**/*.txt`, …) rather than claiming the `.txt`
language globally.

## Develop

```sh
npm install
npm run compile        # or: npm run watch
# Press F5 in VS Code to launch an Extension Development Host.
npm run vsix           # package a .vsix (@vscode/vsce)
```
