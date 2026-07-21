//! Store behavior tests: every case from `internal/cache/cache_test.go` ported
//! 1:1, plus coverage for the deliberate improvements over the Go design
//! (version keys, corrupt-entry handling, atomic writes) and a concurrency
//! stress that reproduces the access pattern which exposed the Go LRU data
//! race (two entries, alternating readers).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pdxl_cache::{CachedParse, Store};
use pdxl_testutil::TempTree;

/// Parses `path` from disk and wraps the result for the cache.
fn parse_file(path: &Path) -> (Vec<u8>, i64, CachedParse) {
    let src = std::fs::read(path).unwrap();
    let mtime = Store::mtime_nanos(&std::fs::metadata(path).unwrap());
    let (tree, diags) =
        pdxl_parser::parse(path.to_string_lossy().into_owned(), src.clone()).into_parts();
    (
        src,
        mtime,
        CachedParse {
            tree: Arc::new(tree),
            diagnostics: diags.into(),
        },
    )
}

fn write_file(t: &TempTree, name: &str, content: &str) -> PathBuf {
    t.write(name, content);
    t.child(name)
}

/// Sets a file's mtime `secs` seconds forward without touching content.
fn bump_mtime(path: &Path, secs: u64) {
    let file = std::fs::File::options().append(true).open(path).unwrap();
    let new = std::time::SystemTime::now() + std::time::Duration::from_secs(secs);
    file.set_modified(new).unwrap();
}

// ── ports of the Go tests ───────────────────────────────────────────────────

#[test]
fn round_trip() {
    let t = TempTree::new();
    let store = Store::new(t.child("cache"), 4).unwrap();
    let path = write_file(&t, "a.txt", "key = value");
    let (src, mtime, parse) = parse_file(&path);

    store.put(&path, mtime, &src, parse.clone()).unwrap();
    let got = store.get(&path, mtime).expect("cache hit");

    assert_eq!(got.tree.nodes(), parse.tree.nodes());
    assert_eq!(got.tree.child_index(), parse.tree.child_index());
    assert_eq!(got.tree.source(), parse.tree.source());
    assert_eq!(&got.diagnostics[..], &parse.diagnostics[..]);
}

#[test]
fn cold_miss() {
    let t = TempTree::new();
    let store = Store::new(t.child("cache"), 4).unwrap();
    let path = write_file(&t, "a.txt", "key = value");
    let (_, mtime, _) = parse_file(&path);
    assert!(store.get(&path, mtime).is_none(), "expected cold miss");
}

#[test]
fn l2_hit_after_l1_bypass() {
    // Go's TestMtimeHit clears L1 by hand; here a disk-only store (cap 0)
    // proves the same L2 path, and a second store proves cross-process reuse.
    let t = TempTree::new();
    let path = write_file(&t, "a.txt", "key = value");
    let (src, mtime, parse) = parse_file(&path);

    let disk_only = Store::new(t.child("cache"), 0).unwrap();
    disk_only.put(&path, mtime, &src, parse).unwrap();
    assert_eq!(disk_only.l1_len(), 0, "cap 0 must mean no L1 at all");
    assert!(disk_only.get(&path, mtime).is_some(), "expected L2 hit");

    // A brand-new store over the same dir = a warm start in a new process.
    let second = Store::new(t.child("cache"), 4).unwrap();
    assert!(
        second.get(&path, mtime).is_some(),
        "expected warm-start hit"
    );
    assert!(second.l1_contains(&path), "L2 hit must populate L1");
}

#[test]
fn changed_content_is_stale() {
    let t = TempTree::new();
    let store = Store::new(t.child("cache"), 4).unwrap();
    let path = write_file(&t, "a.txt", "key = value");
    let (src, mtime, parse) = parse_file(&path);
    store.put(&path, mtime, &src, parse).unwrap();

    std::fs::write(&path, "key = changed").unwrap();
    bump_mtime(&path, 1);
    let new_mtime = Store::mtime_nanos(&std::fs::metadata(&path).unwrap());

    assert!(store.get(&path, new_mtime).is_none(), "expected stale miss");
}

#[test]
fn same_content_new_mtime_hits_and_self_heals() {
    let t = TempTree::new();
    let path = write_file(&t, "a.txt", "key = value");
    let (src, mtime, parse) = parse_file(&path);

    let store = Store::new(t.child("cache"), 0).unwrap(); // force the L2 path
    store.put(&path, mtime, &src, parse).unwrap();

    bump_mtime(&path, 2); // touch: same bytes, drifted mtime
    let new_mtime = Store::mtime_nanos(&std::fs::metadata(&path).unwrap());
    assert_ne!(new_mtime, mtime);

    assert!(
        store.get(&path, new_mtime).is_some(),
        "same content must hit despite mtime drift"
    );

    // Self-heal check: the entry now carries the new mtime, so a fresh store
    // with an L1 can serve it without the hash detour ever failing.
    let second = Store::new(t.child("cache"), 4).unwrap();
    assert!(second.get(&path, new_mtime).is_some());
}

#[test]
fn lru_eviction_falls_back_to_disk() {
    let t = TempTree::new();
    let store = Store::new(t.child("cache"), 2).unwrap();

    let paths: Vec<PathBuf> = ["a", "b", "c"]
        .iter()
        .map(|n| write_file(&t, &format!("{n}.txt"), &format!("{n} = 1")))
        .collect();
    let mut mtimes = Vec::new();
    for p in &paths {
        let (src, mtime, parse) = parse_file(p);
        store.put(p, mtime, &src, parse).unwrap();
        mtimes.push(mtime);
    }

    assert!(
        !store.l1_contains(&paths[0]),
        "oldest entry evicted from L1"
    );
    assert_eq!(store.l1_len(), 2);
    assert!(
        store.get(&paths[0], mtimes[0]).is_some(),
        "evicted entry must still hit on disk"
    );
    assert!(store.l1_contains(&paths[0]), "disk hit re-promotes into L1");
}

#[test]
fn concurrent_reads_two_entries() {
    // The access pattern that exposed the Go data race: two cached entries,
    // alternating concurrent readers, so recency bookkeeping happens on a
    // non-front entry. With the Mutex design this is boring by construction;
    // the test guards against reintroducing a reader/writer split.
    let t = TempTree::new();
    let store = Arc::new(Store::new(t.child("cache"), 8).unwrap());
    let path_a = write_file(&t, "a.txt", "a = 1");
    let path_b = write_file(&t, "b.txt", "b = 2");
    let (src_a, mtime_a, parse_a) = parse_file(&path_a);
    let (src_b, mtime_b, parse_b) = parse_file(&path_b);
    store.put(&path_a, mtime_a, &src_a, parse_a).unwrap();
    store.put(&path_b, mtime_b, &src_b, parse_b).unwrap();

    std::thread::scope(|scope| {
        for i in 0..16 {
            let store = &store;
            let (path, mtime) = if i % 2 == 0 {
                (&path_a, mtime_a)
            } else {
                (&path_b, mtime_b)
            };
            scope.spawn(move || {
                for _ in 0..200 {
                    assert!(store.get(path, mtime).is_some(), "concurrent get failed");
                }
            });
        }
    });
}

// ── improvements over Go ────────────────────────────────────────────────────

#[test]
fn corrupt_entry_is_a_clean_miss() {
    let t = TempTree::new();
    let store = Store::new(t.child("cache"), 4).unwrap();
    let path = write_file(&t, "a.txt", "key = value");
    let (src, mtime, parse) = parse_file(&path);
    store.put(&path, mtime, &src, parse).unwrap();

    let entry = store.entry_path(&path);

    // Garbage entry.
    std::fs::write(&entry, b"not a cache entry").unwrap();
    let fresh = Store::new(t.child("cache"), 0).unwrap();
    assert!(fresh.get(&path, mtime).is_none(), "garbage must miss");

    // Truncated entry (simulates a crash mid-write under the OLD in-place
    // scheme; the atomic rename makes this state unreachable going forward,
    // but a pre-existing corrupt file must still be tolerated).
    let (src, mtime, parse) = parse_file(&path);
    fresh.put(&path, mtime, &src, parse).unwrap();
    let full = std::fs::read(&entry).unwrap();
    std::fs::write(&entry, &full[..full.len() / 2]).unwrap();
    let fresh2 = Store::new(t.child("cache"), 0).unwrap();
    assert!(fresh2.get(&path, mtime).is_none(), "truncated must miss");
}

#[test]
fn put_leaves_no_temp_files() {
    let t = TempTree::new();
    let cache_dir = t.child("cache");
    let store = Store::new(&cache_dir, 4).unwrap();
    let path = write_file(&t, "a.txt", "key = value");
    let (src, mtime, parse) = parse_file(&path);
    store.put(&path, mtime, &src, parse).unwrap();

    let leftovers: Vec<_> = std::fs::read_dir(&cache_dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temp files must be renamed away");
}

#[test]
fn concurrent_writers_same_entry() {
    // Two threads racing put() on the same path: the atomic rename guarantees
    // readers always see one complete entry, never a torn one.
    let t = TempTree::new();
    let store = Arc::new(Store::new(t.child("cache"), 4).unwrap());
    let path = write_file(&t, "a.txt", "key = value");
    let (src, mtime, parse) = parse_file(&path);

    std::thread::scope(|scope| {
        for _ in 0..8 {
            let (store, src, parse, path) = (&store, &src, parse.clone(), &path);
            scope.spawn(move || {
                for _ in 0..50 {
                    store.put(path, mtime, src, parse.clone()).unwrap();
                    assert!(store.get(path, mtime).is_some());
                }
            });
        }
    });
    // The entry on disk is complete and valid afterwards.
    let fresh = Store::new(t.child("cache"), 0).unwrap();
    assert!(fresh.get(&path, mtime).is_some());
}

#[test]
fn gitignore_written_next_to_cache_dir() {
    let t = TempTree::new();
    let pdxl_dir = t.child(".pdxl");
    let _ = Store::new(pdxl_dir.join("cache"), 4).unwrap();
    let gitignore = pdxl_dir.join(".gitignore");
    assert_eq!(std::fs::read(&gitignore).unwrap(), b"*\n");
}

#[test]
fn reconstructed_tree_passes_invariants() {
    let t = TempTree::new();
    let store = Store::new(t.child("cache"), 0).unwrap();
    // Malformed source: diagnostics + partial tree must round-trip too.
    let path = write_file(&t, "broken.txt", "a = { b = { unclosed");
    let (src, mtime, parse) = parse_file(&path);
    assert!(!parse.diagnostics.is_empty());
    store.put(&path, mtime, &src, parse.clone()).unwrap();

    let got = store.get(&path, mtime).expect("hit");
    pdxl_ast::validate_tree(&got.tree).unwrap();
    assert_eq!(&got.diagnostics[..], &parse.diagnostics[..]);
}
