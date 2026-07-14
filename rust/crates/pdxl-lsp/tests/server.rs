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

// ── M8b: references, outline, hover ─────────────────────────────────────────

/// A three-file project: one definition (with params), two referencing files.
fn m8b_project() -> (TempTree, ServerState, Receiver<Message>) {
    let t = TempTree::new();
    t.write(
        "common/scripted_triggers/00.txt",
        "# top\nmy_trigger = {\n\tx = $COUNT$\n}\nother_trigger = { always = yes }\n",
    );
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/a.txt",
        "e = { add_trait = brave }\n",
    );
    t.write(
        "common/scripted_effects/b.txt",
        "f = { add_trait = brave has_trait = brave }\n",
    );
    let (server, rx) = server_over_parts(&t);
    (t, server, rx)
}

fn server_over_parts(t: &TempTree) -> (ServerState, Receiver<Message>) {
    let (tx, rx) = unbounded();
    let mut server = ServerState::new(Some(t.path.clone()), tx);
    let project = build_project(None, Some(&t.path.to_string_lossy()));
    server.project_ready(project.map(Box::new));
    (server, rx)
}

#[test]
fn references_from_a_reference_site() {
    let (t, server, _rx) = m8b_project();
    // Cursor on "brave" in a.txt: `e = { add_trait = brave }` → col 19.
    let uri = uri_for(&t, "common/scripted_effects/a.txt");
    let locs = server.references(
        &uri,
        Position {
            line: 0,
            character: 19,
        },
        false,
    );
    assert_eq!(locs.len(), 3, "three reference sites: {locs:?}");
    assert!(locs.iter().all(|l| {
        let p = l.uri.to_file_path().unwrap();
        p.ends_with("a.txt") || p.ends_with("b.txt")
    }));
}

#[test]
fn references_include_declaration_appends_definition_last() {
    let (t, server, _rx) = m8b_project();
    let uri = uri_for(&t, "common/scripted_effects/a.txt");
    let locs = server.references(
        &uri,
        Position {
            line: 0,
            character: 19,
        },
        true,
    );
    assert_eq!(locs.len(), 4, "three refs + declaration");
    let last = locs.last().unwrap().uri.to_file_path().unwrap();
    assert!(
        last.ends_with("common/traits/00.txt"),
        "declaration last (Go parity)"
    );
}

#[test]
fn references_from_the_definition_name() {
    // Defs-first symbolAt: cursor on the DEFINITION name finds its references.
    let (t, server, _rx) = m8b_project();
    let uri = uri_for(&t, "common/traits/00.txt");
    let locs = server.references(
        &uri,
        Position {
            line: 0,
            character: 2,
        },
        false,
    );
    assert_eq!(locs.len(), 3, "cursor on `brave = {{}}` finds all refs");
}

#[test]
fn document_symbol_lists_definitions() {
    let (t, server, _rx) = m8b_project();
    let uri = uri_for(&t, "common/scripted_triggers/00.txt");
    let symbols = server.document_symbol(&uri);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["my_trigger", "other_trigger"]);
    assert!(
        symbols
            .iter()
            .all(|s| s.kind == lsp_types::SymbolKind::FUNCTION)
    );
    // my_trigger is on line 1 (after the comment).
    assert_eq!(
        symbols[0].range.start,
        Position {
            line: 1,
            character: 0
        }
    );
    assert_eq!(
        symbols[0].selection_range.end,
        Position {
            line: 1,
            character: 10
        }
    );
}

#[test]
fn hover_shows_kind_file_and_params() {
    let (t, server, _rx) = m8b_project();
    // Hover the definition name of my_trigger (has $COUNT$).
    let uri = uri_for(&t, "common/scripted_triggers/00.txt");
    let hover = server
        .hover(
            &uri,
            Position {
                line: 1,
                character: 3,
            },
        )
        .expect("hover on definition");
    let lsp_types::HoverContents::Markup(m) = hover.contents else {
        panic!("expected markup");
    };
    assert!(
        m.value.contains("scripted_trigger my_trigger"),
        "{}",
        m.value
    );
    assert!(
        m.value.contains("common/scripted_triggers/00.txt"),
        "{}",
        m.value
    );
    assert!(m.value.contains("`$COUNT$`"), "{}", m.value);
    // The highlighted span is the definition name on line 1.
    assert_eq!(
        hover.range.unwrap().start,
        Position {
            line: 1,
            character: 0
        }
    );
}

#[test]
fn hover_on_unresolved_reference_says_so() {
    let t = TempTree::new();
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = ghost }\n",
    );
    let (server, _rx) = server_over_parts(&t);
    let uri = uri_for(&t, "common/scripted_effects/e.txt");
    let hover = server
        .hover(
            &uri,
            Position {
                line: 0,
                character: 19,
            },
        )
        .expect("hover on unresolved ref");
    let lsp_types::HoverContents::Markup(m) = hover.contents else {
        panic!("expected markup");
    };
    assert!(m.value.contains("trait ghost"), "{}", m.value);
    assert!(m.value.contains("(unresolved)"), "{}", m.value);
}

#[test]
fn m8b_no_result_branches_are_empty() {
    let (t, server, _rx) = m8b_project();
    let uri = uri_for(&t, "common/scripted_effects/a.txt");
    // NOTE: character 0 (the key "e") is NOT empty — in scripted_effects/ the
    // top-level key is a definition, and defs-first symbol_at finds it. The
    // truly symbol-free position is the '=' at character 2.
    assert!(
        server
            .references(
                &uri,
                Position {
                    line: 0,
                    character: 2
                },
                true
            )
            .is_empty()
    );
    assert!(
        server
            .hover(
                &uri,
                Position {
                    line: 0,
                    character: 2
                }
            )
            .is_none()
    );
    // And the def-at-char-0 behavior itself: declaration-only result.
    let decl_only = server.references(
        &uri,
        Position {
            line: 0,
            character: 0,
        },
        true,
    );
    assert_eq!(decl_only.len(), 1, "def with no refs → declaration only");
    // Untracked file.
    let stranger = Url::from_file_path(t.child("nope.txt")).unwrap();
    assert!(
        server
            .references(
                &stranger,
                Position {
                    line: 0,
                    character: 0
                },
                true
            )
            .is_empty()
    );
    assert!(server.document_symbol(&stranger).is_empty());
}

// ── completion ──────────────────────────────────────────────────────────────

/// Position of the first byte of `needle` in `rel`'s content, as an LSP
/// Position (single-byte-per-char fixtures keep this trivial).
fn pos_of(src: &str, needle: &str) -> Position {
    let off = src.find(needle).expect("needle in fixture");
    let line = src[..off].matches('\n').count() as u32;
    let col = (off - src[..off].rfind('\n').map_or(0, |i| i + 1)) as u32;
    Position::new(line, col)
}

const COMPLETION_EVENT: &str = "namespace = t\n\
t.1 = {\n\
\ttrigger = { is_adult = yes }\n\
\timmediate = { add_gold = 5 }\n\
\toption = {\n\
\t\tname = t.1.a\n\
\t\tadd_dread = 5\n\
\t}\n\
}\n";

fn completion_server() -> (ServerState, Receiver<Message>, TempTree) {
    let t = TempTree::new();
    t.write("events/e.txt", COMPLETION_EVENT);
    t.write(
        "common/scripted_effects/fx.txt",
        "my_scripted_fx = { add_gold = 1 }\n",
    );
    let (server, rx) = server_over(&t);
    (server, rx, t)
}

fn labels(items: &[lsp_types::CompletionItem]) -> Vec<&str> {
    items.iter().map(|i| i.label.as_str()).collect()
}

#[test]
fn completion_in_effect_block_offers_effects_and_scripted() {
    let (server, _rx, t) = completion_server();
    let uri = uri_for(&t, "events/e.txt");
    // Cursor on `add_gold` inside immediate.
    let items = server.completion(&uri, pos_of(COMPLETION_EVENT, "add_gold"));
    let names = labels(&items);
    assert!(names.contains(&"add_gold"), "builtin effect offered");
    assert!(names.contains(&"my_scripted_fx"), "scripted effect offered");
    assert!(names.contains(&"if"), "effect control offered");
    assert!(
        !names.contains(&"is_adult"),
        "triggers not offered in effects"
    );
    // Detail carries the supported scopes from the doc tables.
    let add_gold = items.iter().find(|i| i.label == "add_gold").unwrap();
    assert_eq!(
        add_gold.detail.as_deref(),
        Some("effect · scopes: character")
    );
}

#[test]
fn completion_in_trigger_block_offers_triggers() {
    let (server, _rx, t) = completion_server();
    let uri = uri_for(&t, "events/e.txt");
    let items = server.completion(&uri, pos_of(COMPLETION_EVENT, "is_adult"));
    let names = labels(&items);
    assert!(names.contains(&"is_adult"));
    assert!(names.contains(&"trigger_if"));
    assert!(
        !names.contains(&"add_gold"),
        "effects not offered in triggers"
    );
}

#[test]
fn completion_in_option_offers_fields_and_inline_effects() {
    let (server, _rx, t) = completion_server();
    let uri = uri_for(&t, "events/e.txt");
    let items = server.completion(&uri, pos_of(COMPLETION_EVENT, "add_dread"));
    let names = labels(&items);
    // Structural fields (snippet-shaped)…
    assert!(names.contains(&"ai_chance"));
    let trigger = items.iter().find(|i| i.label == "trigger").unwrap();
    assert_eq!(trigger.insert_text.as_deref(), Some("trigger = {\n\t$0\n}"));
    // …plus effects via the option fallback.
    assert!(names.contains(&"add_dread"));
    assert!(names.contains(&"my_scripted_fx"));
}

#[test]
fn completion_at_event_top_level_offers_field_snippets() {
    let (server, _rx, t) = completion_server();
    let uri = uri_for(&t, "events/e.txt");
    // Cursor on the `trigger` KEY: container is the event struct.
    let items = server.completion(&uri, pos_of(COMPLETION_EVENT, "trigger"));
    let names = labels(&items);
    assert!(names.contains(&"immediate"));
    assert!(names.contains(&"option"));
    let option = items.iter().find(|i| i.label == "option").unwrap();
    assert!(option.insert_text.as_deref().unwrap().contains("name = $1"));
    assert!(
        !names.contains(&"add_gold"),
        "event struct is strict: no effects"
    );
}

#[test]
fn completion_at_file_top_level_offers_event_skeleton() {
    let (server, _rx, t) = completion_server();
    let uri = uri_for(&t, "events/e.txt");
    // Line 0, column 0 — before `namespace`, resolves to the file root…
    // actually `namespace` scalar contains it; use the very end of file.
    let end_line = COMPLETION_EVENT.matches('\n').count() as u32;
    let items = server.completion(&uri, Position::new(end_line, 0));
    let names = labels(&items);
    assert!(
        names.contains(&"event"),
        "skeleton snippet at top level: {names:?}"
    );
    assert!(names.contains(&"namespace"));
    let event = items.iter().find(|i| i.label == "event").unwrap();
    assert!(
        event
            .insert_text
            .as_deref()
            .unwrap()
            .contains("immediate = {")
    );
}
