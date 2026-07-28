# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Crate-level guidance for `pdxl-ck3`, the CK3 schema crate. The workspace root
`CLAUDE.md` covers workspace-wide commands, invariants (FieldSpec vs RefRule,
implicit-loc patterns, intrinsics, soft refs), and corpus paths — read it
first; this file only adds what is specific to working inside this crate.

## Commands

```sh
cargo test --release -p pdxl-ck3                                  # all suites below
UPDATE_GOLDENS=1 cargo test --release -p pdxl-ck3 --test facts    # regen facts goldens
cargo test --release -p pdxl-ck3 --test extract -- <test_name>    # one unit test

# Corpus measurement for the table-derived scope-link rules (ignored by default):
PDXL_CK3_GAME="$HOME/.local/share/Steam/steamapps/common/Crusader Kings III/game" \
PDXL_CK3_MOD="$HOME/.local/share/Paradox Interactive/Crusader Kings III/mod/T4N-CK3/T4N" \
  cargo test --release -p pdxl-ck3 --test derived_proof -- --ignored --nocapture
```

`src/tables/*` is **generated** by `pdxl-gamedocs` (`gen-tables`, see root
CLAUDE.md) — never hand-edit; regenerate after a game patch and review the
diff. Everything else in `src/` is deliberately hand-written.

## Test suites

- `tests/facts.rs` — golden snapshots of fact extraction over `testdata/` plus
  the lexer stress fixtures, replayed under directory "personas" (the same
  fixture read as `common/traits/f.txt`, `events/f.txt`, …). The regression
  net for any schema change.
- `tests/extract.rs` — unit-level assertions per reference shape and skip rule.
- `tests/contexts.rs` — `context_at` classification over the StructSpec trees.
- `tests/tables.rs` — invariants of the generated tables (sorted/unique,
  plausible sizes); does not re-verify game data.
- `tests/derived_proof.rs` — env-gated corpus measurement (`PDXL_CK3_GAME` /
  `PDXL_CK3_MOD`), comparing `schema_hand_only()` vs `schema()` per kind.
  Rerun after touching `src/derived.rs`; module docs there record accepted
  misses.

## Architecture

Everything the analyzer knows about one game concept lives in **one file**
under `src/entities/` — its `KindSpec` rows (def dir, `RefPattern` rules,
icon), its `StructSpec` body tree, implicit-loc patterns, and intrinsics. The
`Entity` trait is a shape contract (associated consts only, all defaulting to
empty); the `registry!` macro in `entities/mod.rs` flattens every registered
entity's consts into the vectors `lib.rs` feeds to `Schema`.

Assembly path (`src/lib.rs`): `schema()` = hand-written entity rows +
`derived::derived_link_rules()` (scope-link table → `ScopePrefix` rules via
the curated `TARGET_KINDS` map), then `set_implicit_loc_patterns` /
`set_intrinsics` / `set_contexts`. `contexts.rs` only assembles entity `ROOTS`
into the `ContextSchema` (plus `EFFECT_STRUCTS` for documented effect blocks
like `create_character`); the specs themselves live with their entities.

Game-wide vocabulary that belongs to no single concept stays in `lib.rs`:
`SCOPE_KEYWORDS`, `TYPED_DEFS`, `KEYED_VALUE_DEFS`, `DOC_REF_ALIASES`,
`CALL_KINDS`, `GUI_KINDS`.

### Adding a game concept (the full wiring)

1. `pub const` in `src/kinds.rs`.
2. New file in `src/entities/` implementing `Entity` — module doc must record
   the source `.info` file, corpus validation numbers, and accepted
   noise/omissions (see `holy_site.rs` for the shape).
3. `mod` line **and** `registry!(…)` line in `entities/mod.rs` — the trait
   makes a missed registry line compile fine and silently contribute nothing.
4. If the kind is a scope-link target (`character:`-style), add one line to
   `TARGET_KINDS` in `src/derived.rs`; its module doc lists the kinds waiting
   for exactly this.
5. Add the kind to `DOC_REF_ALIASES` / `GUI_KINDS.arg_refs` in `lib.rs` if a
   `![alias:…]` or a ByKey datafunction exists for it.
6. Regenerate the facts goldens and reconcile the diff; corpus-validate with
   `pdxl check` (root CLAUDE.md) and, if `derived.rs` changed, `derived_proof`.

Module docs are the crate's institutional memory: they carry corpus counts,
rejected alternatives, and per-entity noise budgets. Keep them current when
changing a rule — the numbers are what make the next change reviewable.
