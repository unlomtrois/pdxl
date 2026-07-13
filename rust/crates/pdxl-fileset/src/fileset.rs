//! `FileSet`: directory scanning + Paradox mod-overlay resolution.
//!
//! A faithful port of `internal/files`. Roots are added in load order (vanilla
//! first, mod last); a later root shadows an earlier one for the same overlay
//! key. Overlay keys are normalized lowercase, forward-slash relative paths.
//!
//! Parity-critical behaviors preserved from Go:
//! - **In-place winner replacement.** Re-adding an existing key overwrites its
//!   slot in `entries`; no historical entry is kept. As a consequence
//!   [`Stats::shadowed`] is always `0` (the Go code's shadow counter is dead —
//!   see the milestone report). This is matched deliberately, not "fixed".
//! - **Deterministic traversal.** Directory entries are sorted by name (byte
//!   order) at each level, reproducing Go `filepath.WalkDir`'s lexical order,
//!   because `std::fs::read_dir` order is unspecified.
//! - `replace_path` applies only to `Vanilla`/`Dlc` and only drops files (the
//!   mod need not provide a replacement); every dropped file bumps `replaced`.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use pdxl_path::{clean, has_txt_ext, join, normalize_key, to_lower};

/// Where a file originates in the load order. Variant order matches Go and must
/// not be reordered (it drives the `Stats` classification).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    Vanilla = 0,
    Dlc = 1,
    Dependency = 2,
    Mod = 3,
}

impl FileKind {
    /// Stable lowercase name used in differential dumps.
    pub const fn as_str(self) -> &'static str {
        match self {
            FileKind::Vanilla => "vanilla",
            FileKind::Dlc => "dlc",
            FileKind::Dependency => "dependency",
            FileKind::Mod => "mod",
        }
    }
}

/// A single resolved `.txt` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    /// Normalized overlay key (forward slashes, lowercase).
    pub rel_path: String,
    /// Path used to read the file. Not canonicalized — it is the cleaned root
    /// joined with the relative path, matching Go's stored `FullPath` (whose
    /// "absolute" doc comment is aspirational; a relative root stays relative).
    pub full_path: PathBuf,
    pub kind: FileKind,
}

/// Summary of a `FileSet` after scanning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Winning `Vanilla` + `Dlc` entries.
    pub vanilla: usize,
    /// Winning `Mod` + `Dependency` entries. (`mod` is a keyword; normalized to
    /// `"mod"` in dumps.)
    pub mod_files: usize,
    /// Total winning entries.
    pub total: usize,
    /// Vanilla files overridden by a later entry. Always `0` with the current
    /// in-place overlay model — preserved from Go, see the milestone report.
    pub shadowed: usize,
    /// Vanilla/DLC files dropped by `replace_path`.
    pub replaced: usize,
}

/// A collection of PDXScript files with overlay semantics applied. The default
/// value is ready to use.
#[derive(Default)]
pub struct FileSet {
    entries: Vec<FileEntry>,
    by_path: HashMap<String, usize>,
    replace_paths: Vec<String>,
    replaced: usize,
    ignore_dirs: HashSet<String>,
    ignore_files: HashSet<String>,
}

impl FileSet {
    /// Creates an empty `FileSet`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers directory and file base names to skip during [`add`](Self::add).
    /// Comparison is case-insensitive. Call before adding any roots.
    pub fn set_ignore<I, J, D, F>(&mut self, dirs: I, files: J)
    where
        I: IntoIterator<Item = D>,
        D: AsRef<str>,
        J: IntoIterator<Item = F>,
        F: AsRef<str>,
    {
        self.ignore_dirs = dirs.into_iter().map(|d| to_lower(d.as_ref())).collect();
        self.ignore_files = files.into_iter().map(|f| to_lower(f.as_ref())).collect();
    }

    /// Registers directory prefixes fully replaced by the mod. Vanilla/DLC files
    /// under one of these prefixes are dropped during [`add`](Self::add). Call
    /// before adding vanilla roots.
    pub fn set_replace_paths<I, P>(&mut self, paths: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<str>,
    {
        self.replace_paths = paths
            .into_iter()
            .map(|p| normalize_key(p.as_ref()))
            .collect();
    }

    /// Scans `root` for `.txt` files and registers them with `kind`.
    ///
    /// Must be called in load order (vanilla first, mod last). Directories whose
    /// base name starts with `.` or is in the ignore set are skipped at any
    /// depth. Returns the first traversal error encountered; entries registered
    /// before the error are kept (matching Go's non-rollback behavior).
    pub fn add(&mut self, root: impl AsRef<Path>, kind: FileKind) -> io::Result<()> {
        let root_str = clean(&root.as_ref().to_string_lossy());
        let root_path = PathBuf::from(&root_str);

        let meta = std::fs::symlink_metadata(&root_path)?;
        if meta.is_dir() {
            // Go's WalkDir applies skipDir to the root itself.
            let base = base_name(&root_str);
            if self.skip_dir(&base) {
                return Ok(());
            }
            self.scan_dir(&root_path, &root_str, "", kind)
        } else {
            // A file root: Go would register it with rel ".".
            self.visit_file(&root_path, &root_str, ".", &base_name(&root_str), kind);
            Ok(())
        }
    }

    /// Recursively scans `dir`, whose path relative to the scan root is `rel`
    /// (empty at the root). `root_str` is the cleaned scan root used to build
    /// stored full paths.
    fn scan_dir(
        &mut self,
        dir: &Path,
        root_str: &str,
        rel: &str,
        kind: FileKind,
    ) -> io::Result<()> {
        // Collect and sort by file name (byte order), matching filepath.WalkDir.
        let mut entries: Vec<(OsString, std::fs::FileType)> = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            entries.push((entry.file_name(), ft));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        for (name_os, ft) in entries {
            let name = name_os.to_string_lossy();
            let child_rel = if rel.is_empty() {
                name.to_string()
            } else {
                format!("{rel}/{name}")
            };
            let child_path = dir.join(&name_os);

            if ft.is_dir() {
                if self.skip_dir(&name) {
                    continue;
                }
                self.scan_dir(&child_path, root_str, &child_rel, kind)?;
            } else {
                self.visit_file(&child_path, root_str, &child_rel, &name, kind);
            }
        }
        Ok(())
    }

    /// Considers a single file for registration: `.txt` only, honoring the file
    /// ignore set, keyed by its normalized relative path.
    fn visit_file(
        &mut self,
        _full_path: &Path,
        root_str: &str,
        rel: &str,
        name: &str,
        kind: FileKind,
    ) {
        if !has_txt_ext(name) {
            return;
        }
        if self.ignore_files.contains(&to_lower(name)) {
            return;
        }
        // Stored full path = Join(root, rel), cleaned — matching WalkDir's path.
        let full = join(&[root_str, rel]);
        self.register(normalize_key(rel), PathBuf::from(full), kind);
    }

    /// Reports whether a directory base name should not be descended.
    fn skip_dir(&self, name: &str) -> bool {
        name.starts_with('.') || self.ignore_dirs.contains(&to_lower(name))
    }

    /// Adds or overlays a winning entry, applying `replace_path` dropping for
    /// vanilla/DLC. Existing keys are replaced in their original slot.
    fn register(&mut self, rel_key: String, full_path: PathBuf, kind: FileKind) {
        if (kind == FileKind::Vanilla || kind == FileKind::Dlc) && self.is_replaced(&rel_key) {
            self.replaced += 1;
            return;
        }
        let entry = FileEntry {
            rel_path: rel_key.clone(),
            full_path,
            kind,
        };
        if let Some(&idx) = self.by_path.get(&rel_key) {
            self.entries[idx] = entry;
        } else {
            let idx = self.entries.len();
            self.by_path.insert(rel_key, idx);
            self.entries.push(entry);
        }
    }

    /// Reports whether `rel_path` falls under a `replace_path` prefix.
    fn is_replaced(&self, rel_path: &str) -> bool {
        self.replace_paths
            .iter()
            .any(|prefix| rel_path == prefix || rel_path.starts_with(&format!("{prefix}/")))
    }

    /// Returns the winning entry for `rel_path` (normalized to lowercase
    /// forward-slash form), or `None`. No filesystem access.
    pub fn resolve(&self, rel_path: impl AsRef<str>) -> Option<&FileEntry> {
        let key = normalize_key(rel_path.as_ref());
        self.by_path.get(&key).map(|&idx| &self.entries[idx])
    }

    /// Iterates winning entries in stable insertion-slot order. Allocation-free.
    pub fn iter(&self) -> impl Iterator<Item = &FileEntry> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(i, e)| self.by_path.get(&e.rel_path) == Some(i))
            .map(|(_, e)| e)
    }

    /// Calls `f` for each winning entry in order, stopping at the first error.
    pub fn try_for_each<E>(&self, mut f: impl FnMut(&FileEntry) -> Result<(), E>) -> Result<(), E> {
        for (i, e) in self.entries.iter().enumerate() {
            if self.by_path.get(&e.rel_path) == Some(&i) {
                f(e)?;
            }
        }
        Ok(())
    }

    /// Returns a summary of the set after all `add` calls.
    pub fn stats(&self) -> Stats {
        let mut st = Stats {
            replaced: self.replaced,
            ..Stats::default()
        };
        for (i, e) in self.entries.iter().enumerate() {
            if self.by_path.get(&e.rel_path) != Some(&i) {
                // Dead branch with in-place overlay (kept for Go parity).
                if e.kind == FileKind::Vanilla || e.kind == FileKind::Dlc {
                    st.shadowed += 1;
                }
                continue;
            }
            st.total += 1;
            match e.kind {
                FileKind::Vanilla | FileKind::Dlc => st.vanilla += 1,
                FileKind::Mod | FileKind::Dependency => st.mod_files += 1,
            }
        }
        st
    }
}

/// The final path element of a cleaned path (its base name).
fn base_name(path: &str) -> String {
    match path.rfind('/') {
        Some(idx) => path[idx + 1..].to_string(),
        None => path.to_string(),
    }
}
