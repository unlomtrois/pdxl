# Backlog — LSP & analysis ideas

Unordered but roughly by value/effort. Context: structural contexts in
`docs/STRUCTURAL-CONTEXTS.md`; schema rows in `docs/SCHEMA-SCALING.md`;
generated doc tables in `crates/pdxl-ck3/src/tables/`; the schema-coverage
worklist comes from `cargo run -p pdxl-ck3 --bin schema-gaps`.

## Schema growth (use schema-gaps; corpus-validate every rule)

- Top uncovered targets by score: `opinion_modifiers` (1.5k defs, heavily
  cross-referenced via `add_opinion = { modifier = X }`),
  `modifier_definition_formats`, `domiciles/buildings`, `flavorization`,
  `messages`, `men_at_arms_types`, `character_memory_types`,
  `important_actions`, `accolade_types`, `lifestyle_perks`.
- Kinds that would unlock already-known references when added: **situation
  participant groups** (`GetTopParticipantGroupByKey`, 46 gui refs), **trait
  tracks** (`GetTraitTrackByKey`), **religions** (`GetReligionByKey`),
  **holdings** (`county_holding_modifier.holding`), **terrains**
  (`province_terrain_modifier.terrain`), **dynasty legacy perks**
  (`county_holder_dynasty_perk`), **epidemics** (death reason `epidemic`),
  **great project types** (building `great_project_type`).

## References / analysis

- **Loc-layer `Custom('X')` refs.** `Custom`/`Custom2` arguments in
  localization `.yml` text (17k refs measured, ~22 dangling) — needs the loc
  parser to scan datafunction arguments the way the gui layer now does.
- **`localization_key` multi-language resolution.** Custom-loc entries
  reference keys defined only in non-English localization; either load key
  *names* from all languages (cheap: keys only, not text) or keep the field
  doc-only as today.
- **Datafn arity checking.** The DumpDataTypes registry has argument counts;
  hover shows them but mismatches are not diagnosed (vararg-style functions
  need a survey first).
- **Datacontext-aware narrowing for gui.** `[Character.X]` currently resolves
  against the type name alone; tracking enclosing `datacontext =` values
  would catch wrong-context usage and improve completion ranking.

## Completion

- **Scope-aware filtering (layer 3).** Completion already passes the current
  scope where known (`scope_at`); extend the scope-state fold through
  iterator element scopes and `scope =` fields so effect/trigger items are
  filtered — not just ranked — by `Supported Scopes`.
- **Saved-scope completion.** After `scope:` offer names saved earlier in the
  file (`save_scope_as` scan) plus `CODE_SAVED_SCOPES` (336 names).
- **More snippets.** `option` with ai_chance, `triggered_desc`, `portrait`,
  on_action skeleton, decision skeleton.

## Navigation / editor

- **Go-to-definition through `scope:x`** → the `save_scope_as = x` site.
- **`blockoverride "name"` → `block "name"`** in gui, resolved through the
  instantiated template/type (needs instance context; deferred from gui M1).
- **Folding ranges** from Block nodes — blocked on zero-width block ranges
  (below).
- **Zed extension** — plan exists (`editor/zed/` layout, tree-sitter-paradox
  grammar, LspSettings passthrough); CodeLens won't render there, `.txt`
  claimed globally.

## Diagnostics (keep light — ck3-tiger owns deep validation)

- **Unknown effect/trigger names.** In an Effect clause, a key not in
  EFFECTS ∪ scripted effects ∪ control keywords is a likely typo. Start as
  hint severity behind config (macros and DLC drift cause false positives).
- **Effect in trigger block / vice versa.** Same machinery, higher confidence
  when the name exists in the other table.

## Infrastructure debts

- **Block/File node ranges are zero-width** (parity-era artifact, now frozen
  by goldens). Real block spans would enable folding/selection-range; batch
  with another `SYNTAX_VERSION` change and regenerate goldens deliberately.
- **`KindId` as interned `u16`** instead of `&'static str` (~6MB savings
  measured at Phase 2; API-stable since KindId is opaque).
- **gen-tables per-patch workflow**: after a game update rerun gen-tables
  (incl. `DumpDataTypes`), review the diff, and re-check hand-distilled
  `_*.info` StructSpecs for drift.
- **`cargo clean` cadence**: `target/` grows several GB per full-workspace
  build cycle (hit 27GB once); consider cargo-sweep.
