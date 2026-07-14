# TODO: LSP value completion (symbol names in value positions)

Task for a fresh session. Read these FIRST, in order:

1. `CLAUDE.md` — the "Rust port (rust/)" section (commands, testing policy,
   invariants). `.claude.local.md` has machine paths (real CK3+T4N corpus,
   game doc-dump logs) and shell traps (`grep` is aliased to ugrep and fails
   on effects.log — use `command grep`; zsh does not word-split unquoted
   vars).
2. `rust/docs/STRUCTURAL-CONTEXTS.md` and `rust/docs/SCHEMA-SCALING.md` —
   the two design docs the completion stack is built on.
3. The existing completion implementation (this is an EXTENSION of it):
   - `rust/crates/pdxl-lsp/src/completion.rs` — item builders per ClauseKind
   - `rust/crates/pdxl-lsp/src/state.rs` — `ServerState::completion` +
     `enclosing_key_chain` (token-scan brace stack; read its doc comment —
     the parsed tree is NOT usable for cursor positioning: Block/File nodes
     have zero-width ranges and empty blocks contain no node)
   - `rust/crates/pdxl-analysis/src/context.rs` — `context_of_chain`
   - `rust/crates/pdxl-analysis/src/schema.rs` — `key_rules` /
     `scope_rules` (currently `pub(crate)`; you will need public read
     access — add narrow accessor methods, do NOT make the fields public)
   - `rust/crates/pdxl-lsp/tests/server.rs` — the five existing completion
     tests and their helpers (`completion_server`, `pos_of`, `labels`)

## The task

Today completion only fires in KEY positions. Implement completion in VALUE
positions:

1. **`key = <cursor>` symbol values.** When the cursor sits after an
   operator whose key has reference rules (`add_trait`, `has_trait`,
   `remove_trait`, `trigger_event`, gated `capital`, …), offer the defined
   names of the mapped SymbolKind from the project symbol table
   (`table.names(kind)`), respecting per-rule gates against the file's
   rel_path. Needs a new public accessor on `Schema`, e.g.
   `value_kinds(key, rel_path) -> impl Iterator<Item = SymbolKind>`,
   wrapping the compiled `key_rules` (KeyForm::Value rules only).
2. **`prefix:<cursor>` scope-literal values.** When the partial token under
   the cursor starts with a configured scope prefix (`title:` today — read
   prefixes from `Schema` via a new accessor over `scope_rules`, don't
   hardcode), offer names of the mapped kind (`table.names(Title)`).
3. **on_action fire lists.** Inside `events = { … }` / `first_valid` /
   `random_events` weighted values etc. (context is the `fire_list` /
   `weighted_fire_list` StructSpec from `pdxl_ck3::contexts`), offer Event
   (or OnAction, per list) names. Mapping list-struct → kind can live in
   the completion layer keyed on `spec.name`, or better: reuse the
   existing KeyList/KeyWeighted ref rules via a Schema accessor.

Detection mechanics (extend the token scan in `state.rs`, do not parse):
after building the brace stack up to the cursor, also record the last
1–2 tokens before `off` — if they are `scalar op` the cursor is in the
value of that key; if the token CONTAINING `off` starts with `prefix:`,
case 2 applies. Keep `enclosing_key_chain`'s contract intact (it excludes
the token containing `off`).

## Item shape

- label = symbol name; kind = an appropriate CompletionItemKind (e.g.
  ENUM_MEMBER for traits — consider reusing `Schema::icon` → a small
  IconHint→CompletionItemKind map, mirroring `lsp_symbol_kind`).
- detail = "<kind> · defined in <file>" (Symbol has `.file`).
- sort_text tier "0_" (symbol values are the most specific suggestion).
- For `prefix:` completions the client may treat `:` as a word boundary —
  if VS Code filtering misbehaves, use `text_edit` with a range covering
  the partial name after the colon (compute from the token range). Test in
  the real T4N workspace before deciding.

## Acceptance

- New handler-level tests in `pdxl-lsp/tests/server.rs` following the
  existing style: value after `add_trait = ` offers trait names and does
  NOT offer effects; `capital = ` offers titles only under
  `common/landed_titles/`; `title:` offers title names; an on_action
  `events = { … }` list offers event ids.
- Zero behavior change to facts/analysis: goldens untouched, NO
  `ANALYSIS_VERSION` bump (completion is a pure query; only add accessors).
- Full gates: `cargo test --workspace` (53 suites currently),
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `go test ./...` untouched/green.
- Rebuild the release binary (`cargo build --release -p pdxl-cli`) — the
  user's VS Code launches `rust/target/release/pdxl` directly.
- Commit style: `feat(rust-lsp): …` with the Co-Authored-By trailer used in
  recent history. Delete this TODO.md in the same commit.

## Known traps

- `enclosing_key_chain` clears its `recent` token ring on every brace; the
  value-position detector must look at tokens AFTER the last brace, not
  across it.
- Operator set: include `?=` (QuestionEqual) and comparison ops — `has_trait
  ?= x` is legal.
- `table.names()` iterates a HashMap — order is arbitrary; tests must
  assert membership, not order.
- The option block's fallback means `add_trait` may appear as a KEY item
  too (an effect) — value completion must replace, not append to, the
  key-position item set when the cursor is in a value.
