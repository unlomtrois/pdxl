//! Whole-project analysis tests: every case from `resolve_test.go` and
//! `project_test.go` ported 1:1, plus a property the Go suite never states
//! outright: an incremental update must produce exactly the state a fresh
//! full analysis of the edited corpus would.

use std::path::PathBuf;

use pdxl_fileset::{FileKind, FileSet};
use pdxl_project::{Project, analyze};
use pdxl_testutil::TempTree;

fn fileset(dir: &TempTree) -> FileSet {
    let mut fs = FileSet::new();
    fs.add(&dir.path, FileKind::Mod).expect("scan");
    fs
}

fn resolve_dir(dir: &TempTree) -> Vec<pdxl_analysis::RefDiag> {
    let (_, diags) = analyze(&fileset(dir), &pdxl_ck3::schema()).expect("analyze");
    diags
}

fn project(dir: &TempTree) -> Project {
    Project::new(&fileset(dir), pdxl_ck3::schema()).expect("project")
}

// ── ports of resolve_test.go ─────────────────────────────────────────────────

#[test]
fn resolve_undefined_trait() {
    let d = TempTree::new();
    d.write("common/traits/00_traits.txt", "brave = { }\n");
    d.write(
        "common/scripted_effects/00_e.txt",
        "give_it = {\n\tadd_trait = brave\n\tadd_trait = nonexistent\n}\n",
    );
    let diags = resolve_dir(&d);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert!(diags[0].msg.contains("nonexistent"));
    assert!(diags[0].msg.contains("trait"));
}

#[test]
fn resolve_skips_macro_and_scope_values() {
    let d = TempTree::new();
    d.write("common/traits/00_traits.txt", "brave = { }\n");
    d.write(
        "common/scripted_effects/00_e.txt",
        "give_it = {\n\tadd_trait = $TRAIT$\n\thas_trait = scope:x\n}\n",
    );
    assert!(resolve_dir(&d).is_empty());
}

#[test]
fn resolve_trait_groups_and_quotes() {
    let d = TempTree::new();
    d.write(
        "common/traits/00_traits.txt",
        "brave = { group = personality_brave }\nbastard = { }\n",
    );
    d.write(
        "common/scripted_effects/00_e.txt",
        "give = {\n\thas_trait = personality_brave\n\tadd_trait = \"bastard\"\n}\n",
    );
    assert!(resolve_dir(&d).is_empty());
}

#[test]
fn resolve_event_references() {
    let d = TempTree::new();
    d.write(
        "events/test_events.txt",
        "namespace = test\ntest.0001 = { type = character_event }\n",
    );
    d.write(
        "common/scripted_effects/00_e.txt",
        "fire = {\n\ttrigger_event = test.0001\n\ttrigger_event = test.9999\n\ttrigger_event = { id = test.0001 days = 5 }\n\ttrigger_event = { id = test.8888 }\n}\n",
    );
    let diags = resolve_dir(&d);
    assert_eq!(diags.len(), 2, "{diags:?}");
    let joined = format!("{} {}", diags[0].msg, diags[1].msg);
    for want in ["test.9999", "test.8888", "event"] {
        assert!(joined.contains(want), "missing {want} in {joined}");
    }
}

#[test]
fn resolve_on_action_lists() {
    let d = TempTree::new();
    d.write(
        "events/test_events.txt",
        "namespace = test\ntest.0001 = { }\n",
    );
    d.write(
        "common/on_action/00_oa.txt",
        "real_oa = { }\nmy_oa = {\n\tevents = { test.0001 test.9999 }\n\tfirst_valid = { test.0001 }\n\trandom_events = { 100 = test.0001  50 = test.8888  chance_to_happen = 10  0 = 0 }\n\ton_actions = { real_oa missing_oa }\n}\n",
    );
    let diags = resolve_dir(&d);
    assert_eq!(diags.len(), 3, "{diags:?}");
    let joined: String = diags
        .iter()
        .map(|d| d.msg.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for want in ["test.9999", "test.8888", "missing_oa"] {
        assert!(joined.contains(want), "missing {want} in:\n{joined}");
    }
}

#[test]
fn resolve_list_rules_only_in_on_action_files() {
    let d = TempTree::new();
    d.write(
        "common/scripted_effects/00_e.txt",
        "e = { events = { totally.9999 } }\n",
    );
    assert!(resolve_dir(&d).is_empty());
}

#[test]
fn resolve_defined_trait_ok() {
    let d = TempTree::new();
    d.write("common/traits/00_traits.txt", "brave = { }\ncraven = { }\n");
    d.write(
        "common/scripted_effects/00_e.txt",
        "give_it = {\n\tadd_trait = brave\n\tremove_trait = craven\n}\n",
    );
    assert!(resolve_dir(&d).is_empty());
}

// ── ports of project_test.go ─────────────────────────────────────────────────

#[test]
fn project_initial_diags() {
    let d = TempTree::new();
    d.write("common/traits/00_t.txt", "brave = { }\n");
    d.write(
        "common/scripted_effects/00_e.txt",
        "e = { add_trait = brave }\n",
    );
    let p = project(&d);
    assert!(p.diags().is_empty(), "{:?}", p.diags());
    assert_eq!(p.table().count(pdxl_ck3::kinds::TRAIT), 1);
}

#[test]
fn project_incremental_update() {
    let d = TempTree::new();
    d.write("common/traits/00_t.txt", "brave = { }\n");
    d.write(
        "common/scripted_effects/00_e.txt",
        "e = { add_trait = brave }\n",
    );
    let trait_path: PathBuf = d.child("common/traits/00_t.txt");

    let mut p = project(&d);
    assert!(p.diags().is_empty());

    // Rename the trait on disk; update only the trait file. The effect file is
    // not re-read, but its reference must now be unresolved.
    std::fs::write(&trait_path, "bold = { }\n").unwrap();
    p.update(&trait_path).unwrap();

    assert_eq!(p.diags().len(), 1, "{:?}", p.diags());
    assert_eq!(p.table().count(pdxl_ck3::kinds::TRAIT), 1);
    assert!(p.table().lookup(pdxl_ck3::kinds::TRAIT, "bold").is_some());
}

#[test]
fn project_update_source() {
    let d = TempTree::new();
    d.write("common/traits/00_t.txt", "brave = { }\n");
    d.write(
        "common/scripted_effects/00_e.txt",
        "e = { add_trait = brave }\n",
    );
    let trait_path = d.child("common/traits/00_t.txt");

    let mut p = project(&d);
    assert!(p.diags().is_empty());

    // In-memory edit (disk unchanged): the trait is renamed in the buffer.
    p.update_source(&trait_path, b"bold = { }\n".to_vec())
        .unwrap();
    assert_eq!(p.diags().len(), 1);

    // Disk still has brave; a disk-based update reverts the buffer view.
    p.update(&trait_path).unwrap();
    assert!(p.diags().is_empty());
}

#[test]
fn ref_diag_carries_offsets() {
    let d = TempTree::new();
    d.write(
        "common/scripted_effects/a.txt",
        "e = { add_trait = nope }\n",
    );
    let p = project(&d);
    let diags = p.diags();
    assert_eq!(diags.len(), 1);
    assert!(!diags[0].file.is_empty());
    assert!(diags[0].end > diags[0].start);
}

#[test]
fn project_file_diags() {
    let d = TempTree::new();
    d.write(
        "common/scripted_effects/a.txt",
        "e = { add_trait = missing_a }\n",
    );
    d.write(
        "common/scripted_effects/b.txt",
        "e = { add_trait = missing_b }\n",
    );
    let p = project(&d);
    assert_eq!(p.diags().len(), 2);
    let fd = p.file_diags(&d.child("common/scripted_effects/a.txt"));
    assert_eq!(fd.len(), 1);
    assert!(fd[0].msg.contains("missing_a"));
}

// ── beyond the Go suite ──────────────────────────────────────────────────────

#[test]
fn untracked_file_is_an_error() {
    let d = TempTree::new();
    d.write("common/traits/00_t.txt", "brave = { }\n");
    let mut p = project(&d);
    assert!(p.update(&d.child("common/traits/nope.txt")).is_err());
    assert!(p.facts_at(&d.child("elsewhere.txt")).is_none());
}

#[test]
fn references_and_rel_to_full() {
    let d = TempTree::new();
    d.write("common/traits/00_t.txt", "brave = { }\n");
    d.write(
        "common/scripted_effects/a.txt",
        "e = { add_trait = brave }\n",
    );
    d.write(
        "common/scripted_effects/b.txt",
        "e = { has_trait = brave }\n",
    );
    let p = project(&d);
    assert_eq!(p.references(pdxl_ck3::kinds::TRAIT, "brave").len(), 2);
    let full = p.rel_to_full("common/traits/00_t.txt").expect("tracked");
    assert!(full.ends_with("common/traits/00_t.txt"));
}

#[test]
fn incremental_equals_fresh_analysis() {
    // The invariant the whole design rests on: after any single-file edit,
    // (table, diags) must equal what a fresh full analysis would compute.
    let d = TempTree::new();
    d.write("common/traits/00_t.txt", "brave = { group = g }\n");
    d.write("common/traits/01_t.txt", "brave = { }\n"); // duplicate
    d.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave has_trait = g add_trait = gone }\n",
    );
    let trait_path = d.child("common/traits/00_t.txt");

    let mut incremental = project(&d);
    std::fs::write(&trait_path, "bold = { }\n").unwrap();
    incremental.update(&trait_path).unwrap();

    let fresh = project(&d); // full re-scan of the edited corpus

    assert_eq!(incremental.diags(), fresh.diags());
    assert_eq!(incremental.table().total(), fresh.table().total());
    assert_eq!(
        incremental.table().duplicates.len(),
        fresh.table().duplicates.len()
    );
    for kind in pdxl_ck3::schema().kinds().iter().copied() {
        assert_eq!(
            incremental.table().count(kind),
            fresh.table().count(kind),
            "{kind:?}"
        );
    }
}

#[test]
fn analyze_with_cache_matches_uncached() {
    // The AST cache must be invisible to results: cold, populate, and warm
    // runs all produce the same table and diagnostics.
    let d = TempTree::new();
    d.write("common/traits/00_t.txt", "brave = { }\n");
    d.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave add_trait = nope }\n",
    );
    let fs = fileset(&d);
    let schema = pdxl_ck3::schema();

    let (cold_table, cold_diags) = analyze(&fs, &schema).unwrap();
    let store = pdxl_cache::Store::new(d.child(".cache"), 16).unwrap();
    let (_, populate_diags) = pdxl_project::analyze_with(&fs, &schema, Some(&store)).unwrap();
    let (warm_table, warm_diags) = pdxl_project::analyze_with(&fs, &schema, Some(&store)).unwrap();

    assert_eq!(cold_diags, populate_diags);
    assert_eq!(cold_diags, warm_diags);
    assert_eq!(cold_table.total(), warm_table.total());
    for kind in pdxl_ck3::schema().kinds().iter().copied() {
        assert_eq!(cold_table.count(kind), warm_table.count(kind), "{kind:?}");
    }
}

#[test]
fn localization_keys_resolve_event_text_refs() {
    let t = TempTree::new();
    t.write(
        "localization/english/my_events_l_english.yml",
        "\u{FEFF}l_english:\n my.1.t: \"A Title\"\n my.1.a: \"An option\"\n",
    );
    t.write(
        "events/my.txt",
        "namespace = my\nmy.1 = {\n\ttitle = my.1.t\n\toption = { name = my.1.a }\n\toption = { name = my.1.missing }\n}\n",
    );
    // The fileset() helper doesn't opt into localization; build one that does.
    let mut fs = pdxl_fileset::FileSet::new();
    fs.set_localization_language(pdxl_project::DEFAULT_LOC_LANGUAGE);
    fs.add(&t.path, pdxl_fileset::FileKind::Mod).unwrap();
    let (table, diags) = analyze(&fs, &pdxl_ck3::schema()).expect("analyze");
    assert_eq!(table.count(pdxl_analysis::LOC_KEY), 2);
    let missing: Vec<&str> = diags.iter().map(|d| d.msg.as_str()).collect();
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert!(missing[0].contains("unknown loc_key \"my.1.missing\""));

    // Definitions carry the yml rel path + key offsets (goto-def target).
    let sym = table
        .lookup(pdxl_analysis::LOC_KEY, "my.1.t")
        .expect("loc symbol");
    assert_eq!(&*sym.file, "localization/english/my_events_l_english.yml");
}

// ── province definitions from map_data/definition.csv ────────────────────────

#[test]
fn province_csv_defs_resolve_history_refs() {
    let d = TempTree::new();
    d.write(
        "map_data/definition.csv",
        "0;0;0;0;x;x;\n1;42;3;128;VESTFIRDIR;x;\n#2;84;6;1;COMMENTED;x;\n",
    );
    d.write(
        "history/provinces/00.txt",
        "1 = { holding = none }\n2 = { holding = none }\n",
    );
    let mut fs = FileSet::new();
    fs.set_include_map_data(true);
    fs.add(&d.path, FileKind::Mod).expect("scan");
    let (table, diags) = analyze(&fs, &pdxl_ck3::schema()).expect("analyze");
    assert_eq!(table.count(pdxl_ck3::kinds::PROVINCE), 2, "ids 0 and 1");
    // Province 2 is a commented-out CSV row, so its history ref dangles.
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert!(diags[0].msg.contains("unknown province \"2\""));
}

#[test]
fn province_csv_ignored_without_opt_in() {
    let d = TempTree::new();
    d.write("map_data/definition.csv", "1;42;3;128;VESTFIRDIR;x;\n");
    let (table, _) = analyze(&fileset(&d), &pdxl_ck3::schema()).expect("analyze");
    assert_eq!(table.count(pdxl_ck3::kinds::PROVINCE), 0);
}
