# Milestone Report — Rust Port: Syntax Cache

## Status
Complete.

## Branch
`rust-port-milestone-4` (based on `5c529e6`, the 9-crate regranularization).

## Commits
- (see git log; feature + report commits on this branch)

## Reference Go commit
`5d02a979cb162ba5d89c7e705618de322884bd79`

## Scope note
Per the milestone spec, this ports the **concept** of the two-level AST cache,
not the Go `gob` encoding, and *mandates* fixing the known design weaknesses
rather than reproducing them. Rust does not read Go cache entries; the formats
are independent by design.

## Implemented
- **`pdxl-cache` crate**: `Store::new(dir, lru_cap)` / `get(path, mtime_nanos)`
  / `put(path, mtime_nanos, src, CachedParse)`; `CachedParse { tree:
  Arc<SyntaxTree>, diagnostics: Arc<[Diagnostic]> }` (clone = two Arc bumps).
- **L1**: bounded LRU keyed by path, recency via monotonic ticks (O(cap)
  eviction scan — trivial at configured capacities), behind a `Mutex`.
- **L2**: postcard-encoded `DiskEntry` files named
  `hex(sha256(clean(path))).bin`, each self-contained: version keys, mtime,
  SHA-256, raw source bytes, the tree's two flat arrays, diagnostics.
- **Invalidation (Go semantics preserved)**: L1 hits on matching mtime; L2
  always re-reads the file and verifies SHA-256; same-content/drifted-mtime
  entries self-heal (rewrite with the new mtime); `lru_cap = 0` disables L1;
  `.gitignore` (`*`) written next to the cache dir.
- Small production additions to support persistence: `TokenKind::{ALL,
  from_u8}` (+`repr(u8)`) in `pdxl-lexer`, `NodeKind::from_u8` and
  `SYNTAX_VERSION` in `pdxl-ast`, `Severity::from_u8` in `pdxl-parser` — all
  guarded by a discriminant-order roundtrip test.

## Improvements over Go (mandated by the spec)
1. **Version keys.** Every entry leads with `format_version` (this layout) and
   `syntax_version` (`pdxl_ast::SYNTAX_VERSION` — bump when lexer/parser/tree
   semantics change). Either mismatching ⇒ miss. Fixes the "content-keyed
   caveat" where stale entries silently outlived parser changes.
2. **Atomic writes.** Temp file (pid + counter uniquified) + `rename`, replacing
   Go's in-place `os.Create` truncation; a crash can no longer corrupt an entry,
   and concurrent writers to one entry leave a complete file.
3. **Race-free L1.** `Lru::get` takes `&mut self` (recency is a write) behind a
   `Mutex`. The Go implementation mutated `list.MoveToFront` under
   `RWMutex.RLock` — **a data race we confirmed with `go test -race`** using a
   two-entry alternating-reader probe (the shipped Go test misses it because a
   single hot entry short-circuits `MoveToFront`). Reported here, not fixed in
   Go.
4. **Corrupt entries are clean misses.** `DiskEntry::decode` validates postcard
   framing, both versions, and every enum byte (`from_u8`) before
   reconstruction; garbage/truncated/flipped entries can never produce a tree
   (Go's corrupt-gzip path silently yielded a nil-source tree with live
   offsets).
5. **Centralized fingerprint.** One `fingerprint` module defines content hash
   and entry filename; Go computed SHA-256 at three call sites.

## Format decisions (user-selected)
- **serde + postcard** for the entry encoding, with **mirror repr types**
  (`NodeRepr`, `DiagRepr`) so `pdxl-ast`/`pdxl-parser` stay serialization-free
  and the persistence contract lives entirely in `pdxl-cache`.
- **No compression** (Go gzipped source): script files are small, hot reads
  skip a decompress step, and the corrupt-gzip failure class disappears. Can be
  reintroduced behind a `format_version` bump if the corpus warrants it.
- New workspace dependencies: `serde` (derive), `postcard` (alloc), `sha2`.

## Tests
- **Go**: `go test ./...` green, unchanged (the race probe was temporary and
  removed; the race is reported, not patched).
- **Rust**: `cargo test --workspace` — 31 suites green. `pdxl-cache`: 5 unit
  tests (fingerprint stability, entry roundtrip incl. malformed-source
  diagnostics, version-mismatch miss, corrupt-bytes miss) + 12 integration
  tests: all 8 Go cache tests ported 1:1 (roundtrip, cold miss, L2 hit, stale
  content, same-content/new-mtime self-heal, LRU eviction with disk fallback,
  concurrent reads, cap-0 disk-only) plus corrupt/truncated entry misses,
  no-leftover-temp-files, concurrent writers to one entry, gitignore, and
  invariant validation of reconstructed malformed-source trees.
- **Concurrency**: the two-entry alternating-reader pattern that exposed the Go
  race runs as a 16-thread stress; safe by construction under the `Mutex`.
- **Regression**: all four differential suites re-run green (lexer 11/11,
  parser 11/11, fileset 5/5, descriptors 6/6); golden trees 8/8.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features
  -- -D warnings` clean; no `unsafe`.

## Benchmarks (fixture 24401 B; simple loop, not Criterion)
| Case | Go | Rust | Difference |
|---|---|---|---|
| CacheReadL1 | ~25 ns | ~126 ns | ~5× slower — the price of the race-free `Mutex` + `PathBuf` hashing; still ~400× faster than the disk path it guards |
| CacheReadDisk | ~184 µs | ~49 µs | ~3.7× faster (no gzip decompress) |
| CacheWriteDisk | ~606 µs | ~106 µs | ~5.7× faster (no gzip compress, despite temp+rename) |

Methodology caveats: Go numbers from `testing.B` (`make bench-cache`), Rust from
`cargo run --release -p pdxl-cache --example cachebench`; both dominated by the
same work (SHA-256 verify on read, encode+fsync-less write on write).

## Bugs or ambiguities discovered
- **Go L1 data race (new, confirmed).** `Store.getL1` holds `mu.RLock()` while
  `lruCache.get` calls `list.MoveToFront`, which splices list pointers. Two
  concurrent readers on a non-front entry race. Repro: two entries + 16
  alternating readers under `go test -race` → `WARNING: DATA RACE` at
  `cache.go:63`. The shipped `TestConcurrentReads` cannot catch it (one entry ⇒
  `MoveToFront` short-circuits without writing). Suggested upstream fix: use a
  plain `sync.Mutex` for all LRU access, or make `getL1` take the write lock.
- Go's `gzipDecompress` swallows errors and returns `nil`, so a corrupt-gzip /
  valid-gob entry reconstructs a tree whose offsets point into a nil source.
  Not reachable in the Rust design (no compression; full validation).

## Files changed
- Added: `rust/crates/pdxl-cache/**` (`src/{lib,entry,lru,fingerprint}.rs`,
  `tests/store.rs`, `examples/cachebench.rs`, `Cargo.toml`),
  `rust/docs/MILESTONE-4-REPORT.md`.
- Modified: `rust/Cargo.toml` (+member, +serde/postcard/sha2 workspace deps),
  `rust/Cargo.lock`, `rust/README.md`, `rust/docs/BASELINE.md`,
  `pdxl-lexer/src/lib.rs` (+`repr(u8)`, `ALL`, `from_u8`),
  `pdxl-lexer/src/tests.rs` (+roundtrip guard), `pdxl-ast/src/node.rs`
  (+`NodeKind::from_u8`), `pdxl-ast/src/lib.rs` (+`SYNTAX_VERSION`),
  `pdxl-parser/src/diagnostic.rs` (+`Severity::from_u8`).
- Unchanged: all Go source.

## Risks for later milestones
- **`SYNTAX_VERSION` discipline is manual.** Nothing detects a parser change
  that forgets the bump; the differential suites catch behavior drift, but a
  stale-cache bug would only surface for users with warm caches. Consider a CI
  reminder (e.g. hash of parser sources) later.
- **Entry filenames key on the path only** (Go parity): renames orphan entries;
  a `cache clear`/GC command (M-CLI) should sweep unknown files.
- **`CachedParse` shares trees.** Consumers must treat `Arc<SyntaxTree>` as
  immutable (it is, structurally); the facts layer (M5) should key its own
  cache the same way (content + schema version + rel_path).
- Times are `i64` nanoseconds since epoch (fine until 2262); mtime `0` is used
  when the platform reports none — such files always take the hash path.

## Recommendation for Milestone 5 (per-file semantic facts) — do not begin
Port `internal/validate`'s `extractFacts` + `FactStore` into a generic
`pdxl-analysis` crate plus a CK3-specific `pdxl-ck3` rules crate. Preserve
`FileFacts { defs, aliases, refs }` extraction (single AST walk, directory-based
definition harvesting, key-based reference rules, `skipRefValue` exclusions).
The facts cache should reuse this milestone's pattern exactly: versioned
postcard entries (content SHA + **schema version** + rel_path in the key,
since directory location affects semantics), atomic writes, corrupt = miss.
Differential: an additive Go `tools/factsdump` emitting normalized JSON facts
per fixture, compared byte-for-byte.
