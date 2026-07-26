//! Shared test helpers for the pdxl workspace.
//!
//! This crate is **dev-infrastructure only** (`publish = false`): production
//! crates use it exclusively through `[dev-dependencies]`, so nothing here can
//! leak into a release build. It centralizes the helpers that every test suite
//! kept reinventing: locating the repository root, discovering script
//! fixtures, and building self-cleaning temporary directory trees.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub mod facts_golden;

/// Every game whose fixtures live under `testdata/<game>/`.
///
/// **Adding a game target is one entry here** plus its own `testdata/<game>/`
/// directory and a schema-coupled suite that calls [`game_fixture_dirs`].
/// Syntax-level suites pick the new fixtures up for free.
pub const GAMES: &[&str] = &["ck3", "eu5"];

/// Fixture directories for suites that are **game-agnostic** — the lexer and
/// parser, which only see syntax. They walk every game's fixtures plus the
/// malformed-input corner, because more script is strictly better coverage and
/// no schema can be misapplied.
pub fn shared_fixture_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = GAMES
        .iter()
        .map(|g| root.join("testdata").join(g))
        .collect();
    dirs.push(root.join("testdata/lint"));
    dirs
}

/// Fixture directories for a **schema-coupled** suite — one game only.
///
/// Keeping these apart is the point: running one game's schema over another's
/// script produces meaningless facts, and the resulting goldens record
/// nonsense that later schema work has to keep re-approving.
pub fn game_fixture_dirs(root: &Path, game: &str) -> Vec<PathBuf> {
    assert!(GAMES.contains(&game), "unknown game {game:?}");
    vec![root.join("testdata").join(game)]
}

/// Every `.txt` fixture directly inside `dirs`, sorted by path. Missing
/// directories are skipped, so a game may exist before it has fixtures.
pub fn collect_fixtures(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("txt") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Walks up from the calling crate's manifest dir to the repository root (the
/// directory containing the workspace `Cargo.toml` and `crates/`).
///
/// Call as `repo_root(env!("CARGO_MANIFEST_DIR"))` so the search starts from the
/// *calling* crate, not this one.
pub fn repo_root(manifest_dir: &str) -> PathBuf {
    let mut dir = PathBuf::from(manifest_dir);
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("crates").is_dir() {
            return dir;
        }
        assert!(
            dir.pop(),
            "could not locate repo root (no workspace Cargo.toml above {manifest_dir})"
        );
    }
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
