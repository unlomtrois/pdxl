//! CLI snapshot parity: the Rust `pdxl` binary vs the Go `cmd/pdxl` on the
//! same inputs — stdout compared byte-for-byte, exit codes compared exactly.
//! Covers `lex` (all flag combinations), `parse` (flat + `--tree`), and
//! `check` (project report incl. duplicates/unresolved and the single-file
//! form; run with `--no-cache` from the repo root so Go's `pdxl.toml` — which
//! equals the built-in defaults — keeps both sides identically configured).
//!
//! Self-skips if `go` is unavailable.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use pdxl_testutil::{TempTree, go_available};

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

fn run_rust(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdxl"))
        .current_dir(repo)
        .args(args)
        .output()
        .expect("spawn rust pdxl")
}

fn run_go(repo: &Path, args: &[&str]) -> Output {
    Command::new("go")
        .current_dir(repo)
        .args(["run", "./cmd/pdxl"])
        .args(args)
        .output()
        .expect("spawn go pdxl")
}

/// Asserts identical stdout and success-vs-failure exit status.
fn assert_cli_parity(repo: &Path, args: &[&str]) {
    let rust = run_rust(repo, args);
    let go = run_go(repo, args);
    assert_eq!(
        String::from_utf8_lossy(&rust.stdout),
        String::from_utf8_lossy(&go.stdout),
        "stdout mismatch for {args:?}"
    );
    assert_eq!(
        rust.status.success(),
        go.status.success(),
        "exit status mismatch for {args:?} (rust {:?}, go {:?})",
        rust.status,
        go.status
    );
}

#[test]
fn lex_and_parse_match_go() {
    let repo = repo_root();
    if !go_available() {
        eprintln!("warning: `go` not found — skipping CLI parity test");
        return;
    }

    let fixtures = [
        "testdata/advance.txt",
        "testdata/subject_type.txt",
        "testdata/ck3/scripted_trigger_macro.txt",
        "testdata/lint/advance_for_lint.txt", // malformed: diagnostics path
    ];
    for f in fixtures {
        assert_cli_parity(&repo, &["lex", f]);
        assert_cli_parity(&repo, &["lex", f, "--tags"]);
        assert_cli_parity(&repo, &["lex", f, "--tags", "--show-pos"]);
        assert_cli_parity(&repo, &["parse", f]);
        assert_cli_parity(&repo, &["parse", f, "--tree"]);
    }
}

#[test]
fn check_matches_go() {
    let repo = repo_root();
    if !go_available() {
        eprintln!("warning: `go` not found — skipping check parity test");
        return;
    }

    // A project with duplicates, aliases, resolved and unresolved refs, plus a
    // .mod descriptor exercising replace_path and the (fixed) absolute path.
    let van = TempTree::new();
    van.write(
        "common/traits/00.txt",
        "brave = { group = personality }\ncraven = { }\n",
    );
    van.write("common/traits/01.txt", "brave = { }\n"); // duplicate
    van.write("common/landed_titles/base.txt", "k_x = { }\n"); // replaced away
    van.write("events/ev.txt", "namespace = t\nt.1 = { }\n");
    let md = TempTree::new();
    md.write(
        "mymod/common/scripted_effects/e.txt",
        "e = {\n\tadd_trait = brave\n\thas_trait = personality\n\ttrigger_event = t.1\n\tadd_trait = missing\n}\n",
    );
    md.write(
        "mymod/common/on_action/oa.txt",
        "oa = { events = { t.1 t.9 } }\n",
    );
    let mod_file = md.child("bench.mod");
    std::fs::write(
        &mod_file,
        format!(
            "name=\"Bench\"\npath=\"{}\"\nreplace_path=\"common/landed_titles\"\n",
            md.child("mymod").display() // absolute Unix path: the M7 fix
        ),
    )
    .unwrap();

    let van_s = van.path.to_string_lossy();
    let mod_s = mod_file.to_string_lossy();

    // Whole-project report (has unresolved → both must exit non-zero).
    assert_cli_parity(
        &repo,
        &["check", "--game", &van_s, "--mod", &mod_s, "--no-cache"],
    );

    // Single-file report.
    let effect = md.child("mymod/common/scripted_effects/e.txt");
    assert_cli_parity(
        &repo,
        &[
            "check",
            effect.to_string_lossy().as_ref(),
            "--game",
            &van_s,
            "--mod",
            &mod_s,
            "--no-cache",
        ],
    );

    // Clean project (no unresolved → both must exit zero).
    let clean = TempTree::new();
    clean.write("common/traits/00.txt", "brave = { }\n");
    clean.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave }\n",
    );
    assert_cli_parity(
        &repo,
        &[
            "check",
            "--mod",
            clean.path.to_string_lossy().as_ref(),
            "--no-cache",
        ],
    );
}

#[test]
fn check_runs_are_deterministic() {
    // Repeated runs over an unchanged tree must produce identical output
    // (HashMap iteration must never leak into any report ordering).
    let proj = TempTree::new();
    proj.write("common/traits/00.txt", "brave = { }\n");
    proj.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave add_trait = nope }\n",
    );
    let proj_s = proj.path.to_string_lossy();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_pdxl"))
            .args(["check", "--mod", &proj_s, "--no-cache"])
            .output()
            .expect("spawn")
    };
    let a = run();
    let b = run();
    assert_eq!(a.stdout, b.stdout);
    assert!(!a.status.success(), "unresolved ref → non-zero exit");
}
