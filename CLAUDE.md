# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

pdxl is a Rust toolkit and language server for Paradox Interactive scripting
(PDXScript — CK3, Vic3, EU5 share the grammar; semantics differ per game) and
the Jomini interface dialect (`.gui`). See `README.md` for the workspace
layout and `docs/` for design/measurement history (BASELINE.md,
MILESTONE-*.md, SCHEMA-SCALING.md, STRUCTURAL-CONTEXTS.md).

## Commands

```sh
cargo test --workspace                                   # all tests incl. goldens
cargo clippy --workspace --all-targets --all-features -- -D warnings   # must be clean
cargo fmt --all --check
cargo build --release -p pdxl-cli                        # → target/release/pdxl
```

Golden regression suites (regenerate deliberately, review the diff like code):

```sh
UPDATE_GOLDENS=1 cargo test -p pdxl-parity --test lexer --test parser \
    --test fileset --test facts --test project
UPDATE_GOLDENS=1 cargo test -p pdxl-cli --test cli
```

Regenerate the game-doc tables after a game patch (needs the game's dumped
logs, incl. `data_types/` from the `DumpDataTypes` console command):

```sh
cargo run -p pdxl-gamedocs --bin gen-tables -- \
  --logs "<paradox user dir>/Crusader Kings III/logs" --out crates/pdxl-ck3/src/tables
```

The schema-coverage worklist ("what to model next"):

```sh
cargo run --release -p pdxl-ck3 --bin schema-gaps -- --game "<game dir>" [--all]
```

## Architecture (short map)

```
crates/pdxl-lexer      tokenizer: Token{start,end,kind}, byte offsets, no copies
crates/pdxl-parser     recursive descent → flat node-pool tree; parse() for
                       script, parse_gui() for the .gui dialect ([Datafn] values)
crates/pdxl-ast        the tree: Nodes + child-index array; NodeKind
                       {File,Field,Block,TaggedBlock,Scalar}
crates/pdxl-analysis   extraction engine: Schema (KindSpec rows) → FileFacts
                       {defs,refs,calls}; structural contexts (ClauseKind /
                       StructSpec / FieldSpec) for completion+hover
crates/pdxl-ck3        the CK3 schema: kinds.rs (KindId consts), entities/*
                       (one file per game concept), tables/* (generated from
                       game doc dumps), contexts.rs, coverage.rs
crates/pdxl-gui        .gui analysis: template/type symbols, DumpDataTypes
                       datafunction typing, corpus-mined completion vocab,
                       curated property docs
crates/pdxl-project    whole-project: gather → merge_and_resolve → SymbolTable
                       + RefDiags; incremental single-file updates
crates/pdxl-lsp        the language server over pdxl-project
```

### Key invariants & gotchas

- Bump `pdxl_analysis::ANALYSIS_VERSION` whenever schema/extraction semantics
  change; `pdxl_ast::SYNTAX_VERSION` for lexer/parser/tree changes (cache
  keys). Bump the workspace crate version (`[workspace.package] version` in
  `Cargo.toml`; all crates inherit) on each new feature; keep it as
  `0.<ANALYSIS_VERSION>.0`.
- Schema growth: one `KindSpec` row per game concept in `pdxl-ck3` (defs dir +
  `RefPattern` rules + gates + icon); **corpus-validate every candidate rule
  before shipping it** (target ~0 unresolved; document accepted noise and
  deliberate omissions in the entity's module doc). Shapes needing *logic* get
  bespoke extractors, not new pattern variants — see `docs/SCHEMA-SCALING.md`.
- Reference rules live in the file of their **target** kind (the loc.rs
  precedent), not where they fire.
- Multi-kind references: `RefRule::alt` — diagnosed only when no kind in the
  chain defines the name; navigation follows whichever resolves.
- `FieldSpec::values(…)` are completion/hover **suggestions, never
  validation** (mods extend vocabularies).
- Interface scripts (`.gui`): routed by extension like `.yml` loc files;
  FileSet opt-in via `set_include_gui(true)`; gui refs are name-gated into
  `FileFacts.calls` (never diagnosed — engine builtins aren't enumerable);
  datafunction typing stops at `[unregistered]` return types.
- Golden tests pin behavior that was originally byte-verified against the
  retired Go implementation; treat golden diffs as behavior changes to review,
  never as noise to regenerate blindly.
- `Stats.shadowed` is always 0 (preserved Go-era quirk, documented in the M3
  report).
- lsp-server crate: use `initialize_start`/`initialize_finish`, NOT
  `Connection::initialize` with a pre-wrapped result (double-nests
  capabilities; regression-tested in `pdxl-cli/tests/cli.rs`).
- Edition 2024: let-chains are used; clippy runs with `-D warnings`.
- Shell here is zsh: unquoted `$VARS` do NOT word-split — pass args
  explicitly. Beware zsh MULTIOS: `2>&1 >/dev/null | …` tees stdout into the
  pipe; use separate redirections.

### Corpus paths (this machine)

- Game: `~/.local/share/Steam/steamapps/common/Crusader Kings III/game`
- Mod:  `~/.local/share/Paradox Interactive/Crusader Kings III/mod/T4N.mod`
  (dir root `…/T4N-CK3/T4N`)
- Game doc dumps: `~/.local/share/Paradox Interactive/Crusader Kings III/logs`

Full-corpus sanity check after schema work:

```sh
./target/release/pdxl check --no-cache --game "<game>" --mod "<mod .mod>"
```

### Commit style

Use git-flow prefixes: `feat(scope):`, `fix(scope):`, `refactor(scope):`,
`chore(scope):`, etc.
