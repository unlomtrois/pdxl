//! Minimal cache throughput benchmark, mirroring Go's cache benchmarks
//! (`BenchmarkCacheReadL1` / `BenchmarkCacheReadDisk` / `BenchmarkCacheWriteDisk`)
//! on the same large fixture. Parity-era sanity check, not a tuned benchmark.
//!
//! Run with: `cargo run --release -p pdxl-cache --example cachebench`

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use pdxl_cache::{CachedParse, Store};

fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("go.mod").is_file() {
            return dir;
        }
        assert!(dir.pop(), "could not locate repo root");
    }
}

fn bench(name: &str, bytes_per_op: usize, mut op: impl FnMut()) {
    let target = std::time::Duration::from_millis(500);
    let mut iters: u64 = 0;
    let start = Instant::now();
    loop {
        op();
        iters += 1;
        if iters.is_multiple_of(16) && start.elapsed() >= target {
            break;
        }
    }
    let ns_per_op = start.elapsed().as_nanos() as f64 / iters as f64;
    let mb_per_s = (bytes_per_op as f64) / (ns_per_op / 1e9) / (1024.0 * 1024.0);
    println!("{name:<22} {ns_per_op:>12.0} ns/op {mb_per_s:>10.2} MB/s  ({iters} iters)");
}

fn main() {
    let fixture = repo_root().join("testdata/international_organization.txt");
    let src = std::fs::read(&fixture).expect("read fixture");
    let mtime = Store::mtime_nanos(&std::fs::metadata(&fixture).unwrap());
    let (tree, diags) = pdxl_parser::parse("bench", src.clone()).into_parts();
    let parse = CachedParse {
        tree: Arc::new(tree),
        diagnostics: diags.into(),
    };

    let tmp = std::env::temp_dir().join(format!("pdxl-cachebench-{}", std::process::id()));
    println!(
        "pdxl-cache throughput (release), fixture {} B:\n",
        src.len()
    );

    // L1 read: warm store with the entry resident in memory.
    let store = Store::new(tmp.join("l1"), 4).unwrap();
    store.put(&fixture, mtime, &src, parse.clone()).unwrap();
    bench("CacheReadL1", src.len(), || {
        assert!(store.get(&fixture, mtime).is_some());
    });

    // Disk read: cap 0 disables L1, so every get pays decode + hash verify.
    let disk = Store::new(tmp.join("disk"), 0).unwrap();
    disk.put(&fixture, mtime, &src, parse.clone()).unwrap();
    bench("CacheReadDisk", src.len(), || {
        assert!(disk.get(&fixture, mtime).is_some());
    });

    // Disk write: encode + atomic temp-file + rename per op.
    bench("CacheWriteDisk", src.len(), || {
        disk.put(&fixture, mtime, &src, parse.clone()).unwrap();
    });

    let _ = std::fs::remove_dir_all(&tmp);
}
