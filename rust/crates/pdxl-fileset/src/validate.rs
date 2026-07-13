//! Structural invariant validator for a [`FileSet`].
//!
//! The FileSet analogue of `pdxl_ast::validate_tree`: a cheap, allocation-light
//! check that the overlay's internal bookkeeping is consistent. Used by every
//! test scenario (unit and differential); harmless to run in production
//! diagnostics too.

use pdxl_path::normalize_key;

use crate::fileset::FileSet;

/// A violated FileSet invariant, with a human-readable description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSetError(pub String);

/// Verifies the structural invariants of `fs`.
///
/// Checks: winner `rel_path`s are unique and already normalized (lowercase,
/// forward slashes); `resolve(rel_path)` returns the same winner; the stats
/// totals agree with the winner count and the vanilla/mod classification.
pub fn validate_fileset(fs: &FileSet) -> Result<(), FileSetError> {
    let stats = fs.stats();
    let mut seen = std::collections::HashSet::new();
    let mut count = 0usize;

    for e in fs.iter() {
        count += 1;

        if !seen.insert(e.rel_path.clone()) {
            return Err(FileSetError(format!(
                "duplicate rel_path among winners: {}",
                e.rel_path
            )));
        }
        if e.rel_path != normalize_key(&e.rel_path) {
            return Err(FileSetError(format!(
                "rel_path not normalized: {}",
                e.rel_path
            )));
        }
        if e.rel_path.contains('\\') {
            return Err(FileSetError(format!(
                "rel_path uses backslash: {}",
                e.rel_path
            )));
        }

        match fs.resolve(&e.rel_path) {
            Some(resolved) if resolved == e => {}
            _ => {
                return Err(FileSetError(format!(
                    "resolve() disagreed for {}",
                    e.rel_path
                )));
            }
        }
    }

    if stats.total != count {
        return Err(FileSetError(format!(
            "stats.total {} != winner count {count}",
            stats.total
        )));
    }
    if stats.vanilla + stats.mod_files != stats.total {
        return Err(FileSetError(format!(
            "vanilla {} + mod {} != total {}",
            stats.vanilla, stats.mod_files, stats.total
        )));
    }
    Ok(())
}
