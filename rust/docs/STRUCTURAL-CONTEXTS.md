# Structural contexts — what clause is legal *here*?

Status: design draft (from the events deep-dive, 2026-07). Companion to
`SCHEMA-SCALING.md` (symbol kinds) and the generated tables in
`pdxl-ck3/src/tables/` (effects / triggers / scope links / scope types).

## The three-layer model

Scope awareness decomposes into three questions, answered by three layers:

1. **Structural context** (this doc): given a position in the tree, what
   *kind of clause* is expected — effect, trigger, script value, dynamic
   description, or a structural block with its own known fields?
   Knowledge source: the game's `_*.info` docs + tiger's bespoke validators.
   This is per *definition shape* (events, decisions, on_actions, …).
2. **Clause content** (generated tables): inside an effect/trigger clause,
   which names are legal? `EFFECTS` / `TRIGGERS` tables + control keywords +
   scripted effects/triggers from the symbol table.
3. **Dynamic scope** (future): which *scope type* is current (`character`,
   `landed_title`, …), constraining table rows and `scope:x` chains.
   Knowledge source: `SCOPE_LINKS` / `SCOPE_TYPES` tables + saved-scope scan.

Layer 1 is prerequisite to wiring layers 2–3 into completion: without it we
cannot even say that `immediate = { <effects> }` but `trigger = { <triggers> }`.

## Clause kinds observed (events corpus, `_events.info`)

- **Effect** — `immediate`, `after`, `on_trigger_fail`, option bodies,
  widget `setup_scope`.
- **Trigger** — `trigger`, `major_trigger`, option `trigger` /
  `show_as_unavailable`, portrait `trigger`, `triggered_animation.trigger`,
  `triggered_outfit.trigger`, widget `is_shown`, every `override_*.trigger`,
  `triggered_desc.trigger`, option-name `trigger`.
- **ScriptValue** — `ai_will_select` (`base`, `if/else_if { limit = <trigger> }`,
  arithmetic ops). Same shape as `common/script_values/`.
- **ScriptedModifier** — `ai_chance` (`base`, `modifier = { add/factor +
  <trigger content> }`). Same shape as `common/scripted_modifiers/`.
- **DynamicDesc** — `title`, `desc`, `opening`, option `name`: a mini-language
  (`desc`, `triggered_desc`, `first_valid`, `random_valid`, `switch`) with
  embedded Trigger clauses.
- **Structural** — blocks whose fields are enumerable: portraits, `artifact`,
  `court_scene`, `cooldown`, `widgets`, `override_*`. Fields are config
  scalars, nested structural blocks, or one of the clause kinds above.

## Key findings (why this is not a trivial key→context map)

1. **The `option` block is a *mixed* context.** Its known structural fields
   (`name`, `trigger`, `ai_chance`, `skill`, …) coexist with **inline
   effects for every unknown key** — the doc literally writes "`X..` effects
   run when this option is picked (inline, no label)". So a context needs a
   *fallback rule* for unknown keys, not just a field map. tiger models this
   the same way: known fields validated, remainder handed to the effect
   walker.
2. **Effect/trigger duality is context-local, not key-global.** `trigger` at
   event top level is a Trigger clause; `first_valid` in a DynamicDesc is a
   description selector; `first_valid` in an on_action is an event list. The
   same key means different things in different contexts — a global key→kind
   map (what our RefRules do) cannot express this; context must thread
   through the walk.
3. **Value forms fork clause kind.** Portraits accept `left_portrait = X`
   (scalar target) *or* `left_portrait = { character = X trigger = { … } }`
   (structural block). `desc` accepts a loc key *or* a DynamicDesc block.
   The context spec must dispatch on the value's node kind.
4. **Scope changes ride on structure** (feeds layer 3 later): a portrait's
   `trigger` evaluates in the *portrait character's* scope; widget
   `is_shown` runs in "event scope after immediate". Structural specs are
   where such scope annotations will attach.
5. **Events are the richest case but not special.** Every def-bearing
   directory has a root structural context: `common/scripted_effects/` bodies
   are Effect, `common/scripted_triggers/` bodies are Trigger, decisions
   have their own field map (`is_shown`/`is_valid` → Trigger, `effect` →
   Effect), on_actions again different. The structural context is a natural
   *extension of `KindSpec`* — the kind that owns a directory also owns the
   structural spec for its definitions' bodies.

## Proposed model (engine in `pdxl-analysis` or a new `pdxl-context`,
## data in `pdxl-ck3` — same split as everything else)

```rust
/// What kind of clause a block position expects.
enum ClauseKind {
    Effect,
    Trigger,
    ScriptValue,
    ScriptedModifier,
    DynamicDesc,
    Struct(&'static StructSpec),   // enumerable fields
    Config,                        // scalar setting; no clause inside
}

/// An enumerable structural block (event root, option, portrait, …).
struct StructSpec {
    name: &'static str,
    fields: &'static [(&'static str, FieldSpec)],
    /// What an unknown key means here (the option-block finding):
    fallback: Fallback,            // Effect | Trigger | Deny | Ignore
}

/// Dispatch on the value's node kind (the portrait finding).
struct FieldSpec {
    scalar: Option<ScalarKind>,    // loc key, event target, bool, …
    block: Option<ClauseKind>,
}
```

Entry: directory (`RelPath` prefix, from the owning `KindSpec`) → root
`StructSpec` for definition bodies. The walker descends the AST carrying the
current `ClauseKind`; Effect/Trigger contexts consult the generated tables +
control keywords (`if/else_if/else/while/random_list` + `limit`/`filter`
flipping to Trigger — tiger's `validate_effect_internal` rules); Struct
contexts consult their field maps.

Escape hatch as ever: a structure needing *logic* (DynamicDesc's `switch`
cases are dynamic keys) gets a bespoke walker arm, not a stretched DSL.

## Scope of the first implementation

- Event root spec (complete, from `_events.info`), option, portrait,
  widgets, override blocks, DynamicDesc; root specs for scripted_effects,
  scripted_triggers, on_action, decisions.
- Deliverable: `context_at(tree, node_id, rel_path) -> ClauseKind` — the
  query completion needs ("cursor is in an Effect clause inside option").
- Explicitly deferred: scope-type tracking (layer 3), argument-shape
  validation, ai lists beyond ScriptValue/ScriptedModifier shells.

## Sources

- `_events.info` (game docs; T4N copy at `events/_events.info`) — event
  structure incl. option inline-effects rule, portrait dual forms,
  DynamicDesc grammar.
- tiger `src/ck3/events.rs` `validate_event` — field→walker mapping;
  `src/effect.rs` `validate_effect_internal` — the `limit` duality rules.
- Other `_*.info` files exist per directory (`common/*/_*.info`) — the same
  distillation applies when a directory's structural spec is added.
