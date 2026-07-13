//! Differential parity test: Rust whole-project analysis vs the Go oracle.
//!
//! Each scenario builds a temp project tree; both sides analyze the same roots
//! and the canonical dumps (symbol counts by kind, duplicates in merge order,
//! unresolved diagnostics in walk order — full file/offset/loc/msg) must match
//! byte-for-byte. Self-skips if `go` is unavailable.

use std::path::{Path, PathBuf};
use std::process::Command;

use pdxl_fileset::{FileKind, FileSet};
use pdxl_parity::dump_project;
use pdxl_testutil::{TempTree, go_available};

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

fn kind_str(k: FileKind) -> &'static str {
    k.as_str()
}

fn rust_dump(roots: &[(&Path, FileKind)]) -> String {
    let mut fs = FileSet::new();
    for (root, kind) in roots {
        fs.add(root, *kind).expect("add root");
    }
    let (table, diags) = pdxl_project::analyze(&fs, &pdxl_ck3::schema()).expect("analyze");
    dump_project(&table, &diags)
}

fn go_dump(repo: &Path, roots: &[(&Path, FileKind)]) -> String {
    let mut args: Vec<String> = vec!["run".into(), "./tools/projectdump".into()];
    for (root, kind) in roots {
        args.push("--root".into());
        args.push(format!("{}:{}", root.display(), kind_str(*kind)));
    }
    let out = Command::new("go")
        .current_dir(repo)
        .args(&args)
        .output()
        .expect("spawn go projectdump");
    assert!(
        out.status.success(),
        "go projectdump failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8")
}

fn assert_parity(name: &str, repo: &Path, roots: &[(&Path, FileKind)]) {
    let rust = rust_dump(roots);
    let go = go_dump(repo, roots);
    assert_eq!(rust, go, "project dump mismatch in scenario '{name}'");
}

#[test]
fn project_differential() {
    let repo = repo_root();
    if !go_available() {
        eprintln!("warning: `go` not found — skipping project differential parity test");
        return;
    }

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
        "real_oa = { }\nmy_oa = {\n\tevents = { t.1 t.9 }\n\tfirst_valid = { t.1 }\n\trandom_events = { 100 = t.1  50 = t.8  chance_to_happen = 10  0 = 0 }\n\ton_actions = { real_oa missing_oa }\n}\n",
    );
    assert_parity("resolution", &repo, &[(&a.path, FileKind::Mod)]);

    // --- duplicates: within a root and their stable "first" ordering ---
    let d = TempTree::new();
    d.write("common/traits/00.txt", "brave = { }\ncraven = { }\n");
    d.write("common/traits/01.txt", "brave = { }\n");
    d.write("common/traits/02.txt", "brave = { }\ncraven = { }\n");
    assert_parity("duplicates", &repo, &[(&d.path, FileKind::Mod)]);

    // --- overlay: shadowing removes a definition AND its aliases; a mod file
    //     replacing a vanilla file must not create duplicates ---
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
    md.write("common/traits/00.txt", "craven = { }\n"); // shadows vanilla 00
    assert_parity(
        "overlay",
        &repo,
        &[(&van.path, FileKind::Vanilla), (&md.path, FileKind::Mod)],
    );

    // --- skip rules end to end: macros/scopes never produce diagnostics ---
    let s = TempTree::new();
    s.write("common/traits/00.txt", "brave = { }\n");
    s.write(
        "common/scripted_effects/e.txt",
        "e = {\n\tadd_trait = $TRAIT$\n\thas_trait = scope:x\n\thas_trait = prev\n\tadd_trait = education_$E$_5\n}\n",
    );
    assert_parity("skips", &repo, &[(&s.path, FileKind::Mod)]);

    eprintln!("project parity: 4 scenarios byte-identical to Go oracle");
}
