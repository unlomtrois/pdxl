//! Server behavior tests, mirroring the Go `internal/lsp/server_test.go`
//! pattern: build a project over a temp tree, drive the handlers directly, and
//! inspect the messages that would have gone to the client (captured on a
//! channel instead of Go's captured notify function).

use std::path::PathBuf;

use crossbeam_channel::{Receiver, unbounded};
use lsp_server::Message;
use lsp_types::{InlayHintLabel, Position, PublishDiagnosticsParams, Range, Url};
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
fn hover_shows_builtin_effect_trigger_and_scope_link_docs() {
    let t = TempTree::new();
    let src = "namespace = t\nt.1 = {\n\ttrigger = { is_adult = yes }\n\timmediate = { add_gold = 10 add_trait = title:e_test root.holder }\n}\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");

    for (needle, expected) in [
        (
            "add_gold",
            "effect add_gold\n```\n\nSupported scopes: character\n\nadds gold to a character",
        ),
        (
            "is_adult",
            "trigger is_adult\n```\n\nSupported scopes: character\n\nIs the scope character adult?",
        ),
        ("title", "scope link title\n```\n\nTakes a `:key` argument."),
        (
            "holder",
            "scope link holder\n```\n\nInput scopes: landed_title",
        ),
    ] {
        let hover = server
            .hover(&uri, pos_of(src, needle))
            .expect("built-in hover");
        let lsp_types::HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup");
        };
        assert!(markup.value.contains(expected), "{}", markup.value);
    }
}

#[test]
fn inlay_hints_show_best_effort_scope_at_block_openers() {
    let t = TempTree::new();
    let src = "namespace = t\nt.1 = {\n\ttype = character_event\n\timmediate = { add_gold = 10 }\n\toption = { name = t.1.a add_gold = 5 }\n\ttrigger = { any_child = { is_adult = yes } }\n\ttitle:e_test = { add_gold = 5 }\n\ttitle:e_test.faith = { add_gold = 5 }\n}\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    let hints = server.inlay_hints(&uri, Range::new(Position::new(0, 0), Position::new(99, 0)));
    let labels: Vec<&str> = hints
        .iter()
        .filter_map(|hint| match &hint.label {
            InlayHintLabel::String(label) => Some(label.as_str()),
            InlayHintLabel::LabelParts(_) => None,
        })
        .collect();
    assert_eq!(
        labels,
        [
            ": character",
            ": character",
            ": character",
            ": character",
            ": landed_title",
            ": faith"
        ]
    );
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

fn pos_after(src: &str, needle: &str) -> Position {
    let off = src.find(needle).expect("needle in fixture") + needle.len();
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
    t.write("common/traits/t.txt", "patient = {}\n");
    t.write("common/culture/cultures/t.txt", "abe_culture = {}\n");
    t.write(
        "common/religion/religion_types/t.txt",
        "abe_religion = { faiths = { abe_faith = {} } }\n",
    );
    t.write(
        "common/landed_titles/t.txt",
        "e_test = {}\nk_testshire = {}\nk_kingdom = {}\n",
    );
    t.write("events/other.txt", "t.2 = {}\n");
    t.write(
        "events/value.txt",
        "namespace = t\nt.3 = { immediate = { add_trait =  } }\n",
    );
    t.write(
        "common/landed_titles/value.txt",
        "e_outer = { capital =  }\n",
    );
    t.write(
        "events/capital.txt",
        "namespace = t\nt.4 = { immediate = { capital =  } }\n",
    );
    t.write(
        "events/scope.txt",
        "namespace = t\nt.5 = { immediate = { add_trait = title:e } }\n",
    );
    t.write(
        "common/on_action/value.txt",
        "my_action = { events = {  } }\n",
    );
    t.write(
        "events/scoped.txt",
        "namespace = t\nt.6 = { type = character_event immediate = {  } }\n",
    );
    t.write(
        "events/title-scoped.txt",
        "namespace = t\nt.7 = { type = character_event immediate = { title:e_test = {  } } }\n",
    );
    t.write(
        "common/laws/law.txt",
        "crown_authority = {\n\tdefault = crown_authority_1\n\tcrown_authority_0 = {  }\n}\n",
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
    let Some(lsp_types::Documentation::MarkupContent(documentation)) = &add_gold.documentation
    else {
        panic!("effect completion has documentation");
    };
    assert_eq!(documentation.value, "adds gold to a character");
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

#[test]
fn completion_after_reference_key_offers_only_matching_symbols() {
    let (server, _rx, t) = completion_server();
    let src = "namespace = t\nt.3 = { immediate = { add_trait =  } }\n";
    let uri = uri_for(&t, "events/value.txt");
    let items = server.completion(&uri, pos_after(src, "add_trait = "));
    let names = labels(&items);
    assert!(names.contains(&"patient"));
    assert!(!names.contains(&"add_gold"));
    let patient = items.iter().find(|i| i.label == "patient").unwrap();
    assert_eq!(
        patient.detail.as_deref(),
        Some("trait · defined in common/traits/t.txt")
    );
}

#[test]
fn completion_after_gated_capital_offers_titles_only_in_landed_titles() {
    let (server, _rx, t) = completion_server();
    let src = "e_outer = { capital =  }\n";
    let uri = uri_for(&t, "common/landed_titles/value.txt");
    let items = server.completion(&uri, pos_after(src, "capital = "));
    let names = labels(&items);
    assert!(names.contains(&"e_test"));

    let event_src = "namespace = t\nt.4 = { immediate = { capital =  } }\n";
    let uri = uri_for(&t, "events/capital.txt");
    assert!(
        server
            .completion(&uri, pos_after(event_src, "capital = "))
            .is_empty()
    );
}

#[test]
fn completion_for_scope_prefix_offers_titles() {
    let (server, _rx, t) = completion_server();
    let src = "namespace = t\nt.5 = { immediate = { add_trait = title:e } }\n";
    let uri = uri_for(&t, "events/scope.txt");
    let items = server.completion(&uri, pos_after(src, "title:e"));
    let names = labels(&items);
    assert!(names.contains(&"e_test"));
}

#[test]
fn completion_immediately_after_scope_prefix_offers_titles() {
    let (mut server, _rx, t) = completion_server();
    let src = "namespace = t\nt.5 = { immediate = { add_trait = title: } }\n";
    let uri = uri_for(&t, "events/scope.txt");
    server.did_open(uri.clone(), src.to_string());
    let items = server.completion(&uri, pos_after(src, "title:"));
    let names = labels(&items);
    assert!(names.contains(&"e_test"));
    assert!(names.contains(&"k_testshire"));
    let title = items.iter().find(|item| item.label == "e_test").unwrap();
    assert_eq!(title.filter_text.as_deref(), Some("title:e_test"));
    assert!(title.text_edit.is_some());
}

#[test]
fn completion_for_partially_typed_scope_prefix_filters_symbols() {
    let (mut server, _rx, t) = completion_server();
    let src = "namespace = t\nt.5 = { immediate = { add_trait = title:k_ } }\n";
    let uri = uri_for(&t, "events/scope.txt");
    server.did_open(uri.clone(), src.to_string());
    let items = server.completion(&uri, pos_after(src, "title:k_"));
    let names = labels(&items);
    assert!(names.contains(&"k_testshire"));
    assert!(names.contains(&"k_kingdom"));
    assert!(!names.contains(&"e_test"));
}

#[test]
fn completion_for_culture_and_faith_prefixes_offers_matching_symbols() {
    let (mut server, _rx, t) = completion_server();
    let culture_src = "namespace = t\nt.5 = { immediate = { culture:abe } }\n";
    let uri = uri_for(&t, "events/scope.txt");
    server.did_open(uri.clone(), culture_src.to_string());
    assert!(
        labels(&server.completion(&uri, pos_after(culture_src, "culture:abe")))
            .contains(&"abe_culture")
    );

    let faith_src = "namespace = t\nt.5 = { immediate = { faith:abe } }\n";
    server.did_change(uri.clone(), faith_src.to_string());
    assert!(
        labels(&server.completion(&uri, pos_after(faith_src, "faith:abe"))).contains(&"abe_faith")
    );
}

#[test]
fn completion_after_scope_link_dot_offers_members() {
    let (mut server, _rx, t) = completion_server();
    let src = "namespace = t\nt.5 = { immediate = { title:e_test. } }\n";
    let uri = uri_for(&t, "events/scope.txt");
    server.did_open(uri.clone(), src.to_string());
    let items = server.completion(&uri, pos_after(src, "title:e_test."));
    let names = labels(&items);
    assert!(names.contains(&"holder"));
    assert!(names.contains(&"de_jure_liege"));
    assert!(!names.contains(&"spouse"));
    let holder = items.iter().find(|item| item.label == "holder").unwrap();
    assert_eq!(holder.filter_text.as_deref(), Some("title:e_test.holder"));
    assert!(holder.text_edit.is_some());
}

#[test]
fn completion_uses_scope_from_a_chained_title_link() {
    let (mut server, _rx, t) = completion_server();
    let src = "namespace = t\nt.8 = { type = character_event trigger = { title:e_test.culture = {  } } }\n";
    let uri = uri_for(&t, "events/title-scoped.txt");
    server.did_open(uri.clone(), src.to_string());
    let items = server.completion(&uri, pos_after(src, "culture = { "));
    let names = labels(&items);
    assert!(names.contains(&"has_innovation"), "{names:?}");
    assert!(!names.contains(&"add_character_flag"));
}

#[test]
fn completion_in_on_action_event_list_offers_events() {
    let (server, _rx, t) = completion_server();
    let src = "my_action = { events = {  } }\n";
    let uri = uri_for(&t, "common/on_action/value.txt");
    let items = server.completion(&uri, pos_after(src, "events = { "));
    let names = labels(&items);
    assert!(names.contains(&"t.2"));
    assert!(!names.contains(&"patient"));
}

#[test]
fn completion_ranks_builtin_items_for_the_current_scope() {
    let (server, _rx, t) = completion_server();
    let src = "namespace = t\nt.6 = { type = character_event immediate = {  } }\n";
    let uri = uri_for(&t, "events/scoped.txt");
    let items = server.completion(&uri, pos_after(src, "immediate = { "));
    let item = |name: &str| items.iter().find(|item| item.label == name).unwrap();
    assert_eq!(
        item("add_character_flag").sort_text.as_deref(),
        Some("3_add_character_flag")
    );
    assert_eq!(
        item("set_variable").sort_text.as_deref(),
        Some("4_set_variable")
    );
    assert!(!items.iter().any(|item| item.label == "set_title_color"));
}

#[test]
fn completion_filters_effects_in_a_known_title_scope() {
    let (server, _rx, t) = completion_server();
    let src =
        "namespace = t\nt.7 = { type = character_event immediate = { title:e_test = {  } } }\n";
    let uri = uri_for(&t, "events/title-scoped.txt");
    let items = server.completion(&uri, pos_after(src, "title:e_test = { "));
    let names = labels(&items);
    assert!(names.contains(&"set_title_color"));
    assert!(
        names.contains(&"set_variable"),
        "global effects remain valid"
    );
    assert!(!names.contains(&"add_character_flag"));
}

// ── formatting ──────────────────────────────────────────────────────────────

#[test]
fn formatting_returns_whole_document_edit() {
    let t = TempTree::new();
    t.write(
        "events/e.txt",
        "namespace = t\nt.1 = { immediate = { add_gold = 5 } }\n",
    );
    let (mut server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    // Format the open buffer, not disk.
    server.did_open(
        uri.clone(),
        "t.1 = { trigger = { is_adult = yes } }\n".to_string(),
    );
    let edits = server.formatting(&uri).expect("edits");
    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].new_text,
        "t.1 = {\n\ttrigger = {\n\t\tis_adult = yes\n\t}\n}\n"
    );
    assert_eq!(edits[0].range.start, Position::new(0, 0));
}

#[test]
fn formatting_already_formatted_returns_empty_and_broken_returns_none() {
    let t = TempTree::new();
    t.write("events/e.txt", "namespace = t\n");
    let (mut server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    assert_eq!(server.formatting(&uri), Some(vec![]));
    server.did_open(uri.clone(), "t.1 = {\n".to_string());
    assert_eq!(server.formatting(&uri), None);
}

// ── localization ────────────────────────────────────────────────────────────

#[test]
fn loc_key_goto_definition_and_hover_text() {
    let t = TempTree::new();
    t.write(
        "localization/english/my_l_english.yml",
        "\u{feff}l_english:\n my.1.a: \"Hold the line\"\n",
    );
    let src = "namespace = my\nmy.1 = {\n\toption = { name = my.1.a }\n}\n";
    t.write("events/my.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/my.txt");
    let pos = pos_of(src, "my.1.a }"); // the ref, not the def line

    // Go-to-definition lands on the key inside the yml.
    let loc = server.definition(&uri, pos).expect("definition");
    assert!(
        loc.uri.path().ends_with("my_l_english.yml"),
        "definition target: {}",
        loc.uri
    );

    // Hover shows the localized text.
    let hover = server.hover(&uri, pos).expect("hover");
    let lsp_types::HoverContents::Markup(m) = hover.contents else {
        panic!("markup expected")
    };
    assert!(m.value.contains("loc_key my.1.a"), "{}", m.value);
    assert!(m.value.contains("> Hold the line"), "{}", m.value);
}

#[test]
fn inlay_hints_show_loc_text_for_resolved_keys() {
    let t = TempTree::new();
    let long_text = "A common soldier offers a correction to the officers' maneuver. It contradicts the drill manuals.";
    t.write(
        "localization/english/my_l_english.yml",
        &format!("\u{feff}l_english:\n my.1.desc: \"{long_text}\"\n my.1.t: \"Short\"\n"),
    );
    let src = "namespace = my\nmy.1 = {\n\ttitle = my.1.t\n\tdesc = my.1.desc\n\topening = my.1.gone\n}\n";
    t.write("events/my.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/my.txt");

    let whole = lsp_types::Range::new(Position::new(0, 0), Position::new(20, 0));
    let hints = server.inlay_hints(&uri, whole);
    let labels: Vec<String> = hints
        .iter()
        .filter_map(|h| match &h.label {
            lsp_types::InlayHintLabel::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    // Short text shows whole; long text truncates with an ellipsis and keeps
    // the full text in the tooltip; unresolved keys get no hint.
    assert!(labels.iter().any(|l| l == "Short"), "{labels:?}");
    let truncated = labels
        .iter()
        .find(|l| l.starts_with("A common soldier"))
        .expect("desc hint");
    assert!(truncated.ends_with('…'), "{truncated}");
    assert!(truncated.chars().count() <= 73);
    assert!(!labels.iter().any(|l| l.contains("gone")));

    // The desc hint sits at the end of its value.
    let desc_hint = hints
        .iter()
        .find(|h| matches!(&h.label, lsp_types::InlayHintLabel::String(s) if s.starts_with("A common")))
        .unwrap();
    let value_start = pos_of(src, "my.1.desc\n");
    let value_end = Position::new(value_start.line, value_start.character + 9); // "my.1.desc"
    assert_eq!(desc_hint.position, value_end);
    let Some(lsp_types::InlayHintTooltip::String(full)) = &desc_hint.tooltip else {
        panic!("tooltip expected");
    };
    assert!(full.ends_with("drill manuals."));
}

#[test]
fn references_work_from_inside_a_loc_yml() {
    let t = TempTree::new();
    let yml = "\u{feff}l_english:\n my.1.t: \"A Title\"\n";
    t.write("localization/english/my_l_english.yml", yml);
    t.write(
        "events/my.txt",
        "namespace = my\nmy.1 = {\n\ttitle = my.1.t\n}\n",
    );
    let (server, _rx) = server_over(&t);
    let yml_uri = uri_for(&t, "localization/english/my_l_english.yml");

    // Cursor on the KEY inside the yml (line 1, col 1 — after the space).
    let refs = server.references(&yml_uri, Position::new(1, 1), false);
    assert_eq!(refs.len(), 1, "{refs:?}");
    assert!(refs[0].uri.path().ends_with("events/my.txt"));
}

#[test]
fn completion_inside_a_law_offers_law_fields() {
    let (server, _rx, t) = completion_server();
    let uri = uri_for(&t, "common/laws/law.txt");
    let src = std::fs::read_to_string(t.child("common/laws/law.txt")).unwrap();
    // Cursor inside `crown_authority_0 = {  }`.
    let pos = pos_of(&src, "  }");
    let items = server.completion(&uri, pos);
    let names = labels(&items);
    assert!(names.contains(&"can_keep"), "{names:?}");
    assert!(names.contains(&"on_pass"));
    assert!(names.contains(&"ai_will_do"));
    assert!(names.contains(&"succession"));
    // A law body is a strict struct — no builtin effect/trigger names.
    assert!(!names.contains(&"add_gold"), "{names:?}");
}

#[test]
fn completion_inside_a_law_group_offers_attributes_only() {
    let (server, _rx, t) = completion_server();
    let uri = uri_for(&t, "common/laws/law.txt");
    let src = std::fs::read_to_string(t.child("common/laws/law.txt")).unwrap();
    // Cursor right after the group's `default = crown_authority_1` line — in
    // the group body, before the law. Use the newline after that line.
    let pos = pos_of(&src, "\n\tcrown_authority_0");
    let items = server.completion(&uri, pos);
    let names = labels(&items);
    assert!(names.contains(&"cumulative"), "{names:?}");
    assert!(names.contains(&"can_change_law_group"));
    // The struct-fallback (a new law name) injects no effect/trigger names.
    assert!(!names.contains(&"can_keep"));
}
