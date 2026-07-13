//! The in-memory L1: a small bounded LRU keyed by file path.
//!
//! **Every method takes `&mut self` — including `get`.** An LRU lookup is never
//! a read-only operation: it must record recency, which is a write. The Go
//! implementation hid that write behind a `sync.RWMutex` read lock (its
//! `list.MoveToFront` mutated the list under `RLock`), a data race we confirmed
//! with `go test -race`. In Rust the honest `&mut self` signature makes the
//! same mistake a compile error: you cannot call `get` through a shared
//! `RwLock` read guard, so the store wraps this in a plain `Mutex`.
//!
//! Recency is tracked with a monotonic **tick** per entry instead of a linked
//! list: `get`/`put` stamp the entry with a fresh tick, and eviction scans for
//! the minimum. That makes eviction O(cap) instead of O(1) — irrelevant at the
//! configured capacities (hundreds), and it removes the pointer-splicing that
//! caused the Go bug in the first place.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::CachedParse;

struct Slot {
    mtime_nanos: i64,
    parse: CachedParse,
    last_used: u64,
}

pub(crate) struct Lru {
    cap: usize,
    tick: u64,
    map: HashMap<PathBuf, Slot>,
}

impl Lru {
    /// Creates an LRU holding at most `cap` entries (`cap > 0`).
    pub fn new(cap: usize) -> Lru {
        debug_assert!(cap > 0, "cap 0 means 'no L1'; the store passes None");
        Lru {
            cap,
            tick: 0,
            map: HashMap::with_capacity(cap),
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// Returns the cached parse if present **and** fresh (matching mtime).
    /// A stale entry is evicted so the caller falls through to L2 — mirroring
    /// Go's `getL1` contract.
    pub fn get(&mut self, path: &Path, mtime_nanos: i64) -> Option<CachedParse> {
        let tick = self.next_tick();
        match self.map.get_mut(path) {
            Some(slot) if slot.mtime_nanos == mtime_nanos => {
                slot.last_used = tick;
                Some(slot.parse.clone()) // clone = two Arc bumps, no data copy
            }
            Some(_) => {
                self.map.remove(path);
                None
            }
            None => None,
        }
    }

    /// Inserts or refreshes an entry, evicting the least-recently-used one
    /// when a *new* key would exceed capacity.
    pub fn put(&mut self, path: &Path, mtime_nanos: i64, parse: CachedParse) {
        let tick = self.next_tick();
        if let Some(slot) = self.map.get_mut(path) {
            *slot = Slot {
                mtime_nanos,
                parse,
                last_used: tick,
            };
            return;
        }
        if self.map.len() == self.cap {
            // O(cap) scan for the oldest tick; cap is small by configuration.
            if let Some(oldest) = self
                .map
                .iter()
                .min_by_key(|(_, slot)| slot.last_used)
                .map(|(key, _)| key.clone())
            {
                self.map.remove(&oldest);
            }
        }
        self.map.insert(
            path.to_path_buf(),
            Slot {
                mtime_nanos,
                parse,
                last_used: tick,
            },
        );
    }

    /// Whether `path` currently has an L1 entry (introspection for tests and
    /// future `cache size` tooling).
    pub fn contains(&self, path: &Path) -> bool {
        self.map.contains_key(path)
    }

    /// Number of live L1 entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }
}
