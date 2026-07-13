//! The single source of truth for cache hashing.
//!
//! The Go implementation computed SHA-256 in three separate call sites (entry
//! filename, `Put`, `Get` verification); this module centralizes both uses so
//! there is exactly one definition of "the content fingerprint" and one of "the
//! entry filename".

use std::path::Path;

use sha2::{Digest, Sha256};

/// SHA-256 of a file's raw bytes — the ground-truth freshness check.
pub fn content_hash(src: &[u8]) -> [u8; 32] {
    Sha256::digest(src).into()
}

/// The on-disk entry file name for a source path: `hex(sha256(clean(path))).bin`.
///
/// Hashed by *path* (Go parity): stable filesystem-safe names of fixed length,
/// and re-caching a file overwrites its single entry instead of accumulating
/// content-addressed orphans. The path is lexically cleaned first so `a/./b`
/// and `a/b` share an entry, matching Go's `filepath.Clean`.
pub fn entry_file_name(path: &Path) -> String {
    let cleaned = pdxl_path::clean(&path.to_string_lossy());
    let hash: [u8; 32] = Sha256::digest(cleaned.as_bytes()).into();
    let mut name = String::with_capacity(68);
    for byte in hash {
        name.push_str(&format!("{byte:02x}"));
    }
    name.push_str(".bin");
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_stable() {
        assert_eq!(content_hash(b"abc"), content_hash(b"abc"));
        assert_ne!(content_hash(b"abc"), content_hash(b"abd"));
    }

    #[test]
    fn entry_name_ignores_lexical_noise() {
        assert_eq!(
            entry_file_name(Path::new("a/./b.txt")),
            entry_file_name(Path::new("a/b.txt"))
        );
        assert!(entry_file_name(Path::new("x.txt")).ends_with(".bin"));
        assert_eq!(entry_file_name(Path::new("x.txt")).len(), 64 + 4);
    }
}
