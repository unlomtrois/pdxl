//! Two-level parse cache: an in-memory LRU over a versioned on-disk store.
//!
//! Port of `internal/cache` — the two-level shape and invalidation *logic* are
//! preserved exactly, while the known design weaknesses are fixed (the port
//! spec for this milestone mandates the improvements rather than bug parity):
//!
//! | kept from Go | fixed vs Go |
//! |---|---|
//! | L1 mtime check guards L2 SHA-256 check | entries carry `format_version` + `syntax_version`; mismatch = miss |
//! | same-content/new-mtime entries self-heal | writes are atomic (temp file + rename), never in-place truncation |
//! | entry files named `sha256(clean(path)).bin` | L1 behind a `Mutex` — Go mutated its LRU under a shared `RWMutex` read lock (a `go test -race`-confirmed data race) |
//! | entries carry their own source bytes | any corrupt/truncated/alien entry is a clean miss, never a nil-source tree |
//! | `lru_cap = 0` disables L1 entirely | one `fingerprint` module instead of three ad-hoc hash sites |
//!
//! Invalidation model (unchanged): mtime is a cheap *hint* that lets the hot
//! path skip hashing; content SHA-256 is the *truth* that L2 always verifies
//! against the file's current bytes. Coarse filesystem timestamps mean mtime
//! alone can claim freshness after an in-place edit — hence the two tiers.

mod entry;
mod fingerprint;
mod lru;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pdxl_ast::SyntaxTree;
use pdxl_parser::Diagnostic;

use entry::DiskEntry;
pub use entry::FORMAT_VERSION;
use lru::Lru;

/// A cached parse result. `clone` is two `Arc` bumps — the tree and diagnostics
/// are shared, never copied, between the cache and all readers.
#[derive(Clone)]
pub struct CachedParse {
    pub tree: Arc<SyntaxTree>,
    pub diagnostics: Arc<[Diagnostic]>,
}

/// A two-level parse cache rooted at a directory.
pub struct Store {
    dir: PathBuf,
    /// `None` when `lru_cap == 0` (disk-only mode). The `Mutex` is deliberate:
    /// LRU `get` mutates recency state, so there is no shared/read fast path.
    lru: Option<Mutex<Lru>>,
    /// Uniquifies concurrent temp files within this process.
    tmp_counter: AtomicU64,
}

impl Store {
    /// Creates a store backed by `dir` (created if missing). `lru_cap` bounds
    /// the in-memory L1; `0` disables it and every hit pays the disk path.
    ///
    /// Writes `<parent-of-dir>/.gitignore` (`*`) on first use so cache files
    /// stay out of version control, matching the Go behavior for `.pdxl/`.
    pub fn new(dir: impl Into<PathBuf>, lru_cap: usize) -> io::Result<Store> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        if let Some(parent) = dir.parent() {
            let gitignore = parent.join(".gitignore");
            if !gitignore.exists() {
                let _ = std::fs::write(&gitignore, b"*\n");
            }
        }
        Ok(Store {
            dir,
            lru: (lru_cap > 0).then(|| Mutex::new(Lru::new(lru_cap))),
            tmp_counter: AtomicU64::new(0),
        })
    }

    /// Extracts the mtime in nanoseconds from file metadata — the freshness
    /// hint `get`/`put` expect. Callers already hold the metadata from their
    /// own `stat`, mirroring Go's `Get(path, info os.FileInfo)`.
    pub fn mtime_nanos(meta: &std::fs::Metadata) -> i64 {
        meta.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }

    /// Returns the cached parse for `path`, or `None` on any miss: cold cache,
    /// changed content, version mismatch, or corrupt entry. Never an error —
    /// a cache that cannot answer simply declines, and the caller re-parses.
    pub fn get(&self, path: &Path, mtime_nanos: i64) -> Option<CachedParse> {
        // L1: cheap mtime check under the lock, then release before any I/O.
        if let Some(lru) = &self.lru {
            let hit = lru
                .lock()
                .expect("cache lock poisoned")
                .get(path, mtime_nanos);
            if hit.is_some() {
                return hit;
            }
        }

        // L2: decode + validate the entry, then verify content ground truth.
        let bytes = std::fs::read(self.entry_path(path)).ok()?;
        let mut entry = DiskEntry::decode(&bytes)?;

        // Always verify the hash against the file's CURRENT bytes: mtime can
        // match even after an in-place edit on coarse-resolution filesystems.
        let current = std::fs::read(path).ok()?;
        if fingerprint::content_hash(&current) != entry.sha256 {
            return None; // content changed; caller must re-parse
        }

        if entry.mtime_nanos != mtime_nanos {
            // Same content, drifted mtime (touch, re-checkout): self-heal the
            // stored mtime so future L1/L2 checks stop paying the hash cost.
            entry.mtime_nanos = mtime_nanos;
            let _ = self.write_entry(path, &entry.encode());
        }

        let (tree, diagnostics) = entry.reconstruct();
        let parse = CachedParse { tree, diagnostics };
        self.insert_l1(path, mtime_nanos, parse.clone());
        Some(parse)
    }

    /// Stores a parse result. `src` must be the exact bytes of `path`'s content
    /// that produced `parse.tree` — offsets are meaningless against anything
    /// else.
    pub fn put(
        &self,
        path: &Path,
        mtime_nanos: i64,
        src: &[u8],
        parse: CachedParse,
    ) -> io::Result<()> {
        let entry = DiskEntry::build(
            mtime_nanos,
            fingerprint::content_hash(src),
            src,
            &parse.tree,
            &parse.diagnostics,
        );
        self.write_entry(path, &entry.encode())?;
        self.insert_l1(path, mtime_nanos, parse);
        Ok(())
    }

    /// The on-disk entry file for a source path (exposed for tests and the
    /// future `cache size` / `cache clear` tooling).
    pub fn entry_path(&self, path: &Path) -> PathBuf {
        self.dir.join(fingerprint::entry_file_name(path))
    }

    /// Number of entries currently in L1 (introspection).
    pub fn l1_len(&self) -> usize {
        self.lru
            .as_ref()
            .map(|l| l.lock().expect("cache lock poisoned").len())
            .unwrap_or(0)
    }

    /// Whether `path` currently has an L1 entry (introspection).
    pub fn l1_contains(&self, path: &Path) -> bool {
        self.lru
            .as_ref()
            .map(|l| l.lock().expect("cache lock poisoned").contains(path))
            .unwrap_or(false)
    }

    fn insert_l1(&self, path: &Path, mtime_nanos: i64, parse: CachedParse) {
        if let Some(lru) = &self.lru {
            lru.lock()
                .expect("cache lock poisoned")
                .put(path, mtime_nanos, parse);
        }
    }

    /// Atomically replaces the entry for `path`: write a uniquely-named temp
    /// file in the same directory, then `rename` over the target. On POSIX the
    /// rename is atomic, so readers observe either the old complete entry or
    /// the new complete entry — never a truncated one (Go's `os.Create`
    /// truncated in place, leaving a window where a crash corrupts the entry).
    fn write_entry(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let target = self.entry_path(path);
        let tmp = self.dir.join(format!(
            "{}.{}.{}.tmp",
            target.file_name().unwrap_or_default().to_string_lossy(),
            std::process::id(),
            self.tmp_counter.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::write(&tmp, bytes)?;
        match std::fs::rename(&tmp, &target) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp); // don't leak temp files
                Err(e)
            }
        }
    }
}
