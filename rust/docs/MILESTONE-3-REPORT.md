# Milestone Report — Rust Port: FileSet

## Status
Complete.

## Branch
`rust-port-milestone-3` (based on `bc9f4b4`).

## Commits
- `2bfed5c` feat(rust): port FileSet + Paradox overlay resolution with Go parity (M3)
- (report commit follows)

## Reference Go commit
`5d02a979cb162ba5d89c7e705618de322884bd79`

## Baseline verification
Base commit `bc9f4b4d9a254cee1b0fcd8c38de54bf15b2b014`. Before changes:
`go test ./...` green; `cargo fmt --check`, `cargo clippy --workspace
--all-targets --all-features -- -D warnings`, and `cargo test --workspace` all
green.

## Implemented
- **FileSet model** — `FileKind`, `FileEntry`, `Stats`, `FileSet` (default-ready).
- **Deterministic scanner** — recursive `.txt` discovery, directory entries
  sorted by name (byte order) at each level to reproduce `filepath.WalkDir`.
- **Overlay registration** — in-place winner replacement (keeps the original
  slot); `resolve`, `iter`, `try_for_each`, `stats`.
- **Replacement paths** — `set_replace_paths`, vanilla/DLC-only dropping.
- **Ignore rules** — `set_ignore`, dot-directories, case-insensitive base names.
- **Descriptor parsing** — `parse_mod` via `pdxl-syntax`, quote trimming, ordered
  duplicate `replace_path`, relative/Windows path handling.
- **Windows helper** — `is_windows_absolute` (used by `parse_mod`).
- **Differential harness** — `tools/filesetdump` (Go) + Rust `filesetdump` bin,
  canonical scan/descriptor dumps; integration parity + invariant tests.

## Architecture
- **Crate dependencies**: `pdxl-files` → `pdxl-syntax` → `pdxl-lexer` →
  `pdxl-source`. No std-only-violating deps; no cache/analysis/CLI/LSP deps.
- **Public types**: `FileKind`, `FileEntry { rel_path: String, full_path:
  PathBuf, kind }`, `Stats { vanilla, mod_files, total, shadowed, replaced }`,
  `FileSet`, `ModDescriptor { name, path: PathBuf, replace_paths: Vec<String> }`;
  functions `parse_mod`, `is_windows_absolute`, `dump_scan`, `dump_descriptor`,
  and Go-compatible `clean_path`/`join_paths`/`normalize_key` helpers.
- **Normalized overlay-key representation**: `to_slash` (native separator → `/`,
  identity on Unix) then lowercase, matching `strings.ToLower(filepath.ToSlash)`.
  Keys are `String`; non-UTF-8 Unix names are a documented limitation (below).
- **Ordering strategy**: `entries: Vec<FileEntry>` in insertion-slot order;
  `by_path: HashMap<key, index>` records the winning slot. Winner iteration walks
  `entries` and keeps those whose `by_path` index matches — never hash order. New
  keys append; existing keys overwrite in place (stable slot).
- **Filesystem-path ownership**: `full_path` is `Join(clean(root), rel)` (Go
  `WalkDir` path), **not** canonicalized — a relative root stays relative,
  matching Go (whose `FullPath` "absolute" comment is aspirational).

## Parity
### FileSet
- **Scenarios compared (5)**: basic scan (nested, `.TXT`, non-script, empty dir);
  4-kind overlay with stable winner slots; replacement (exact/descendant/similar
  non-match, kinds, count); ignore (nested dirs/files, case-insensitive, dot
  dirs); normalization (mixed case, nested, non-ASCII Greek/accented Latin, case
  collision).
- **Exact entry-order matches**: 5/5 byte-identical dumps — entries in exact
  unsorted `Walk`/`iter` order.
- **Exact stats matches**: vanilla/mod/total/shadowed/replaced identical,
  including `shadowed = 0`.
- **Exact resolve matches**: case-insensitive queries and absent paths identical.

### Mod descriptors
- **Fixtures compared (6)**: repo `T4N.mod`; relative path; Windows forward-slash;
  Windows backslash; repeated `name`/`path` + unknown field; malformed-but-
  readable. 6/6 byte-identical dumps.
- **Duplicate preservation**: `T4N.mod`'s two `common/achievements` entries kept
  in order (not deduplicated).
- **Path-resolution matches**: relative joined to the `.mod` dir; Windows kept
  verbatim; `is_windows_absolute` agrees on all probed forms.

### Previous milestones
- lexer: 11/11 byte-identical (re-run, unaffected).
- parser: 11/11 structured-dump byte-identical (re-run, unaffected).
- golden trees: 8/8 (re-run, unaffected).

## Tests
### Go
`go test ./...` — green, unchanged. Only the additive `tools/filesetdump` package
was added; `internal/files` behavior is untouched.

### Rust
`cargo test --workspace` — green. `pdxl-files`: 22 FileSet tests + 10 descriptor
tests (ported from `files_test.go` plus the milestone gaps). `cargo fmt --check`
and `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
No `unsafe`.

### Differential
- `cargo test -p pdxl-files --test parity` — 5 scan scenarios + 6 descriptors
  byte-identical to the Go oracle (`go run ./tools/filesetdump`). Self-skips if
  `go` is absent.
- Lexer/parser regressions: `cargo test -p pdxl-lexer --test parity`,
  `cargo test -p pdxl-syntax --test parity`, `cargo test -p pdxl-syntax --test
  fixtures` — all green.

### Invariants
A test-only `validate_fileset` runs on every scan scenario and most unit tests:
`by_path` indices in range; keys map to entries with the same `rel_path`; every
`rel_path` is normalized lowercase with `/` separators and no `\`; no duplicate
winner `rel_path`; `resolve(rel)` returns that winner; `stats.total ==
iter().count()`; `vanilla + mod_files == total`; deterministic order across
repeated scans (checked separately).

## Deviations from Go
- **Proton path translation dropped.** `ResolveWindowsPath` (the Windows →
  `<prefix>/drive_c/...` mapping) is intentionally **not** ported, per project
  direction: mods are referenced by local folder path, never via a Steam/Proton
  drive path. `is_windows_absolute` is retained (it drives `parse_mod`'s
  relative-vs-absolute branch). This is the only intentional behavioral cut; the
  rest of the package is matched exactly.
- `Stats.mod_files` is named to avoid the `mod` keyword; normalized to `"mod"` in
  dumps.

## Bugs or ambiguities discovered
- **`Stats.Shadowed` is always 0 (confirmed Go behavior).** `register` overwrites
  `entries[idx]` in place, so no non-winning historical entry ever remains; the
  `Stats()` loop's `byPath[e.RelPath] != i` branch is unreachable. The shadow
  counter therefore never increments, even when a mod shadows vanilla. Verified by
  reading `register`/`Stats` and reproduced in `shadowed_is_always_zero` and the
  differential overlay scenario. Matched deliberately. **Recommended post-parity
  fix:** if a real shadow count is wanted, either keep a separate counter
  incremented on in-place overwrite, or stop reusing the slot — but that changes
  winner ordering, so it must be a deliberate, separately-tested change.
- **Non-C drive mapping** — N/A this milestone (Proton resolution dropped). If it
  is ever ported, note the Go code maps any drive letter through `drive_c`.
- **Relative vs absolute `FullPath`** — `FileEntry.full_path` is not
  canonicalized; a relative scan root yields a relative `full_path`, despite the
  Go field comment calling it absolute. Preserved as-is.
- **Platform-specific normalization** — overlay keys use `str::to_lowercase`
  (full Unicode lowercase). Go uses simple per-rune `unicode.ToLower`. They agree
  for ASCII and the accented-Latin/Greek names tested; exotic 1:many mappings
  (e.g. `İ`) could differ. Non-UTF-8 Unix filenames are decoded lossily for the
  key — also a documented limitation. Normal PDXScript paths are unaffected.

## Files changed
- Added: `rust/crates/pdxl-files/**` (`src/{lib,fileset,mod_descriptor,path,
  dump}.rs`, `src/bin/filesetdump.rs`, `tests/{common/mod,fileset,mod_descriptor,
  parity}.rs`, `Cargo.toml`), `tools/filesetdump/main.go`,
  `rust/docs/MILESTONE-3-REPORT.md`.
- Modified: `rust/Cargo.toml` (+`pdxl-files`, +`pdxl-syntax` workspace dep),
  `rust/Cargo.lock`, `rust/README.md`.
- Unchanged: all existing Go source under `cmd/`, `internal/`.

## Risks for later milestones
- **Cache keys: normalized vs full path.** The overlay key (`rel_path`) is the
  semantic identity; `full_path` is read-location only and is not canonicalized.
  Cache/fact keys (M4/M5) should key on `rel_path` + content, and must account for
  directory location where semantics depend on it (the schema does).
- **Path identity & symlinks.** Traversal does not follow directory symlinks
  specially (it uses `file_type` without resolving links); `full_path` is
  lexical, not canonical. Two roots pointing at the same files via different paths
  produce different `full_path`s but the same `rel_path` (overlay still works).
- **FileSet ordering as semantic precedence.** Winner slot order is now locked by
  differential tests; later duplicate-definition precedence (M6) depends on it.
  Do not switch to hash-ordered iteration.
- **Non-UTF-8 paths.** Keys are `String`; a non-UTF-8 Unix filename is decoded
  lossily. If such paths must round-trip, the key type would need to become
  `OsString`/bytes — a deliberate future change.
- **Adding/removing files in long-running projects.** `FileSet` is build-once;
  it has no incremental add/remove. The watch/LSP layers rebuild rather than
  mutate, consistent with the project's whole-table-rebuild model.

## Recommendation for Milestone 4 (syntax cache) — do not begin
Add a `pdxl-cache` crate depending on `pdxl-syntax` (+ `pdxl-source`). Port the
**concept** of the two-level AST cache, not the Go `gob` encoding:
- **L1** in-memory bounded LRU (only when capacity > 0), invalidated by mtime.
- **L2** persistent on-disk entries keyed by a hash of the cleaned path, each
  storing `{format_version, syntax_version, mtime, sha256, source, nodes,
  child_ids, diags}`.
Adopt the spec's improvements over the Go design: include an explicit
**cache-format version** and a **syntax-version key** (so a parser change
invalidates stale entries — the content-keyed caveat); **atomic temp-file write +
rename**; never mutate the LRU while holding a shared/read lock; centralize the
source fingerprint (SHA-256) calculation; treat corrupt entries as misses. Test
concurrent access, stale timestamps, and same-content/different-timestamp. Rust
need not read Go `gob`. Reuse `SyntaxTree`'s `Box<[Node]>`/`Box<[NodeId]>` +
`Arc<[u8]>` layout for serialization, but do **not** attempt zero-copy disk
deserialization yet (parity/correctness first). Do not begin cache work until
Milestone 4 is the assigned run.
