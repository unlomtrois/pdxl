//! Shared test helpers: temp-tree builder and the FileSet invariant validator.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use pdxl_files::{FileSet, normalize_key};

/// Creates a file (and parent dirs) at `dir/rel`, where `rel` uses `/`.
pub fn write_file(dir: &Path, rel: &str, content: &str) {
    let full = dir.join(rel);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(&full, content).unwrap();
}

/// A fresh temp directory that cleans itself up on drop.
pub struct TempTree {
    pub path: PathBuf,
}

impl TempTree {
    pub fn new() -> Self {
        // Unique dir under the system temp without external deps.
        let base = std::env::temp_dir();
        let mut n = 0u64;
        loop {
            let candidate = base.join(format!(
                "pdxl-files-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + n
            ));
            if std::fs::create_dir(&candidate).is_ok() {
                return TempTree { path: candidate };
            }
            n += 1;
        }
    }

    pub fn write(&self, rel: &str, content: &str) {
        write_file(&self.path, rel, content);
    }

    pub fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Verifies the FileSet structural invariants required by the milestone. Panics
/// with a descriptive message on the first violation.
pub fn validate_fileset(fs: &FileSet) {
    let winners: Vec<_> = fs.iter().collect();
    let stats = fs.stats();

    // 5. no duplicate rel_path among winners.
    let mut seen = std::collections::HashSet::new();
    for e in &winners {
        assert!(
            seen.insert(&e.rel_path),
            "duplicate rel_path among winners: {}",
            e.rel_path
        );

        // 3 + 4. rel_path is lowercase and uses normalized overlay separators.
        assert_eq!(
            e.rel_path,
            normalize_key(&e.rel_path),
            "rel_path not normalized: {}",
            e.rel_path
        );
        assert!(
            !e.rel_path.contains('\\'),
            "rel_path uses backslash: {}",
            e.rel_path
        );

        // 6. resolve returns this same winner.
        let resolved = fs.resolve(&e.rel_path).expect("winner must resolve");
        assert_eq!(resolved, *e, "resolve disagreed for {}", e.rel_path);
    }

    // 7. stats.total == winner count.
    assert_eq!(stats.total, winners.len(), "stats.total != winner count");
    // 8. vanilla + mod_files == total.
    assert_eq!(
        stats.vanilla + stats.mod_files,
        stats.total,
        "vanilla + mod != total"
    );
}
