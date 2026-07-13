//! Project file discovery and Paradox mod-overlay resolution.
//!
//! A faithful port of the scanning/overlay half of `internal/files`. Callers
//! add source roots in load order (vanilla first, mod last); later roots shadow
//! earlier ones for the same normalized overlay key. This crate never reads or
//! parses file *contents* — `.mod` descriptor parsing lives in `pdxl-moddesc`,
//! precisely so this layer stays parser-free.
//!
//! The Go implementation is the oracle; behavior (including the always-zero
//! `Stats::shadowed`) is matched exactly, not "fixed".

mod fileset;
mod validate;

pub use fileset::{FileEntry, FileKind, FileSet, Stats};
pub use validate::{FileSetError, validate_fileset};
