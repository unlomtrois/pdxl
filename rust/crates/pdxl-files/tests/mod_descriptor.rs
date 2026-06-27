//! `.mod` descriptor tests: ported from `files_test.go` plus the milestone gaps
//! (duplicate/repeated fields, malformed-but-readable recovery, Windows paths).

mod common;

use std::path::{Path, PathBuf};

use common::TempTree;
use pdxl_files::{is_windows_absolute, parse_mod};

fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("go.mod").is_file() {
            return dir;
        }
        assert!(dir.pop(), "could not locate repo root");
    }
}

#[test]
fn parse_repo_t4n_mod() {
    let mod_file = repo_root().join("testdata/T4N.mod");
    let m = parse_mod(&mod_file).unwrap();
    assert_eq!(m.name, "The Four Nations");
    assert!(!m.path.as_os_str().is_empty());
    assert!(!m.replace_paths.is_empty());
    assert!(m.replace_paths.iter().any(|p| p == "common/landed_titles"));
}

#[test]
fn preserves_duplicate_replace_paths() {
    // T4N.mod declares common/achievements twice; both must be kept, in order.
    let mod_file = repo_root().join("testdata/T4N.mod");
    let m = parse_mod(&mod_file).unwrap();
    let dups = m
        .replace_paths
        .iter()
        .filter(|p| p.as_str() == "common/achievements")
        .count();
    assert_eq!(dups, 2, "duplicate replace_path must not be deduplicated");
}

#[test]
fn windows_path_kept_verbatim() {
    let t = TempTree::new();
    let mod_file = t.child("test.mod");
    std::fs::write(
        &mod_file,
        "name=\"Test Mod\"\npath=\"C:/users/steamuser/Documents/Paradox Interactive/Crusader Kings III/mod/mods/TestMod\"\n",
    )
    .unwrap();
    let m = parse_mod(&mod_file).unwrap();
    assert!(
        is_windows_absolute(&m.path.to_string_lossy()),
        "Windows absolute path must be kept as-is, got {:?}",
        m.path
    );
}

#[test]
fn windows_backslash_path_kept() {
    let t = TempTree::new();
    let mod_file = t.child("bs.mod");
    std::fs::write(&mod_file, "path=\"C:\\users\\steamuser\\mods\\X\"\n").unwrap();
    let m = parse_mod(&mod_file).unwrap();
    assert_eq!(m.path, PathBuf::from("C:\\users\\steamuser\\mods\\X"));
}

#[test]
fn relative_path_joined_to_mod_dir() {
    let t = TempTree::new();
    let mod_file = t.child("mymod.mod");
    std::fs::write(&mod_file, "name=\"My Mod\"\npath=\"mods/mymod\"\n").unwrap();
    let m = parse_mod(&mod_file).unwrap();
    // Joined to the .mod file's directory (the temp dir), not the CWD.
    let expected = Path::new(&t.path).join("mods").join("mymod");
    assert_eq!(m.path, expected);
}

#[test]
fn later_name_and_path_overwrite() {
    let t = TempTree::new();
    let mod_file = t.child("dup.mod");
    std::fs::write(
        &mod_file,
        "name=\"First\"\nname=\"Second\"\npath=\"a/one\"\npath=\"a/two\"\n",
    )
    .unwrap();
    let m = parse_mod(&mod_file).unwrap();
    assert_eq!(m.name, "Second");
    assert_eq!(m.path, Path::new(&t.path).join("a").join("two"));
}

#[test]
fn unknown_and_missing_fields() {
    let t = TempTree::new();
    let mod_file = t.child("sparse.mod");
    // Only replace_path; no name/path. Unknown fields ignored.
    std::fs::write(
        &mod_file,
        "version=\"1.0\"\ntags={ \"X\" }\nreplace_path=\"common/x\"\nsupported_version=\"1.19.*\"\n",
    )
    .unwrap();
    let m = parse_mod(&mod_file).unwrap();
    assert_eq!(m.name, "");
    assert_eq!(m.path, PathBuf::new());
    assert_eq!(m.replace_paths, vec!["common/x".to_string()]);
}

#[test]
fn malformed_but_readable_descriptor() {
    // An unterminated block and a stray field: the tolerant parser still yields
    // the recognizable facts from the partial tree.
    let t = TempTree::new();
    let mod_file = t.child("broken.mod");
    std::fs::write(
        &mod_file,
        "name=\"Broken\"\nreplace_path=\"common/y\"\ntags={ \"oops\"\n",
    )
    .unwrap();
    let m = parse_mod(&mod_file).unwrap();
    assert_eq!(m.name, "Broken");
    assert_eq!(m.replace_paths, vec!["common/y".to_string()]);
}

#[test]
fn missing_mod_file_is_error() {
    let err = parse_mod("/no/such/file.mod");
    assert!(err.is_err(), "missing .mod file must return an error");
}

#[test]
fn is_windows_absolute_cases() {
    for (p, want) in [
        ("C:/foo", true),
        ("C:\\foo", true),
        ("D:/foo", true),
        ("c:/foo", true),
        ("/foo", false),
        ("foo/bar", false),
        ("C:foo", false),
        ("", false),
    ] {
        assert_eq!(is_windows_absolute(p), want, "is_windows_absolute({p:?})");
    }
}
