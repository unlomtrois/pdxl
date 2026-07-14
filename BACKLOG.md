# Backlog — LSP & analysis ideas

Unordered but roughly by value/effort. Each item should become a TODO.md-style
handoff (or a design doc) before implementation. Context: three-layer scope
model in `rust/docs/STRUCTURAL-CONTEXTS.md`; schema rows in
`rust/docs/SCHEMA-SCALING.md`; generated doc tables in
`pdxl-ck3/src/tables/`.

## Completion

- **Value completion** — see `TODO.md` (active handoff).
- **Scope-aware filtering/ranking (layer 3).** Track the current scope type
  through the walk (event `scope =` field, iterator element scopes from the
  doc tables' `Supported Targets`, `SCOPE_LINKS` output scopes) and filter —
  or at least rank — effect/trigger items whose `Supported Scopes` don't
  match. The tables already carry everything; the missing piece is a small
  scope-state fold, a lightweight cousin of tiger's ScopeContext
  (`src/context.rs` there — do NOT port the narrowing/Reason machinery).
- **Saved-scope completion.** After `scope:` offer names saved earlier in
  the file (token-scan for `save_scope_as` / `save_temporary_scope_as`
  values, in order) plus the 336 engine-saved names in
  `pdxl_ck3::tables::CODE_SAVED_SCOPES`.
- **Scope-chain completion.** After `root.` / `scope:x.` offer chain links
  from `SCOPE_LINKS` filtered by input scope (needs layer 3's current-scope
  estimate; degrade gracefully to "all links" without it).
- **Documentation on items.** The gen-tables parsers currently DROP the
  description text from effects.log/triggers.log. Extend `DocEntry`/`DocRow`
  with `desc: &'static str`, regenerate, and fill CompletionItem
  `documentation` (and hover, below). Size check first: tables grow ~2×.
- **More snippets.** `option` variants (with ai_chance), `triggered_desc`,
  `portrait` block, `on_action` skeleton in `common/on_action/`, decision
  skeleton in `common/decisions/`.

## Hover / navigation

- **Hover on built-in effects/triggers/scope links.** Currently hover only
  covers project symbols. In Effect/Trigger context (via `context_of_chain`),
  look the word up in EFFECTS/TRIGGERS/SCOPE_LINKS and show scopes + (once
  added) the description text.
- **Go-to-definition through `scope:x`.** Jump to the `save_scope_as = x`
  site in the same file/event chain.
- **Workspace symbols** (`workspace/symbol`). The SymbolTable already has
  everything; needs a name-substring index. Pairs with `SymbolTable::names`.

## Diagnostics (keep light — ck3-tiger owns deep validation)

- **Unknown effect/trigger names.** In an Effect clause, a key that is not
  in EFFECTS ∪ scripted effects ∪ control keywords is a likely typo. Same
  for triggers. This is the cheapest high-value lint the tables enable;
  gate behind config since coverage gaps (macros, DLC drift) cause false
  positives — start as "hint" severity.
- **Unknown event struct fields.** `Fallback::Deny` positions returning
  `Unknown` from `context_at` are exactly the fields `_events.info` doesn't
  document — surface as hints.
- **Effect in trigger block / trigger in effect block.** Same machinery,
  higher confidence when a name exists in the OTHER table.

## Schema growth (one KindSpec row each; ANALYSIS_VERSION bump + golden regen)

- Cultures + faiths (`culture:` / `faith:` ScopePrefix rules come free;
  religion files may need a bespoke nested extractor — see the DefShape
  escape hatch).
- Script values, scripted modifiers (kinds; their contexts already exist).
- Character interactions, casus belli, buildings, innovations.
- Phase 2 (KindId open registry) at >~20 kinds — see SCHEMA-SCALING.md.

## Editor polish

- **Semantic tokens** colored by ClauseKind (effects vs triggers vs config
  visually distinct — no other CK3 tooling does this).
- **Folding ranges** from Block nodes (needs real block spans — see below).
- **Inlay hints**: current scope type at block openers (layer 3 again).

## Infrastructure debts

- **Block/File node ranges are zero-width** (Go-parity artifact). Giving
  blocks real spans would help folding, selection range, and future
  tree-based features — but it changes SYNTAX_VERSION and every offset
  golden; batch it with another tree change.
- **gen-tables per-patch workflow**: after a game update, rerun gen-tables,
  review the diff, and re-check the `_*.info`-derived StructSpecs (they are
  hand-distilled and can drift — 162 `_*.info` files exist in vanilla).
- **LSP config**: completion result caps / disable-table-items toggle via
  initializationOptions if big item lists feel slow in practice.
