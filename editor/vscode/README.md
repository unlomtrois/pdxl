# pdxl — Paradox script language support

A language server for **Paradox Interactive scripting** — the `.txt` script,
`.yml` localization, and `.gui` interface files that mods are written in.

pdxl reads your mod *and the base game together*, applying Paradox load-order
and `replace_path` rules, then indexes every definition in the project. That
index is what makes the rest work: a trait, event, decision, on-action, casus
belli or message becomes a symbol you can jump to, rename, and count references
for — across files, and across the mod/vanilla boundary.

Supports **Crusader Kings III** and **Europa Universalis V**.

<!-- screenshot: diagnostics + hover on a trait reference -->

## Features

### Catches broken references as you type

The core of it. Every reference is checked against the whole project, so a typo
in `add_trait = brve`, or a deleted event still fired by an on-action, is
flagged immediately — including references that break only because your mod
overrode a vanilla file.

<!-- screenshot: an unresolved reference diagnostic -->

### Navigation

- **Go to definition** on any reference, following mod-overrides-vanilla order
- **Find all references** across script, localization and interface files
- **Rename** a symbol and every use of it
- **CodeLens** reference counts above each definition
- **Document links** — smart-doc references are clickable
- **Workspace symbols** (`Ctrl+T`) and the document outline

### Hover

Documentation distilled from the game's own dumps: what a field means, its
default, the values it accepts, the scope its block runs in. Hovering an entity
also links to its localization, and hovering a localization key shows the text.

Definitions the *engine* uses directly — `on_death`, `msg_siege_won` — say so,
rather than leaving you to wonder why nothing references them.

<!-- screenshot: hover on a schema field showing docs + accepted values -->

### Completion

Schema-driven, so suggestions match the block you are actually inside: an
`is_shown` trigger offers triggers, an `on_complete` effect offers effects.
Localization completion in `.yml`, corpus-mined completion in `.gui`, and
completion inside smart-doc `![…]` references.

<!-- screenshot: completion inside a definition body -->

### Highlighting and inlay hints

Semantic highlighting that reflects meaning rather than spelling — a resolved
reference is coloured, an unresolved one fades. Inlay hints show the scope a
block runs in, and localized text beside the key it belongs to. Colour swatches
render inline for every colour literal, including map palettes.

<!-- screenshot: semantic highlighting + scope inlay hints -->

### Smart documentation

pdxl reads `#!` comments as documentation rather than noise:

```
#! @todo:rebalance_piety Piety gain is far too fast in the early game.
#! Grants ![trait:brave] — see also ![my_scripted_effect].
```

`![…]` links resolve to the symbol they name and are clickable. `#! @key`
declares an **anchor**: a name of your own with no script definition — a
subsystem, a design decision, a TODO — which then behaves like any other symbol,
with go-to-definition, find-references and `Ctrl+T` search.

<!-- screenshot: smart-doc anchor with references -->

### Formatting

A comment-preserving formatter for PDXScript, plus a **Format Fields** command
that reorders a definition's fields to match the schema.

## Getting started

1. Install the extension.
2. **Open your mod folder** as the workspace.
3. Point pdxl at the base game, so references resolve against vanilla:
   - `pdxl.gamePathCk3` — e.g. `…/steamapps/common/Crusader Kings III/game`
   - `pdxl.gamePathEu5` — e.g. `…/steamapps/common/Europa Universalis V/game`

On first run pdxl looks for its language server and, finding none, offers to
download the matching build from the project's GitHub releases — accept and it
installs itself; there is nothing to build. Use **pdxl: Install/Update Language
Server** to refresh it later, or set `pdxl.serverPath` to run your own build.

## Settings

| Setting | Description |
|---|---|
| `pdxl.game` | Which game the workspace targets: `auto` (default), `ck3`, `eu5`. `auto` detects the EU5-era mod layout and falls back to CK3. |
| `pdxl.gamePathCk3` | CK3 base-game directory. Empty falls back to the default Steam locations. |
| `pdxl.gamePathEu5` | EU5 base-game directory. |
| `pdxl.serverPath` | Run a specific `pdxl` binary instead of the managed one. |
| `pdxl.logLevel` | `debug`, `info` (default), `warn`, `error`. |
| `pdxl.gamePath` | Legacy CK3-only path, superseded by `pdxl.gamePathCk3`. |

## Commands

| Command | |
|---|---|
| **pdxl: Show Server Log** | Open the server's output channel |
| **pdxl: Install/Update Language Server** | Fetch the matching server release |
| **pdxl: Pick Server Binary Path** | Choose a binary interactively |
| **pdxl: Format Fields** | Reorder a definition's fields to schema order |

## How it attaches

PDXScript lives in ordinary `.txt` files, so the extension cannot claim the
`.txt` language globally without fighting every other editor feature. It
attaches to `**/*.txt` in the workspace, plus `**/localization/**/*.yml` and
`**/*.gui` — the narrow `.yml` pattern keeps CI configs and other unrelated YAML
out.

Analysis is whole-project: the first index of a large mod plus base game takes a
few seconds, after which edits are incremental.

## Contributing

Source, issues and the server itself live at
[github.com/unlomtrois/pdxl](https://github.com/unlomtrois/pdxl).

```sh
npm install
npm run compile        # or: npm run watch
# F5 in VS Code launches an Extension Development Host.
npm run vsix           # package a .vsix
```

MIT licensed.
