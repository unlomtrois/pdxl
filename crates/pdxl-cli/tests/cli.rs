//! CLI snapshot tests.
//!
//! `lex` and `parse` are still compared byte-for-byte against the Go binary —
//! those layers remain at exact parity. `check` output depends on the analysis
//! schema, whose Go oracle retired with the landed-titles addition
//! (`ANALYSIS_VERSION` 2: the counts table gained a `title` row Go doesn't
//! print), so its scenarios are pinned by golden files instead; regenerate
//! deliberately with `UPDATE_GOLDENS=1 cargo test -p pdxl-cli --test cli`.
//!
//! Go-comparing tests self-skip if `go` is unavailable.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use pdxl_testutil::TempTree;

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

/// Runs the Rust binary, returning (stdout with roots normalized, success).
fn run_check(args: &[&str], roots: &[(&str, &TempTree)]) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_pdxl"))
        .args(args)
        .output()
        .expect("spawn pdxl");
    let mut stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    for (placeholder, tree) in roots {
        stdout = stdout.replace(&tree.path.to_string_lossy().into_owned(), placeholder);
    }
    (stdout, out.status.success())
}

fn check_golden(name: &str, dump: &str) {
    let dir = repo_root().join("crates/pdxl-cli/tests/goldens");
    let path = dir.join(format!("{name}.golden"));
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, dump).unwrap();
        return;
    }
    let golden = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {path:?} — run with UPDATE_GOLDENS=1"));
    assert_eq!(dump, golden, "check output changed for '{name}'");
}

#[test]
fn check_matches_goldens() {
    // A project with duplicates, aliases, resolved and unresolved refs, plus a
    // .mod descriptor exercising replace_path and Unix-absolute paths — and,
    // post-parity, a landed-titles tree with title: references.
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
        "mymod/common/landed_titles/t4n.txt",
        "e_empire = { k_kingdom = { d_duchy = { } } }\n",
    );
    md.write(
        "mymod/common/scripted_effects/e.txt",
        "e = {\n\tadd_trait = brave\n\thas_trait = personality\n\ttrigger_event = t.1\n\tadd_trait = missing\n\thas_title = title:k_kingdom\n\thas_title = title:k_x\n}\n",
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
            md.child("mymod").display()
        ),
    )
    .unwrap();

    let van_s = van.path.to_string_lossy().into_owned();
    let mod_s = mod_file.to_string_lossy().into_owned();
    let roots: &[(&str, &TempTree)] = &[("<van>", &van), ("<mod>", &md)];

    // Whole-project report: k_x was replaced away, so title:k_x is unresolved.
    let (stdout, ok) = run_check(
        &["check", "--game", &van_s, "--mod", &mod_s, "--no-cache"],
        roots,
    );
    assert!(!ok, "unresolved refs must exit non-zero");
    check_golden("check_project", &stdout);

    // Single-file report.
    let effect = md.child("mymod/common/scripted_effects/e.txt");
    let (stdout, ok) = run_check(
        &[
            "check",
            effect.to_string_lossy().as_ref(),
            "--game",
            &van_s,
            "--mod",
            &mod_s,
            "--no-cache",
        ],
        roots,
    );
    assert!(!ok);
    check_golden("check_file", &stdout);

    // Clean project (no unresolved → exit zero).
    let clean = TempTree::new();
    clean.write("common/traits/00.txt", "brave = { }\n");
    clean.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave }\n",
    );
    let (stdout, ok) = run_check(
        &[
            "check",
            "--mod",
            clean.path.to_string_lossy().as_ref(),
            "--no-cache",
        ],
        &[("<mod>", &clean)],
    );
    assert!(ok, "clean project must exit zero:\n{stdout}");
    check_golden("check_clean", &stdout);
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

#[test]
fn lsp_handshake_declares_capabilities_at_the_right_nesting() {
    // Field-tested the hard way: lsp-server's `initialize` helper wraps its
    // argument in {"capabilities": ...}; passing a pre-wrapped InitializeResult
    // double-nests everything, and vscode-languageclient then sees no declared
    // sync/providers and never sends a single textDocument notification. This
    // test speaks the real wire protocol to the real binary and asserts the
    // exact JSON shape a spec-respecting client reads.
    use std::io::{BufRead, BufReader, Read, Write};

    let mut proc = Command::new(env!("CARGO_BIN_EXE_pdxl"))
        .args(["lsp", "--log-level", "error"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn pdxl lsp");

    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "rootUri": "file:///tmp", "capabilities": {} }
    })
    .to_string();
    let stdin = proc.stdin.as_mut().unwrap();
    write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
    stdin.flush().unwrap();

    let mut reader = BufReader::new(proc.stdout.as_mut().unwrap());
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length: ") {
            content_length = v.parse().unwrap();
        }
    }
    let mut buf = vec![0u8; content_length];
    reader.read_exact(&mut buf).unwrap();
    let resp: serde_json::Value = serde_json::from_slice(&buf).unwrap();

    let caps = &resp["result"]["capabilities"];
    assert_eq!(
        caps["textDocumentSync"], 1,
        "full sync must be declared: {resp}"
    );
    assert_eq!(caps["definitionProvider"], true);
    assert_eq!(caps["referencesProvider"], true);
    assert_eq!(caps["hoverProvider"], true);
    assert_eq!(caps["documentSymbolProvider"], true);
    assert_eq!(resp["result"]["serverInfo"]["name"], "pdxl");
    // The double-nesting bug's signature: capabilities inside capabilities.
    assert!(
        caps.get("capabilities").is_none(),
        "double-wrapped InitializeResult: {resp}"
    );

    let _ = proc.kill();
    let _ = proc.wait(); // reap the child (clippy: zombie_processes)
}

// ── fmt ─────────────────────────────────────────────────────────────────────

#[test]
fn fmt_prints_formatted_output_to_stdout() {
    let t = TempTree::new();
    t.write("a.txt", "a = { b = 1 c = { d = 2 } }\n");
    let file = t.child("a.txt");
    let out = run_rust(&repo_root(), &["fmt", &file.to_string_lossy()]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "a = {\n\tb = 1\n\tc = {\n\t\td = 2\n\t}\n}\n"
    );
}

#[test]
fn fmt_check_exits_nonzero_and_lists_unformatted() {
    let t = TempTree::new();
    t.write("dense.txt", "a = { b = 1 }\n");
    t.write("clean.txt", "a = {\n\tb = 1\n}\n");
    let dense = t.child("dense.txt");
    let clean = t.child("clean.txt");
    let out = run_rust(
        &repo_root(),
        &[
            "fmt",
            "--check",
            &dense.to_string_lossy(),
            &clean.to_string_lossy(),
        ],
    );
    assert!(!out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("dense.txt") && !stdout.contains("clean.txt"));

    let out = run_rust(&repo_root(), &["fmt", "--check", &clean.to_string_lossy()]);
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
}

#[test]
fn fmt_write_rewrites_in_place_and_parse_errors_refuse() {
    let t = TempTree::new();
    t.write("a.txt", "a = { b = 1 }  # keep me\n");
    t.write("broken.txt", "a = {\n");
    let a = t.child("a.txt");
    let broken = t.child("broken.txt");
    let out = run_rust(
        &repo_root(),
        &[
            "fmt",
            "--write",
            &a.to_string_lossy(),
            &broken.to_string_lossy(),
        ],
    );
    // broken.txt makes the run fail, but a.txt is still rewritten.
    assert!(!out.status.success());
    assert_eq!(
        std::fs::read_to_string(&a).unwrap(),
        "a = {\n\tb = 1\n} # keep me\n"
    );
    assert_eq!(std::fs::read_to_string(&broken).unwrap(), "a = {\n");
    assert!(String::from_utf8_lossy(&out.stderr).contains("parse errors"));
}
