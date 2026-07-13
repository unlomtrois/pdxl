//! Shared test helpers for the pdxl workspace.
//!
//! This crate is **dev-infrastructure only** (`publish = false`): production
//! crates use it exclusively through `[dev-dependencies]`, so nothing here can
//! leak into a release build. It centralizes the helpers that every test suite
//! kept reinventing: locating the repository root, probing for the Go oracle,
//! and building self-cleaning temporary directory trees.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Walks up from the calling crate's manifest dir to the repository root (the
/// directory containing `go.mod`).
///
/// Call as `repo_root(env!("CARGO_MANIFEST_DIR"))` so the search starts from the
/// *calling* crate, not this one.
pub fn repo_root(manifest_dir: &str) -> PathBuf {
    let mut dir = PathBuf::from(manifest_dir);
    loop {
        if dir.join("go.mod").is_file() {
            return dir;
        }
        assert!(
            dir.pop(),
            "could not locate repo root (no go.mod above {manifest_dir})"
        );
    }
}

/// Reports whether a usable `go` toolchain is on PATH (the differential oracle).
pub fn go_available() -> bool {
    Command::new("go")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Creates a file (and its parent directories) at `dir/rel`, where `rel` uses
/// forward slashes.
pub fn write_file(dir: &Path, rel: &str, content: &str) {
    let full = dir.join(rel);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(&full, content).unwrap();
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A fresh temporary directory that removes itself on drop.
pub struct TempTree {
    pub path: PathBuf,
}

impl TempTree {
    /// Creates a unique directory under the system temp dir (no external deps).
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let base = std::env::temp_dir();
        loop {
            let candidate = base.join(format!(
                "pdxl-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            if std::fs::create_dir(&candidate).is_ok() {
                return TempTree { path: candidate };
            }
        }
    }

    /// Writes `content` to `rel` (forward-slash path) inside this tree.
    pub fn write(&self, rel: &str, content: &str) {
        write_file(&self.path, rel, content);
    }

    /// A path to a (not necessarily existing) child of this tree.
    pub fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
