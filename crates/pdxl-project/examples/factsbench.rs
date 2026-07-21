//! Cold-path facts benchmark: read + parse + extract every `.txt` under a
//! root, sequentially and threaded. This is the measurement that decides
//! whether a persistent facts cache (Go's `FactStore`) ever earns its
//! complexity in the Rust port: if the cold path is already fast enough, the
//! cache stays unwritten.
//!
//! Usage:
//!   cargo run --release -p pdxl-project --example factsbench [-- <root>]
//!
//! Without a root, a CK3-scale corpus (~3,500 files) is synthesized by
//! replicating the repository fixtures into a temp tree.

use std::path::PathBuf;
use std::time::Instant;

use pdxl_analysis::extract_facts;
use pdxl_ck3::schema;
use pdxl_fileset::{FileKind, FileSet};

fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("crates").is_dir() {
            return dir;
        }
        assert!(dir.pop(), "could not locate repo root");
    }
}

/// Replicates the repo fixtures under plausible CK3 directories until the
/// corpus reaches roughly CK3 size (~3,500 files).
fn synthesize_corpus(target_files: usize) -> PathBuf {
    let root = std::env::temp_dir().join(format!("pdxl-factsbench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let fixtures: Vec<Vec<u8>> = std::fs::read_dir(repo_root().join("testdata"))
        .expect("read testdata")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("txt"))
        .map(|p| std::fs::read(p).expect("read fixture"))
        .collect();
    let dirs = [
        "common/scripted_triggers",
        "common/scripted_effects",
        "common/traits",
        "common/decisions",
        "common/on_action",
        "events",
        "history/characters",
        "gfx/models", // some files that match no def rule, like a real corpus
    ];
    let mut written = 0;
    'outer: for round in 0.. {
        for (d, dir) in dirs.iter().enumerate() {
            let content = &fixtures[(round + d) % fixtures.len()];
            let path = root.join(dir).join(format!("{round:04}.txt"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            written += 1;
            if written >= target_files {
                break 'outer;
            }
        }
    }
    root
}

/// One full cold pass: read, parse, extract. Returns (files, bytes, defs, refs).
fn cold_pass(entries: &[(String, PathBuf)], threads: usize) -> (usize, usize, usize, usize) {
    let sch = schema();
    let chunk = entries.len().div_ceil(threads);
    let results: Vec<(usize, usize, usize, usize)> = std::thread::scope(|scope| {
        let handles: Vec<_> = entries
            .chunks(chunk)
            .map(|slice| {
                let sch = &sch;
                scope.spawn(move || {
                    let (mut files, mut bytes, mut defs, mut refs) = (0, 0, 0, 0);
                    for (rel, full) in slice {
                        let src = std::fs::read(full).expect("read");
                        bytes += src.len();
                        let parsed = pdxl_parser::parse(rel.clone(), src);
                        let facts =
                            extract_facts(parsed.tree(), rel, &full.to_string_lossy(), sch, None);
                        files += 1;
                        defs += facts.defs.len();
                        refs += facts.refs.len();
                    }
                    (files, bytes, defs, refs)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    results.into_iter().fold((0, 0, 0, 0), |a, b| {
        (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3)
    })
}

fn main() {
    let arg_root = std::env::args().nth(1).map(PathBuf::from);
    let (root, synthetic) = match arg_root {
        Some(r) => (r, false),
        None => (synthesize_corpus(3500), true),
    };

    let mut fs = FileSet::new();
    fs.add(&root, FileKind::Mod).expect("scan root");
    let entries: Vec<(String, PathBuf)> = fs
        .iter()
        .map(|e| (e.rel_path.clone(), e.full_path.clone()))
        .collect();

    println!(
        "factsbench: {} files under {}{}\n",
        entries.len(),
        root.display(),
        if synthetic { " (synthesized)" } else { "" },
    );

    for threads in [
        1,
        std::thread::available_parallelism().map_or(4, |n| n.get()),
    ] {
        // Warm the page cache once so runs compare CPU, not first-touch I/O.
        let start = Instant::now();
        let (files, bytes, defs, refs) = cold_pass(&entries, threads);
        let elapsed = start.elapsed();
        println!(
            "threads={threads:<3} {elapsed:>10.1?}  {files} files, {:.1} MB, {defs} defs, {refs} refs  ({:.1} MB/s)",
            bytes as f64 / 1e6,
            bytes as f64 / 1e6 / elapsed.as_secs_f64(),
        );
    }

    if synthetic {
        let _ = std::fs::remove_dir_all(&root);
    }
}
