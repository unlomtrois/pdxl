//! FileSet unit tests: every case from `internal/files/files_test.go`, plus the
//! gaps the Go tests don't lock down (unsorted winner order, in-place slot
//! retention, the always-zero `shadowed` counter, DLC/dependency kinds, `.TXT`,
//! nested dot dirs, resolve normalization, replace-prefix boundaries).

mod common;

use common::{TempTree, validate_fileset};
use pdxl_files::{FileKind, FileSet};

fn winners(fs: &FileSet) -> Vec<String> {
    fs.iter().map(|e| e.rel_path.clone()).collect()
}

#[test]
fn add_and_resolve() {
    let t = TempTree::new();
    t.write("common/traits/noble.txt", "");

    let mut s = FileSet::new();
    s.add(&t.path, FileKind::Vanilla).unwrap();

    let e = s.resolve("common/traits/noble.txt").expect("found");
    assert_eq!(e.kind, FileKind::Vanilla);
    assert_eq!(e.rel_path, "common/traits/noble.txt");
    validate_fileset(&s);
}

#[test]
fn set_ignore() {
    let t = TempTree::new();
    t.write("common/traits/noble.txt", "");
    t.write("licenses/software/zlib.txt", "");
    t.write("credits.txt", "");
    t.write("fonts/Open_Sans/LICENSE.txt", ""); // case-insensitive file ignore

    let mut s = FileSet::new();
    s.set_ignore(["licenses"], ["credits.txt", "license.txt"]);
    s.add(&t.path, FileKind::Vanilla).unwrap();

    assert!(s.resolve("common/traits/noble.txt").is_some());
    for rel in [
        "licenses/software/zlib.txt",
        "credits.txt",
        "fonts/Open_Sans/LICENSE.txt",
    ] {
        assert!(s.resolve(rel).is_none(), "{rel} should be ignored");
    }
    assert_eq!(s.stats().total, 1);
    validate_fileset(&s);
}

#[test]
fn overlay_shadowing() {
    let van = TempTree::new();
    let md = TempTree::new();
    van.write("common/traits/noble.txt", "vanilla");
    md.write("common/traits/noble.txt", "mod");

    let mut s = FileSet::new();
    s.add(&van.path, FileKind::Vanilla).unwrap();
    s.add(&md.path, FileKind::Mod).unwrap();

    let e = s.resolve("common/traits/noble.txt").unwrap();
    assert_eq!(e.kind, FileKind::Mod, "mod should shadow vanilla");
    validate_fileset(&s);
}

#[test]
fn walk_all_winners_unsorted_order() {
    let van = TempTree::new();
    let md = TempTree::new();
    van.write("a.txt", "");
    van.write("b.txt", "");
    md.write("b.txt", ""); // shadows vanilla b.txt, keeps its slot
    md.write("c.txt", "");

    let mut s = FileSet::new();
    s.add(&van.path, FileKind::Vanilla).unwrap();
    s.add(&md.path, FileKind::Mod).unwrap();

    // The Go test sorts before asserting; we lock the exact unsorted slot order:
    // a (vanilla), b (mod, in vanilla's original slot), c (mod, appended).
    assert_eq!(winners(&s), vec!["a.txt", "b.txt", "c.txt"]);
    assert_eq!(s.resolve("b.txt").unwrap().kind, FileKind::Mod);
    validate_fileset(&s);
}

#[test]
fn overlay_winner_keeps_original_slot() {
    // c.txt is added (mod) before b.txt is shadowed; b.txt must NOT migrate after c.
    let van = TempTree::new();
    let md = TempTree::new();
    van.write("a.txt", "");
    van.write("b.txt", "");
    van.write("d.txt", "");
    md.write("b.txt", "");

    let mut s = FileSet::new();
    s.add(&van.path, FileKind::Vanilla).unwrap();
    s.add(&md.path, FileKind::Mod).unwrap();

    assert_eq!(winners(&s), vec!["a.txt", "b.txt", "d.txt"]);
    validate_fileset(&s);
}

#[test]
fn skip_dot_dirs() {
    let t = TempTree::new();
    t.write(".git/config.txt", "");
    t.write("real.txt", "");

    let mut s = FileSet::new();
    s.add(&t.path, FileKind::Mod).unwrap();

    assert_eq!(winners(&s), vec!["real.txt"]);
    validate_fileset(&s);
}

#[test]
fn nested_dot_dir_skipped() {
    let t = TempTree::new();
    t.write("common/.hidden/x.txt", "");
    t.write("common/visible/y.txt", "");

    let mut s = FileSet::new();
    s.add(&t.path, FileKind::Mod).unwrap();

    assert_eq!(winners(&s), vec!["common/visible/y.txt"]);
}

#[test]
fn nested_ignore_dir() {
    let t = TempTree::new();
    t.write("a/licenses/x.txt", "");
    t.write("a/keep/y.txt", "");

    let mut s = FileSet::new();
    s.set_ignore(["licenses"], Vec::<String>::new());
    s.add(&t.path, FileKind::Mod).unwrap();

    assert_eq!(winners(&s), vec!["a/keep/y.txt"]);
}

#[test]
fn replace_path_drops_vanilla() {
    let van = TempTree::new();
    let md = TempTree::new();
    van.write("common/landed_titles/base.txt", "");
    van.write("common/traits/noble.txt", "");
    md.write("common/landed_titles/custom.txt", "");

    let mut s = FileSet::new();
    s.set_replace_paths(["common/landed_titles"]);
    s.add(&van.path, FileKind::Vanilla).unwrap();
    s.add(&md.path, FileKind::Mod).unwrap();

    assert!(s.resolve("common/landed_titles/base.txt").is_none());
    assert!(s.resolve("common/landed_titles/custom.txt").is_some());
    assert!(s.resolve("common/traits/noble.txt").is_some());
    assert_eq!(s.stats().replaced, 1);
    validate_fileset(&s);
}

#[test]
fn replace_path_prefix_boundary() {
    // prefix "common/traits" must not match "common/traitsets" or "common/traits_x".
    let van = TempTree::new();
    van.write("common/traits/a.txt", ""); // descendant → dropped
    van.write("common/traitsets/b.txt", ""); // similar but not under prefix → kept
    van.write("common/traits_extra/c.txt", ""); // kept

    let mut s = FileSet::new();
    s.set_replace_paths(["common/traits"]);
    s.add(&van.path, FileKind::Vanilla).unwrap();

    assert!(s.resolve("common/traits/a.txt").is_none());
    assert!(s.resolve("common/traitsets/b.txt").is_some());
    assert!(s.resolve("common/traits_extra/c.txt").is_some());
    assert_eq!(s.stats().replaced, 1);
}

#[test]
fn replace_path_exact_txt_match() {
    // The `rel_path == prefix` branch only fires for a .txt file whose key equals
    // the prefix exactly (a non-.txt file is never registered).
    let van = TempTree::new();
    van.write("common/foo.txt", ""); // exact match → dropped
    van.write("common/foobar.txt", ""); // not under prefix → kept

    let mut s = FileSet::new();
    s.set_replace_paths(["common/foo.txt"]);
    s.add(&van.path, FileKind::Vanilla).unwrap();

    assert!(s.resolve("common/foo.txt").is_none());
    assert!(s.resolve("common/foobar.txt").is_some());
    assert_eq!(s.stats().replaced, 1);
}

#[test]
fn replace_path_only_vanilla_and_dlc() {
    // replace_path drops Vanilla and DLC, but never Dependency or Mod.
    for (kind, dropped) in [
        (FileKind::Vanilla, true),
        (FileKind::Dlc, true),
        (FileKind::Dependency, false),
        (FileKind::Mod, false),
    ] {
        let t = TempTree::new();
        t.write("common/landed_titles/x.txt", "");
        let mut s = FileSet::new();
        s.set_replace_paths(["common/landed_titles"]);
        s.add(&t.path, kind).unwrap();
        let present = s.resolve("common/landed_titles/x.txt").is_some();
        assert_eq!(present, !dropped, "kind {kind:?}");
    }
}

#[test]
fn shadowed_is_always_zero() {
    // Confirmed Go oracle behavior: the in-place overlay never leaves a stale
    // entry, so Stats.Shadowed stays 0 even when a mod shadows vanilla.
    let van = TempTree::new();
    let md = TempTree::new();
    van.write("common/x.txt", "");
    md.write("common/x.txt", "");

    let mut s = FileSet::new();
    s.add(&van.path, FileKind::Vanilla).unwrap();
    s.add(&md.path, FileKind::Mod).unwrap();

    let st = s.stats();
    assert_eq!(st.shadowed, 0, "shadowed must be 0 (matches Go)");
    assert_eq!(st.total, 1);
    assert_eq!(st.mod_files, 1);
    assert_eq!(st.vanilla, 0);
}

#[test]
fn all_four_kinds_same_path() {
    // The same path present in all four kinds: last-added (mod) wins.
    let v = TempTree::new();
    let d = TempTree::new();
    let dep = TempTree::new();
    let md = TempTree::new();
    for t in [&v, &d, &dep, &md] {
        t.write("common/x.txt", "");
    }
    let mut s = FileSet::new();
    s.add(&v.path, FileKind::Vanilla).unwrap();
    s.add(&d.path, FileKind::Dlc).unwrap();
    s.add(&dep.path, FileKind::Dependency).unwrap();
    s.add(&md.path, FileKind::Mod).unwrap();

    assert_eq!(s.resolve("common/x.txt").unwrap().kind, FileKind::Mod);
    assert_eq!(s.stats().total, 1);
    validate_fileset(&s);
}

#[test]
fn dlc_and_dependency_classification() {
    let dlc = TempTree::new();
    let dep = TempTree::new();
    dlc.write("a.txt", "");
    dep.write("b.txt", "");
    let mut s = FileSet::new();
    s.add(&dlc.path, FileKind::Dlc).unwrap();
    s.add(&dep.path, FileKind::Dependency).unwrap();
    let st = s.stats();
    assert_eq!(st.vanilla, 1, "DLC counts as vanilla");
    assert_eq!(st.mod_files, 1, "dependency counts as mod");
    assert_eq!(st.total, 2);
}

#[test]
fn uppercase_txt_extension() {
    let t = TempTree::new();
    t.write("events.TXT", "");
    t.write("readme.md", "");
    let mut s = FileSet::new();
    s.add(&t.path, FileKind::Mod).unwrap();
    assert_eq!(winners(&s), vec!["events.txt"]); // key is lowercased
}

#[test]
fn non_txt_skipped() {
    let t = TempTree::new();
    t.write("mod.mod", "");
    t.write("notes.log", "");
    t.write("events.txt", "");
    let mut s = FileSet::new();
    s.add(&t.path, FileKind::Mod).unwrap();
    assert_eq!(winners(&s), vec!["events.txt"]);
}

#[test]
fn case_insensitive_resolve() {
    let t = TempTree::new();
    t.write("Common/Traits/Noble.txt", "");
    let mut s = FileSet::new();
    s.add(&t.path, FileKind::Mod).unwrap();
    // Keys are normalized lowercase; resolve normalizes its query too.
    assert!(s.resolve("COMMON/TRAITS/NOBLE.TXT").is_some());
    assert_eq!(
        s.resolve("common/traits/noble.txt").unwrap().rel_path,
        "common/traits/noble.txt"
    );
}

#[test]
fn case_collision_later_wins() {
    // "A.txt" and "a.txt" normalize to the same key; later one wins its slot.
    let t = TempTree::new();
    t.write("A.txt", "first");
    t.write("a.txt", "second");
    let mut s = FileSet::new();
    s.add(&t.path, FileKind::Mod).unwrap();
    // Exactly one winner under key "a.txt".
    assert_eq!(winners(&s), vec!["a.txt"]);
    validate_fileset(&s);
}

#[test]
fn missing_root_is_error() {
    let mut s = FileSet::new();
    let err = s.add("/no/such/path/anywhere-xyz", FileKind::Mod);
    assert!(err.is_err(), "missing root must return an error");
}

#[test]
fn try_for_each_stops_on_error() {
    let t = TempTree::new();
    t.write("a.txt", "");
    t.write("b.txt", "");
    t.write("c.txt", "");
    let mut s = FileSet::new();
    s.add(&t.path, FileKind::Mod).unwrap();

    let mut count = 0;
    let res: Result<(), ()> = s.try_for_each(|_e| {
        count += 1;
        if count == 2 { Err(()) } else { Ok(()) }
    });
    assert!(res.is_err());
    assert_eq!(count, 2, "iteration must stop at the first error");
}

#[test]
fn deterministic_across_repeated_scans() {
    let t = TempTree::new();
    t.write("z/last.txt", "");
    t.write("a/first.txt", "");
    t.write("m/mid.txt", "");

    let order1 = {
        let mut s = FileSet::new();
        s.add(&t.path, FileKind::Mod).unwrap();
        winners(&s)
    };
    let order2 = {
        let mut s = FileSet::new();
        s.add(&t.path, FileKind::Mod).unwrap();
        winners(&s)
    };
    assert_eq!(order1, order2, "scan order must be deterministic");
    // And it must be lexical by component.
    assert_eq!(order1, vec!["a/first.txt", "m/mid.txt", "z/last.txt"]);
}
