# TODO: Schema scaling — implement Phase 1 (KindSpec rows + unified RefRule)

Task for a fresh session. Read these FIRST, in order:

1. `CLAUDE.md` — especially the "Rust port (rust/)" section (commands, testing
   policy, invariants). `.claude.local.md` has machine paths (real CK3+T4N
   corpus for validation).
2. `rust/docs/SCHEMA-SCALING.md` — the accepted design this task implements.
   It contains the diagnosis, the target types (KindSpec / DefSource /
   RefPattern / RefRule / gates), and the two-phase migration plan.
3. Current code to be refactored: `rust/crates/pdxl-analysis/src/schema.rs`
   (the five parallel rule maps + `list_gate_prefix` — the accretion being
   removed), `extract.rs` (their consumers), and `rust/crates/pdxl-ck3/src/lib.rs`
   (the CK3 rules that become one `KindSpec` block per kind).

## Scope: Phase 1 ONLY (behavior-identical)

Implement from the design doc:

- `KindSpec { name, icon, defs: Option<DefSource>, refs, aliases }` — one row
  co-locating everything about a kind; `Schema::new(&[KindSpec])` compiles rows
  into lookup indices (one key→rules map replacing the four key-based maps, a
  scope-prefix list, a dir-prefix table).
- Unified `RefRule { pattern: RefPattern, gate: Option<&'static str> }` with
  `RefPattern::{KeyValue, KeyBlockId, KeyList, KeyWeighted, ScopePrefix}`.
  Per-rule `gate` (RelPath dir prefix) SUBSUMES `list_gate_prefix` — delete it;
  the on_action list/weighted rules get `gate: Some("common/on_action/")`.
- `IconHint` — small neutral enum in `pdxl-analysis`; move the LSP icon mapping
  (`lsp_symbol_kind` in `pdxl-lsp/src/state.rs`) to consume it via the spec.
- KEEP the `SymbolKind` enum as the ID (Phase 2 — `KindId`/open registry — is
  explicitly deferred; do NOT start it).
- Rewrite `pdxl-ck3::schema()` as 8 `KindSpec` blocks (trigger, effect, trait,
  decision, on_action, event, character, title). Trait keeps its aliases;
  title keeps `DefShape::Tree` + `ScopePrefix("title")`.

## Acceptance

- **Zero behavior change**: all goldens pass UNTOUCHED
  (`cargo test -p pdxl-parity --test facts --test project`,
  `cargo test -p pdxl-cli --test cli`) — if a golden differs, the refactor has
  a bug; do NOT regenerate to make it pass.
- Still-Go-verified layers unaffected: lexer/parser/fileset/descriptor
  differentials green; `go test ./...` green (no Go changes at all).
- Full gates: `cargo test --workspace` (currently 48 suites), `cargo fmt --all
  --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- No `ANALYSIS_VERSION` bump (semantics unchanged). Update the design doc's
  status line (Phase 1: done) and `CLAUDE.md`'s schema-growth gotcha if wording
  changes.
- Commit style: `refactor(rust-analysis): ...` (git-flow prefixes,
  Co-Authored-By trailer as in recent history).

## Optional follow-up (separate commit, only if Phase 1 lands clean)

Add the first gated scalar rule the design was built for:
`RefRule { pattern: KeyValue("capital"), gate: Some("common/landed_titles/") }`
on the title kind. This IS a behavior change: bump `ANALYSIS_VERSION` to 3,
regenerate goldens deliberately (`UPDATE_GOLDENS=1 …`), add extraction unit
tests in `pdxl-ck3/tests/extract.rs` (gated in-dir hit, out-of-dir miss), and
validate on the real corpus (`.claude.local.md` paths):
`./rust/target/release/projectdump --root <game>:vanilla --root <t4n>:mod
--replace … ` — expect `capital = c_x` refs to resolve against the title tree;
report unresolved-count delta before/after.

## Known traps (learned this session)

- zsh: unquoted `$VARS` don't word-split — write test/bench commands with
  explicit args.
- The check CLI goldens normalize temp roots (`<van>`, `<mod>`); keep that.
- `extract_refs` visits every scalar (keys, values, list items) — the
  ScopePrefix pattern must keep matching in ALL positions with the range
  covering only the name (unit tests exist: `title_ref_range_covers_only_the_name`
  et al in `pdxl-ck3/tests/extract.rs`).
- After changing anything the LSP serves, rebuild the release binary
  (`cargo build --release -p pdxl-cli`) — the user's VS Code launches
  `rust/target/release/pdxl` directly.
