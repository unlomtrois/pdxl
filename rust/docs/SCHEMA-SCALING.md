# Schema scaling design — surviving 100 SymbolKinds

Status: accepted design, phased. Phase 1 pending; Phase 2 deferred until
kind-count or multi-game pressure is real. Raised by a design review after the
landed-titles addition ("do we add a field to DefRule per special case?").

## Diagnosis

Three accretions threaten the current schema at scale:
1. `Schema` grows a **field per reference shape** (already five parallel maps
   plus a one-off gate, `list_gate_prefix`).
2. Knowledge about one kind is **scattered across collections** ("trait" =
   def_rules + 3 ref_rules entries + alias_keys).
3. `SymbolKind` is a **closed enum inside the engine** — each new kind edits
   `pdxl-analysis` and `pdxl-lsp`, leaking the engine/knowledge split.

Reference point: ck3-tiger at ~280 item types uses (a) an enum of loader
strategies with flag columns for the generic path, (b) 36 bespoke handlers for
types whose loading needs *logic*, and (c) a prefix→item table for scope
literals. It never grows optional fields on a shared rule struct.

## Design

Organizing principle: **co-locate by growth axis**.
- Grows per game concept → one row in one place (`KindSpec`).
- Grows per PDXScript syntactic shape → engine enum variant (rare).
- Grows per game → a new rules crate of rows (engine untouched).

```rust
// pdxl-analysis — game-agnostic, changes ~never
pub struct KindId(u16);                       // registration order
pub struct KindInfo { name: &'static str, icon: IconHint }

pub struct KindSpec {                         // ALL knowledge about one kind
    pub name: &'static str,
    pub icon: IconHint,                       // neutral; LSP maps to lsp-types
    pub defs: Option<DefSource>,
    pub refs: &'static [RefRule],
    pub aliases: &'static [&'static str],
}
pub struct DefSource { pub dir_prefix: &'static str, pub shape: DefShape }

pub enum RefPattern {
    KeyValue(&'static str),                   // add_trait = X
    KeyBlockId(&'static str),                 // trigger_event = { id = X }
    KeyList(&'static str),                    // events = { X Y }
    KeyWeighted(&'static str),                // random_events = { 50 = X }
    ScopePrefix(&'static str),                // title:X — any scalar position
}
pub struct RefRule { pub pattern: RefPattern, pub gate: Option<&'static str> }
```

`Schema::new(&[KindSpec])` compiles rows into lookup indices once:
- one `HashMap<key, SmallVec<(KindId, pattern, gate)>>` (replaces 4 maps),
- one scope-prefix list, one dir-prefix table,
- `Vec<KindInfo>` indexed by `KindId` (SymbolTable becomes Vec-indexed).

Per-rule `gate` subsumes `list_gate_prefix` and unlocks dir-scoped scalar
rules (e.g. `capital = c_x` only inside `common/landed_titles/`).

Escape hatch (unchanged from the DefShape decision): a shape that needs
*logic* — tiger's tier-order validation class — gets a bespoke extractor, not
a new pattern variant stretched into a DSL.

Deliberately out of scope: loading schemas from TOML/runtime config. Rows
make that trivial later, but Rust-source rows are type-checked, reviewed, and
versioned by ANALYSIS_VERSION; runtime schemas are a modder-facing feature
decision, not an architecture prerequisite.

## Migration

- **Phase 1** (behavior-identical): KindSpec + unified RefRule/RefPattern +
  per-rule gates + compiled indices. Keep the SymbolKind enum as the ID.
  Goldens unchanged.
- **Phase 2** (on real pressure: >~20 kinds or a second game): replace the
  enum with KindId/KindInfo. Touches dumps + table; golden regen +
  ANALYSIS_VERSION bump. Costs exhaustive matching on kinds — dead at that
  scale anyway; IconHint (small, closed) carries presentation.

## Trade-offs accepted

- Losing exhaustive `match SymbolKind` in Phase 2 (fallbacks via IconHint).
- Slightly more indirection in Schema construction (rows → indices) for
  much better locality of change.
