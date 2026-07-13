# Milestone Report — Rust Port: LSP (M8a)

## Status
Complete (M8a scope: handshake, full-text sync, debounced mod-scoped
diagnostics, go-to-definition). M8b (references, documentSymbol, hover) next.

## Branch
`rust-port-milestone-8a` (based on `39346ca`).

## Reference Go commit
`5d02a979cb162ba5d89c7e705618de322884bd79` (`internal/lsp`, ~1050 lines).

## Framework decision (documented before implementation, per the M8 spec)
**`lsp-server` + `lsp-types 0.95`** (the rust-analyzer/Ruff stack) over
`tower-lsp-server`/`async-lsp`. Rationale: every handler is a sub-millisecond
in-memory lookup — async buys nothing here; the one slow operation (initial
build) runs on a plain background thread. Concurrency model: **a single event
loop thread owns all state**, selecting (crossbeam) over the client channel
and an internal `Event` channel (`ProjectReady`, `Debounce`). Cancellation:
not needed at this feature set (no long-running requests). Document sync:
full-text (Go parity). Position encoding: UTF-16 at the boundary only.

## Architecture vs Go
- Go: mutex-guarded `Server` + handler goroutines, with the documented
  non-reentrant-lock hazard (`readFile` vs `readFileLocked`). Rust: the event
  loop owns `ServerState` outright — **no mutex exists**; background work
  posts events instead of touching state, so the Go deadlock class is
  unrepresentable.
- Debounce: per-document generation counters; a sleeper thread posts
  `Debounce{path, generation}` after 200 ms and stale generations are ignored
  on arrival (replaces Go's `time.AfterFunc` timer map).
- Behavior ported exactly: async build in `initialized` with instant
  `initialize`; open-buffers-override-disk after the build; edit → debounce →
  `update_source` → republish for every mod file (open or not); explicit
  clearing publishes; `didClose` reverts to disk state; vanilla analyzed but
  never flagged; every definition no-result branch returns null.
- `position.rs` ports Go's `offsetToPosition`/`positionToOffset` including
  clamping semantics and surrogate-pair counting; unit tests cover multibyte
  round-trips (`é`, `😀`) and the clamp cases.
- CLI: `pdxl lsp [--game DIR]` (mod root arrives as the workspace root URI;
  `initializationOptions.gamePath` override supported; hidden `--stdio` for
  client compatibility).

## Deviations from Go (documented)
- No AST/facts caches in the build — the measured decision (BASELINE.md);
  cold build is a once-per-session cost.
- `referencesProvider` not yet declared (Go has it; M8b).
- No `pdxl.toml` loading yet (built-in defaults equal `config.Default()`).
- Diagnostics publish in deterministic (sorted) file order; Go iterated a map.

## Verification
- **Unit/behavior tests (11)**: the Go `server_test.go` captured-notify
  pattern — initial mod-scoped publish; buffer edit fixes a ref and produces a
  clearing publish; stale debounce generations ignored; `didClose` reverts to
  disk; vanilla never flagged; cross-file definition with exact UTF-16 ranges;
  all no-result branches; docs opened before the build analyzed after it.
- **Live protocol smoke** (scripted stdio client): full lifecycle —
  capabilities, initial diagnostics, fix → clear, definition jump, clean
  shutdown/exit (a real channel-lifetime bug in shutdown was found and fixed
  this way: `ServerState`'s sender clone kept the writer thread alive).
- **Real corpus** (CK3 vanilla + T4N): `initialize` answers in ~1 ms; first
  diagnostics 5.8 s after `initialized` (async build), pinpointing the known
  `ep3_akolouthos_on_action` at line 2393; definition responds in ~2 ms;
  clean exit.
- Gates: 48 workspace suites green; all six differential suites unaffected;
  `go test ./...` green; fmt + clippy `-D warnings` clean; no `unsafe`.
- New dependencies: `lsp-server`, `lsp-types` (0.95, `Url`-based),
  `serde_json`, `crossbeam-channel`.

## Files changed
- Added: `rust/crates/pdxl-lsp/**` (`src/{lib,state,position}.rs`,
  `tests/server.rs`), `rust/docs/MILESTONE-8A-REPORT.md`.
- Modified: `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/README.md`,
  `pdxl-cli/{Cargo.toml, src/main.rs}` (+`lsp` subcommand).
- Unchanged: all Go code.

## M8b (completed on the follow-up branch)
- **references** (Go parity): defs-first `symbol_at` (cursor on a `NAME = {}`
  name finds that symbol's references), all reference sites in walk order via
  `Project::references`, declaration appended last under `includeDeclaration`,
  per-file source cache (Go's `refsToLocations`). Empty result → null.
- **documentSymbol** (exceeds Go): the file's `FileFacts.defs` as a flat
  outline (name span as both range and selection until facts record block
  extents), with a presentation mapping to LSP symbol kinds.
- **hover** (exceeds Go): markdown with kind + name, defining rel_path,
  `$PARAM$` list; unresolved symbols marked `*(unresolved)*`; highlight range
  = the def/ref span under the cursor.
- 7 new behavior tests (18 total in the crate), incl. a test that documents
  the defs-first subtlety: in `common/scripted_effects/`, the top-level key
  at character 0 IS a definition, so "cursor on nothing" is the `=` sign.
- Live smoke on real CK3+T4N: 18 outline symbols in ~13 ms, hover ~1.4 ms,
  references ~2.2 ms, clean exit.

## Remaining LSP work (future)
completion and rename (the two "hard" features), workspace/symbol (needs
explicit sorting), `pdxl.toml` loading, and editor field-testing via VS Code
against the real T4N workspace.

## Field test (VS Code + real T4N workspace) — PASSED
Setup: the existing `editor/vscode` extension unchanged (built + installed as
a .vsix); workspace `.vscode/settings.json` pointing `pdxl.serverPath` at the
Rust release binary and `pdxl.gamePath` at the CK3 game dir. The Rust server
is a drop-in replacement for the Go one behind the same client.

Two launch-blocking bugs were found only by the real client, then fixed:
1. **Deploy-order**: the extension always passes `--log-level`, which the
   binary initially rejected (added in `5721ca2` + `3b73678` gave the flag a
   real leveled stderr logger feeding the "pdxl (server)" output channel).
2. **Double-wrapped InitializeResult** (`c800593`): `lsp-server`'s
   `initialize()` helper wraps its argument in `{"capabilities": ...}`; we
   passed a pre-wrapped result, so VS Code saw no declared sync/providers and
   — being spec-respecting — never sent a single textDocument notification.
   Hand-rolled smoke clients ignore the handshake and could not catch this;
   the wire-level regression test now asserts the exact nesting.

Confirmed live by the user: hot reload works — diagnostics appear/clear
~200 ms after edits, across the full CK3 vanilla + T4N corpus.
