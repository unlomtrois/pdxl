//! Server behavior tests, mirroring the Go `internal/lsp/server_test.go`
//! pattern: build a project over a temp tree, drive the handlers directly, and
//! inspect the messages that would have gone to the client (captured on a
//! channel instead of Go's captured notify function).

use std::path::PathBuf;

use crossbeam_channel::{Receiver, unbounded};
use lsp_server::Message;
use lsp_types::{Position, PublishDiagnosticsParams, Url};
use pdxl_lsp::{Event, ServerState, build_project};
use pdxl_testutil::TempTree;

/// Builds a ready server over `mod_root` plus captured-output receiver.
fn server_over(mod_root: &TempTree) -> (ServerState, Receiver<Message>) {
    let (tx, rx) = unbounded();
    let mut server = ServerState::new(Some(mod_root.path.clone()), tx);
    let project = build_project(None, Some(&mod_root.path.to_string_lossy()));
    server.project_ready(project.map(Box::new));
    assert!(server.is_ready());
    (server, rx)
}

/// Drains all captured publishDiagnostics notifications into (path, count).
fn drain_publishes(rx: &Receiver<Message>) -> Vec<(PathBuf, usize)> {
    let mut out = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let Message::Notification(n) = msg
            && n.method == "textDocument/publishDiagnostics"
        {
            let p: PublishDiagnosticsParams = serde_json::from_value(n.params).unwrap();
            let path = p.uri.to_file_path().unwrap();
            out.push((path, p.diagnostics.len()));
        }
    }
    out
}

fn uri_for(t: &TempTree, rel: &str) -> Url {
    Url::from_file_path(t.child(rel)).unwrap()
}

#[test]
fn initial_diagnostics_published_for_mod_files() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave add_trait = missing }\n",
    );
    let (_server, rx) = server_over(&t);

    let published = drain_publishes(&rx);
    assert_eq!(published.len(), 1, "one file has diagnostics");
    assert!(published[0].0.ends_with("common/scripted_effects/e.txt"));
    assert_eq!(published[0].1, 1, "exactly the unresolved 'missing'");
}

#[test]
fn edit_fixes_reference_and_clears_diagnostics() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = wrong }\n",
    );
    let (mut server, rx) = server_over(&t);
    assert_eq!(drain_publishes(&rx).len(), 1, "baseline: one flagged file");

    // Open the effect file and fix the reference in the buffer (disk untouched).
    let uri = uri_for(&t, "common/scripted_effects/e.txt");
    server.did_open(uri.clone(), "e = { add_trait = wrong }".to_string());
    drain_publishes(&rx);

    let (path, generation) = server
        .did_change(uri.clone(), "e = { add_trait = brave }".to_string())
        .unwrap();
    // Fire the debounce directly (tests don't sleep).
    server.debounce_fired(&path, generation);

    let published = drain_publishes(&rx);
    // The previously flagged file must be explicitly CLEARED (empty publish).
    assert!(
        published
            .iter()
            .any(|(p, n)| p.ends_with("e.txt") && *n == 0),
        "expected a clearing publish, got {published:?}"
    );
}

#[test]
fn stale_debounce_generation_is_ignored() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave }\n",
    );
    let (mut server, rx) = server_over(&t);
    drain_publishes(&rx);

    let uri = uri_for(&t, "common/scripted_effects/e.txt");
    let (path, gen1) = server
        .did_change(uri.clone(), "e = { add_trait = broken_one }".to_string())
        .unwrap();
    let (_, gen2) = server
        .did_change(uri.clone(), "e = { add_trait = brave }".to_string())
        .unwrap();
    assert!(gen2 > gen1);

    // The stale timer fires first: must do nothing.
    server.debounce_fired(&path, gen1);
    assert!(drain_publishes(&rx).is_empty(), "stale generation acted");

    // The current one analyzes the fixed buffer: still no diagnostics.
    server.debounce_fired(&path, gen2);
    let published = drain_publishes(&rx);
    assert!(
        published.iter().all(|(_, n)| *n == 0),
        "fixed buffer must not flag: {published:?}"
    );
}

#[test]
fn did_close_reverts_to_disk_state() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave }\n",
    );
    let (mut server, rx) = server_over(&t);
    drain_publishes(&rx);

    // Break the reference in the buffer only.
    let uri = uri_for(&t, "common/scripted_effects/e.txt");
    server.did_open(uri.clone(), "e = { add_trait = nope }".to_string());
    let flagged = drain_publishes(&rx);
    assert!(
        flagged.iter().any(|(_, n)| *n == 1),
        "buffer breakage flags"
    );

    // Close: disk still has the good text, so diagnostics clear.
    server.did_close(uri);
    let published = drain_publishes(&rx);
    assert!(
        published
            .iter()
            .any(|(p, n)| p.ends_with("e.txt") && *n == 0),
        "close must revert to on-disk (clean) state: {published:?}"
    );
}

#[test]
fn vanilla_files_are_never_flagged() {
    // Vanilla defines nothing the mod file needs; the vanilla file itself has
    // a broken reference — it must feed the table but never be published.
    let vanilla = TempTree::new();
    vanilla.write("common/traits/00.txt", "brave = { }\n");
    vanilla.write(
        "common/scripted_effects/v.txt",
        "v = { add_trait = broken_in_vanilla }\n",
    );
    let modroot = TempTree::new();
    modroot.write(
        "common/scripted_effects/m.txt",
        "m = { add_trait = also_broken }\n",
    );

    let (tx, rx) = unbounded();
    let mut server = ServerState::new(Some(modroot.path.clone()), tx);
    let project = build_project(
        Some(&vanilla.path.to_string_lossy()),
        Some(&modroot.path.to_string_lossy()),
    );
    server.project_ready(project.map(Box::new));

    let published = drain_publishes(&rx);
    assert_eq!(published.len(), 1, "only the mod file: {published:?}");
    assert!(published[0].0.ends_with("m.txt"));
}

#[test]
fn goto_definition_resolves_across_files() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "# header\nbrave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave }\n",
    );
    let (server, _rx) = server_over(&t);

    // Cursor inside "brave" on line 0: `e = { add_trait = brave }`
    //                                   0123456789012345678
    let uri = uri_for(&t, "common/scripted_effects/e.txt");
    let loc = server
        .definition(
            &uri,
            Position {
                line: 0,
                character: 19,
            },
        )
        .expect("definition found");

    let def_path = loc.uri.to_file_path().unwrap();
    assert!(def_path.ends_with("common/traits/00.txt"));
    // Definition is on line 1 (after the comment), name starts at column 0.
    assert_eq!(
        loc.range.start,
        Position {
            line: 1,
            character: 0
        }
    );
    assert_eq!(
        loc.range.end,
        Position {
            line: 1,
            character: 5
        }
    ); // "brave"
}

#[test]
fn definition_no_result_branches_return_none() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = gone }\n",
    );
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/e.txt");

    // Cursor not on any reference (the key "e").
    assert!(
        server
            .definition(
                &uri,
                Position {
                    line: 0,
                    character: 0
                }
            )
            .is_none()
    );
    // Cursor on an UNRESOLVED reference: no definition to jump to.
    assert!(
        server
            .definition(
                &uri,
                Position {
                    line: 0,
                    character: 19
                }
            )
            .is_none()
    );
    // Untracked file.
    let stranger = Url::from_file_path(t.child("not/tracked.txt")).unwrap();
    assert!(
        server
            .definition(
                &stranger,
                Position {
                    line: 0,
                    character: 0
                }
            )
            .is_none()
    );
}

#[test]
fn docs_opened_before_project_ready_are_analyzed_after() {
    // Go parity: didOpen during the async build must not be lost.
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave }\n",
    );

    let (tx, rx) = unbounded();
    let mut server = ServerState::new(Some(t.path.clone()), tx);
    // Document opens (with a broken buffer) BEFORE the project is ready.
    let uri = uri_for(&t, "common/scripted_effects/e.txt");
    server.did_open(uri, "e = { add_trait = broken }".to_string());
    assert!(
        drain_publishes(&rx).is_empty(),
        "nothing published pre-build"
    );

    // Build completes: the open buffer must override disk in the analysis.
    let project = build_project(None, Some(&t.path.to_string_lossy()));
    server.project_ready(project.map(Box::new));
    let published = drain_publishes(&rx);
    assert!(
        published
            .iter()
            .any(|(p, n)| p.ends_with("e.txt") && *n == 1),
        "post-build publish must reflect the open buffer: {published:?}"
    );
}

#[test]
fn event_enum_is_send() {
    // The build thread and debounce timers send these across threads.
    fn assert_send<T: Send>() {}
    assert_send::<Event>();
}
