//! Whole-project analysis regression tests — golden snapshots.
//!
//! **The Go oracle is retired for the analysis layer** as of the landed-titles
//! schema (`ANALYSIS_VERSION` 2): `SymbolKind::Title` changes the counts shape,
//! so byte-comparison against `tools/projectdump` is no longer meaningful.
//! Each scenario builds a temp project tree, analyzes it, and compares the
//! canonical dump (with the temp root normalized to `<root>`) against a golden.
//!
//! To accept an intentional behavior change, regenerate with:
//! `UPDATE_GOLDENS=1 cargo test -p pdxl-parity --test project`

use std::path::PathBuf;

use pdxl_fileset::{FileKind, FileSet};
use pdxl_parity::dump_project;
use pdxl_testutil::TempTree;

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

/// Analyzes `roots` and returns the dump with every root path normalized.
fn scenario_dump(roots: &[(&TempTree, FileKind)]) -> String {
    let mut fs = FileSet::new();
    for (tree, kind) in roots {
        fs.add(&tree.path, *kind).expect("add root");
    }
    let schema = pdxl_ck3::schema();
    let (table, diags) = pdxl_project::analyze(&fs, &schema).expect("analyze");
    let mut dump = dump_project(&table, &diags, schema.kinds());
    for (i, (tree, _)) in roots.iter().enumerate() {
        dump = dump.replace(
            &tree.path.to_string_lossy().into_owned(),
            &format!("<root{i}>"),
        );
    }
    dump
}

fn check_golden(name: &str, dump: &str) {
    let goldens_dir = repo_root().join("rust/crates/pdxl-parity/testdata/goldens/project");
    let golden_path = goldens_dir.join(format!("{name}.golden"));
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(&goldens_dir).unwrap();
        std::fs::write(&golden_path, dump).unwrap();
        eprintln!("regenerated {golden_path:?}");
        return;
    }
    let golden = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|_| panic!("missing golden {golden_path:?} — run with UPDATE_GOLDENS=1"));
    assert_eq!(
        dump, golden,
        "project dump changed for scenario '{name}'. If intentional, regenerate:\n\
         UPDATE_GOLDENS=1 cargo test -p pdxl-parity --test project"
    );
}

#[test]
fn project_scenarios_match_goldens() {
    // --- resolution across files and kinds; aliases; quoted refs ---
    let a = TempTree::new();
    a.write(
        "common/traits/00.txt",
        "brave = { group = personality_brave }\nbastard = { }\n",
    );
    a.write(
        "common/scripted_effects/e.txt",
        "give = {\n\thas_trait = personality_brave\n\tadd_trait = \"bastard\"\n\tadd_trait = nope\n}\n",
    );
    a.write("events/ev.txt", "namespace = t\nt.1 = { }\n");
    a.write(
        "common/on_action/oa.txt",
        "real_oa = { }\nmy_oa = {\n\tevents = { t.1 t.9 }\n\tfirst_valid = { t.1 }\n\trandom_events = { 100 = t.1  50 = t.8  chance_to_happen = 10  0 = 0 }\n\ton_actions = { real_oa missing_oa }\n\tfirst_valid_on_action = { real_oa }\n\trandom_on_actions = { 100 = real_oa  50 = gone_oa  25 = 0 }\n\tfallback = real_oa\n\teffect = {\n\t\ttrigger_event = { on_action = real_oa }\n\t\ttrigger_event = { on_action = phantom_oa }\n\t}\n}\n",
    );
    check_golden("resolution", &scenario_dump(&[(&a, FileKind::Mod)]));

    // --- duplicates: within a root and their stable "first" ordering ---
    let d = TempTree::new();
    d.write("common/traits/00.txt", "brave = { }\ncraven = { }\n");
    d.write("common/traits/01.txt", "brave = { }\n");
    d.write("common/traits/02.txt", "brave = { }\ncraven = { }\n");
    check_golden("duplicates", &scenario_dump(&[(&d, FileKind::Mod)]));

    // --- overlay: shadowing removes definitions AND aliases; no false dups ---
    let van = TempTree::new();
    let md = TempTree::new();
    van.write(
        "common/traits/00.txt",
        "brave = { group = personality }\ncraven = { }\n",
    );
    van.write("common/traits/01_dup.txt", "brave = { }\n");
    van.write(
        "common/scripted_effects/e.txt",
        "e = {\n add_trait = brave\n has_trait = personality\n add_trait = missing\n}\n",
    );
    md.write("common/traits/00.txt", "craven = { }\n");
    check_golden(
        "overlay",
        &scenario_dump(&[(&van, FileKind::Vanilla), (&md, FileKind::Mod)]),
    );

    // --- skip rules end to end ---
    let s = TempTree::new();
    s.write("common/traits/00.txt", "brave = { }\n");
    s.write(
        "common/scripted_effects/e.txt",
        "e = {\n\tadd_trait = $TRAIT$\n\thas_trait = scope:x\n\thas_trait = prev\n\tadd_trait = education_$E$_5\n}\n",
    );
    check_golden("skips", &scenario_dump(&[(&s, FileKind::Mod)]));

    // --- titles: tree defs + title: refs across the project (post-parity) ---
    let t = TempTree::new();
    t.write(
        "common/landed_titles/00.txt",
        "e_empire = {\n\tcolor = { 1 2 3 }\n\tk_kingdom = {\n\t\td_duchy = {\n\t\t\tc_shore = { b_port = { province = 1 } }\n\t\t}\n\t}\n}\nh_hegemony = { }\n",
    );
    t.write(
        "common/scripted_effects/e.txt",
        "e = {\n\thas_title = title:e_empire\n\tx = title:d_duchy.holder\n\ttitle:h_hegemony = { set_x = 1 }\n\thas_title = title:k_gone\n}\n",
    );
    check_golden("titles", &scenario_dump(&[(&t, FileKind::Mod)]));

    eprintln!("project goldens: 5 scenarios match");
}
