//! Project file discovery and Paradox mod-overlay resolution.
//!
//! A faithful port of `internal/files`. Callers add source roots in load order
//! (vanilla first, mod last); later roots shadow earlier ones for the same
//! normalized overlay key. This crate discovers and resolves `.txt` files and
//! parses `.mod` descriptors — it does **not** read or parse project file
//! contents (only the requested `.mod` descriptor, via `pdxl-syntax`).
//!
//! Overlay keys and diagnostics use the same byte-offset / normalized-path model
//! as the rest of the workspace. The Go implementation is the oracle; behavior
//! (including the always-zero `Stats::shadowed`) is matched exactly, not "fixed".

mod dump;
mod fileset;
mod mod_descriptor;
mod path;

pub use dump::{DUMP_VERSION, dump_descriptor, dump_scan};
pub use fileset::{FileEntry, FileKind, FileSet, Stats};
pub use mod_descriptor::{ModDescriptor, parse_mod};
pub use path::is_windows_absolute;

// Path helpers exposed for tools/tests that need Go-compatible lexical behavior.
pub use path::{clean as clean_path, join as join_paths, normalize_key};
