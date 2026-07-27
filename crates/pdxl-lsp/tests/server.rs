//! Server behavior tests, mirroring the Go `internal/lsp/server_test.go`
//! pattern: build a project over a temp tree, drive the handlers directly, and
//! inspect the messages that would have gone to the client (captured on a
//! channel instead of Go's captured notify function).
//!
//! Every test carries a `#[cfg(feature = …)]` for the game whose script it
//! writes: a scenario built from CK3 directories resolves nothing under the EU5
//! schema, so it would fail rather than skip. The bulk are CK3; the five EU5
//! ones exercise schema-driven behavior that has no CK3 equivalent.

// Helpers are shared across both feature sets but not all are used by both —
// `m8b_project` and the completion helpers are CK3-only, for instance. Gating
// each one would have to be re-widened the moment an EU5 test reached for it.
#![allow(dead_code, unused_imports)]

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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

/// Extracts the markdown string from a hover result.
fn hover_md(server: &ServerState, uri: &Url, pos: Position) -> String {
    match server.hover(uri, pos).expect("hover").contents {
        lsp_types::HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup"),
    }
}

#[cfg(feature = "ck3")]
#[test]
fn namespace_hover_shows_file_doc() {
    // A file-start `#!` doc above `namespace = X`; hovering the namespace name
    // shows it (the events using the namespace are untouched).
    let t = TempTree::new();
    let src = "#! Drill events for the battalion.\nnamespace = T4N_drill\nT4N_drill.1 = { type = character_event }\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    let md = hover_md(&server, &uri, pos_of(src, "T4N_drill\n"));
    assert!(md.contains("namespace T4N_drill"), "{md}");
    assert!(md.contains("Drill events for the battalion."), "{md}");
}

#[cfg(feature = "eu5")]
#[test]
fn entity_hover_and_backlinks_follow_schema_loc_patterns() {
    let t = TempTree::new();
    let src = "#! Entity docs.\nmaona_advance = { age = age_1_traditions }\n";
    let script = "in_game/common/advances/x.txt";
    let loc = "in_game/localization/test/x_l_english.yml";
    let loc_src = "\u{feff}l_english:\n maona_advance: \"Maona\"\n maona_advance_desc: \"A chartered company.\"\n";
    t.write(script, src);
    t.write(loc, loc_src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, script);
    let hover = hover_md(&server, &uri, pos_of(src, "maona_advance"));
    assert!(
        hover.contains("**Localization:** [maona_advance](file:")
            && hover.contains(", [maona_advance_desc](file:"),
        "{hover}"
    );
    let docs = hover.find("Entity docs.").unwrap();
    let loc_label = hover.find("**Localization:**").unwrap();
    let defined = hover.find("Defined in").unwrap();
    assert!(docs < loc_label && loc_label < defined, "{hover}");

    // Both suffix conventions backlink from localization to the advance.
    let loc_uri = uri_for(&t, loc);
    for key in ["maona_advance:", "maona_advance_desc:"] {
        let refs = server.references(&loc_uri, pos_of(loc_src, key), false);
        assert_eq!(refs.len(), 1, "{key}: {refs:?}");
        assert!(refs[0].uri.path().ends_with(script), "{:?}", refs[0]);
    }
}

#[cfg(feature = "ck3")]
#[test]
fn doc_block_shown_on_definition_and_call_site() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    let src = "#! This is a ![brave] effect.\n#! It is removed when the scheme ends.\nscripted_effect my_fx = {\n\tadd_gold = 1\n}\nother = {\n\tmy_fx = yes\n}\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");

    // On the definition name.
    let md = hover_md(&server, &uri, pos_of(src, "my_fx = {"));
    assert!(md.contains("It is removed when the scheme ends."), "{md}");
    // `![brave]` became a go-to-definition link (brave resolves as a trait).
    assert!(md.contains("[brave](file://") && md.contains("#L1"), "{md}");

    // On a call site — shows the target's doc too.
    let md_call = hover_md(&server, &uri, pos_of(src, "my_fx = yes"));
    assert!(
        md_call.contains("It is removed when the scheme ends."),
        "{md_call}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn doc_ref_jumps_to_nested_field() {
    let t = TempTree::new();
    t.write(
        "common/scripted_effects/fx.txt",
        "my_fx = {\n\tadd_gold = 1\n\tinner = {\n\t\tdeep = 2\n\t}\n}\n",
    );
    let src = "#! ![effect:my_fx.inner] ![effect:my_fx.inner.deep] ![effect:my_fx.nope]\nd = { }\n";
    t.write("events/doc.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/doc.txt");

    let links = server.document_links(&uri);
    let fragments: Vec<String> = links
        .iter()
        .map(|l| {
            let u = l.target.as_ref().unwrap().as_str();
            assert!(u.contains("common/scripted_effects/fx.txt"), "{u}");
            u.rsplit('#').next().unwrap().to_string()
        })
        .collect();
    // inner → line 3, inner.deep → line 4, nope (missing) → def line 1.
    assert_eq!(fragments, ["L3", "L4", "L1"]);
}

#[cfg(feature = "ck3")]
#[test]
fn doc_ref_prefers_definition_over_loc_and_honors_explicit_kind() {
    // Name collision: a script value AND a loc key both named `my_val`.
    let t = TempTree::new();
    t.write("common/script_values/v.txt", "my_val = 10\n");
    t.write(
        "localization/english/l.yml",
        "\u{feff}l_english:\n my_val: \"some text\"\n",
    );
    let src = "#! bare ![my_val], loc ![loc:my_val], val ![value:my_val]\nfx = { }\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");

    // Links are in source order: bare, loc:, value:.
    let links = server.document_links(&uri);
    let all: Vec<String> = links
        .iter()
        .map(|l| l.target.as_ref().unwrap().as_str().to_string())
        .collect();
    assert_eq!(links.len(), 3, "{all:?}");
    assert!(
        all[0].contains("common/script_values/v.txt"),
        "bare → def: {all:?}"
    );
    assert!(
        all[1].contains("localization/english/l.yml"),
        "loc: → loc: {all:?}"
    );
    assert!(
        all[2].contains("common/script_values/v.txt"),
        "value: → def: {all:?}"
    );

    // The `value:` link range covers `my_val`, not the `value:` qualifier.
    let val_link = &links[2];
    let range_start = position_to_off(src, val_link.range.start);
    assert_eq!(
        &src[range_start as usize..range_start as usize + 6],
        "my_val"
    );
}

/// Byte offset of an LSP position on line 0 of a single-hover fixture.
fn position_to_off(src: &str, pos: Position) -> u32 {
    let line_start: usize = src
        .split_inclusive('\n')
        .take(pos.line as usize)
        .map(str::len)
        .sum();
    (line_start + pos.character as usize) as u32
}

#[cfg(feature = "ck3")]
#[test]
fn doc_ref_is_clickable_document_link() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    let src = "#! see ![brave] and ![nope]\nfx = { }\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    let links = server.document_links(&uri);
    // Only the resolvable ref becomes a link, targeting the trait file + line.
    assert_eq!(links.len(), 1, "{links:?}");
    let link = &links[0];
    let target = link.target.as_ref().unwrap().as_str();
    assert!(
        target.contains("common/traits/00.txt") && target.ends_with("#L1"),
        "{target}"
    );
    // The link range covers `brave`, not the `![` markers.
    assert_eq!(link.range.start.line, 0);
    assert_eq!(link.range.start.character, "#! see ![".len() as u32);
}

#[cfg(feature = "ck3")]
#[test]
fn doc_ref_semantic_color_reflects_resolution() {
    // Resolved smart-doc refs are TYPE; unresolved ones remain COMMENT.
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    let src = "#! doc ![brave] but not ![nope]\nfx = { }\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    let tokens = server.semantic_tokens(&uri).expect("tokens").data;
    // Legend (semantic.rs TOKEN_TYPES): comment=5, type=9.
    let doc_refs: Vec<_> = tokens.iter().filter(|t| t.token_type == 9).collect();
    assert_eq!(
        doc_refs.len(),
        2,
        "both doc refs should be TYPE: {tokens:?}"
    );
    assert!(
        doc_refs.iter().any(|t| t.token_modifiers_bitset == 0),
        "resolved ref has no modifier"
    );
    assert!(
        doc_refs.iter().any(|t| t.token_modifiers_bitset == 1 << 1),
        "unresolved ref carries the unresolved modifier"
    );
    assert!(tokens.iter().any(|t| t.token_type == 5), "comment segments");
}

#[cfg(feature = "ck3")]
#[test]
fn anchor_declaration_is_coloured_apart_from_its_comment() {
    let t = TempTree::new();
    // One line declares, the next references — the two must not read alike,
    // and neither may read as plain comment text.
    let src = "#! @todo:piety rework the curve\n\
               #! blocked on ![@todo:piety]\n\
               fx = { }\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    let tokens = server.semantic_tokens(&uri).expect("tokens").data;

    // Legend (semantic.rs TOKEN_TYPES): comment=5, type=9, decorator=13.
    let decorators: Vec<_> = tokens.iter().filter(|t| t.token_type == 13).collect();
    assert_eq!(
        decorators.len(),
        1,
        "exactly the declaration is a decorator: {tokens:?}"
    );
    assert_eq!(decorators[0].length as usize, "todo:piety".len());
    // The reference stays TYPE, resolved (no modifier).
    let refs: Vec<_> = tokens.iter().filter(|t| t.token_type == 9).collect();
    assert_eq!(refs.len(), 1, "the reference is a type: {tokens:?}");
    assert_eq!(refs[0].token_modifiers_bitset, 0, "it resolves");
    // The prose around them is still comment.
    assert!(tokens.iter().any(|t| t.token_type == 5), "comment segments");
}

#[cfg(feature = "ck3")]
#[test]
fn doc_block_rules() {
    // Blank line ends the block; plain `#` is not a doc; unresolved ref is plain.
    let t = TempTree::new();
    let src = "#! detached doc\n\n# ordinary comment\n#! attached ![nope]\nscripted_effect fx = { add_gold = 1 }\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    let md = hover_md(&server, &uri, pos_of(src, "fx = {"));
    assert!(md.contains("attached"), "{md}");
    assert!(
        !md.contains("detached doc"),
        "blank line must end the block: {md}"
    );
    assert!(
        !md.contains("ordinary comment"),
        "plain # is not a doc: {md}"
    );
    // Unresolved `![nope]` renders as a code span, not a link.
    assert!(md.contains("`nope`") && !md.contains("[nope]("), "{md}");
}

#[cfg(feature = "ck3")]
#[test]
fn option_field_completion_carries_docs() {
    let t = TempTree::new();
    let src = "t.1 = {\n\ttype = character_event\n\toption = {\n\t\tname = t.1.a\n\t}\n}\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    // Cursor just after the name line, still inside the option block.
    let items = server.completion(&uri, pos_after(src, "name = t.1.a"));
    let trait_item = items
        .iter()
        .find(|i| i.label == "trait")
        .expect("`trait` option field offered");
    let Some(lsp_types::Documentation::MarkupContent(doc)) = &trait_item.documentation else {
        panic!("option field should carry documentation");
    };
    assert!(doc.value.contains("unlock-reason"), "{}", doc.value);
}

#[cfg(feature = "ck3")]
#[test]
fn character_interaction_body_completion_and_hover() {
    let t = TempTree::new();
    let src = "my_interaction = {\n\tcategory = interaction_category_hostile\n\t\n}\n";
    t.write("common/character_interactions/00.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/character_interactions/00.txt");

    let items = server.completion(&uri, pos_after(src, "interaction_category_hostile"));
    let names: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for f in ["is_valid", "is_shown", "on_accept", "ai_accept", "cost"] {
        assert!(names.contains(&f), "missing `{f}`: {names:?}");
    }
    let hover = hover_md(
        &server,
        &uri,
        pos_of(src, "category = interaction_category_hostile"),
    );
    assert!(
        hover.contains("character_interaction field category"),
        "{hover}"
    );
    assert!(hover.contains("interaction menu category"), "{hover}");

    // Value completion for `category = ` offers the defined categories.
    t.write(
        "common/character_interaction_categories/00.txt",
        "interaction_category_hostile = { }\ninteraction_category_diplomacy = { }\n",
    );
    let (mut server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/character_interactions/00.txt");
    let vsrc = "x = {\n\tcategory = \n}\n";
    server.did_open(uri.clone(), vsrc.to_string());
    let vnames: Vec<String> = server
        .completion(&uri, pos_after(vsrc, "category = "))
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(
        vnames.iter().any(|n| n == "interaction_category_hostile"),
        "{vnames:?}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn event_type_enum_and_structural_docs() {
    let t = TempTree::new();
    let src = "namespace = my\nmy.1 = {\n\ttype = \n\timmediate = { }\n}\n";
    t.write("events/my.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/my.txt");

    // `type = ` completes the event-type enum.
    let names: Vec<String> = server
        .completion(&uri, pos_after(src, "type = "))
        .iter()
        .map(|i| i.label.clone())
        .collect();
    for v in [
        "character_event",
        "letter_event",
        "court_event",
        "activity_event",
    ] {
        assert!(names.iter().any(|n| n == v), "missing `{v}`: {names:?}");
    }

    // Hover on a structural effect field documents it.
    let hover = hover_md(&server, &uri, pos_of(src, "immediate = "));
    assert!(hover.contains("event field immediate"), "{hover}");
    assert!(hover.to_lowercase().contains("effect"), "{hover}");
}

#[cfg(feature = "ck3")]
#[test]
fn artifact_enum_field_values_complete_and_hover() {
    let t = TempTree::new();
    let src = "my_crown = {\n\tslot = \n}\n";
    t.write("common/artifacts/types/00.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/artifacts/types/00.txt");

    // `slot = ` completes the known slot types (suggestions, not validation).
    let names: Vec<String> = server
        .completion(&uri, pos_after(src, "slot = "))
        .iter()
        .map(|i| i.label.clone())
        .collect();
    for v in ["helmet", "primary_armament", "wall_big"] {
        assert!(names.iter().any(|n| n == v), "missing `{v}`: {names:?}");
    }
    // Hover on the field lists the vocabulary.
    let hover = hover_md(&server, &uri, pos_of(src, "slot = "));
    assert!(hover.contains("artifact_type field slot"), "{hover}");
    assert!(hover.contains("Values:"), "{hover}");
    assert!(hover.contains("`journal`"), "{hover}");

    // Effect-struct fields get the same treatment: `rarity = ` inside
    // `create_artifact`, and the nested history `type = `.
    let esrc = "e = {\n\tcreate_artifact = {\n\t\trarity = \n\t\thistory = { type = \
                 }\n\t}\n}\n";
    t.write("common/scripted_effects/x.txt", esrc);
    let (server, _rx) = server_over(&t);
    let euri = uri_for(&t, "common/scripted_effects/x.txt");
    let rarity: Vec<String> = server
        .completion(&euri, pos_after(esrc, "rarity = "))
        .iter()
        .map(|i| i.label.clone())
        .collect();
    for v in ["common", "masterwork", "famed", "illustrious"] {
        assert!(rarity.iter().any(|n| n == v), "missing `{v}`: {rarity:?}");
    }
    let history: Vec<String> = server
        .completion(&euri, pos_after(esrc, "history = { type = "))
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(
        history.iter().any(|n| n == "created_before_history"),
        "{history:?}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn secret_type_body_completion_and_hover() {
    let t = TempTree::new();
    let src = "secret_deviant = {\n\tcategory = deviancy\n\t\n}\n";
    t.write("common/secret_types/00.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/secret_types/00.txt");

    let items = server.completion(&uri, pos_after(src, "category = deviancy"));
    let names: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for f in [
        "is_valid",
        "is_shunned",
        "on_discover",
        "on_expose",
        "on_owner_death",
    ] {
        assert!(names.contains(&f), "missing `{f}`: {names:?}");
    }
    let hover = hover_md(&server, &uri, pos_of(src, "category = deviancy"));
    assert!(hover.contains("secret_type field category"), "{hover}");
}

#[cfg(feature = "ck3")]
#[test]
fn character_template_body_completion_and_hover() {
    // A template definition body shares create_character's field structure.
    let t = TempTree::new();
    let src = "T4N_avatar = {\n\tage = 20\n\t\n}\n";
    t.write("common/scripted_character_templates/x.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_character_templates/x.txt");

    let items = server.completion(&uri, pos_after(src, "age = 20"));
    let names: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for f in [
        "faith",
        "culture",
        "dynasty",
        "random_traits_list",
        "after_creation",
    ] {
        assert!(names.contains(&f), "missing `{f}`: {names:?}");
    }
    let hover = hover_md(&server, &uri, pos_of(src, "age = 20"));
    assert!(hover.contains("Starting age"), "{hover}");
}

#[cfg(feature = "ck3")]
#[test]
fn create_character_block_completion_and_hover() {
    let t = TempTree::new();
    // `create_character` is a built-in effect with a documented block structure.
    let src = "t.1 = {\n\ttype = character_event\n\timmediate = {\n\t\tcreate_character = {\n\t\t\tage = 20\n\t\t\t\n\t\t}\n\t}\n}\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");

    // Completion inside the block offers its fields.
    let items = server.completion(&uri, pos_after(src, "age = 20"));
    let names: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for f in [
        "age",
        "gender",
        "faith",
        "culture",
        "after_creation",
        "trait",
    ] {
        assert!(names.contains(&f), "missing `{f}`: {names:?}");
    }
    // A field carries its documentation.
    let after = items.iter().find(|i| i.label == "after_creation").unwrap();
    let Some(lsp_types::Documentation::MarkupContent(doc)) = &after.documentation else {
        panic!("field should carry docs");
    };
    assert!(doc.value.contains("after creation"), "{}", doc.value);

    // Hover on a field key shows its doc.
    let hover = hover_md(&server, &uri, pos_of(src, "age = 20"));
    assert!(hover.contains("create_character field age"), "{hover}");
    assert!(hover.contains("Starting age"), "{hover}");
}

#[cfg(feature = "ck3")]
#[test]
fn modifier_body_completion_and_hover() {
    let t = TempTree::new();
    let src =
        "murder_advice_modifier = {\n\ticon = intrigue_positive\n\tscheme_success_chance = 5\n}\n";
    t.write("common/modifiers/00_x.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/modifiers/00_x.txt");

    // Hover on the built-in modifier tag.
    let hover = server
        .hover(&uri, pos_of(src, "scheme_success_chance"))
        .expect("modifier hover");
    let lsp_types::HoverContents::Markup(markup) = hover.contents else {
        panic!("markup");
    };
    assert!(
        markup.value.contains("modifier scheme_success_chance"),
        "{}",
        markup.value
    );

    // Completion inside the body offers modifier tags.
    let items = server.completion(&uri, pos_of(src, "scheme_success_chance"));
    let names: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        names.contains(&"scheme_success_chance"),
        "modifier tags offered"
    );
}

#[cfg(feature = "eu5")]
#[test]
fn advance_modifier_fallback_hover() {
    let t = TempTree::new();
    let src = "written_alphabet = {\n\tglobal_max_literacy = 5\n}\n";
    t.write("in_game/common/advances/00_x.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "in_game/common/advances/00_x.txt");

    let hover = hover_md(&server, &uri, pos_of(src, "global_max_literacy"));
    assert!(hover.contains("modifier global_max_literacy"), "{hover}");
}

#[cfg(feature = "ck3")]
#[test]
fn hover_builtin_effect_under_struct_fallback() {
    // `start_scheme` is not a named `option` field — it falls under the
    // option struct's effect fallback, so built-in effect hover must still fire.
    let t = TempTree::new();
    let src = "T4N_drill.0 = {\n\ttype = character_event\n\toption = {\n\t\tname = T4N_drill.0.a\n\t\tstart_scheme = { type = x target_character = root }\n\t}\n}\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    let hover = server
        .hover(&uri, pos_of(src, "start_scheme"))
        .expect("built-in effect hover under option fallback");
    let lsp_types::HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup");
    };
    assert!(
        markup.value.contains("effect start_scheme"),
        "{}",
        markup.value
    );
}

#[cfg(feature = "ck3")]
#[test]
fn hover_builtin_effect_in_inline_scripted_def() {
    // An inline `scripted_effect NAME = { … }` in an event file makes its body
    // an Effect clause, so built-in effect hover works there (and inside a
    // `scope:` block) — not just in scripted_effects/ files.
    for (path, src) in [
        (
            "events/e.txt",
            "scripted_effect my_fx = {\n\tadd_stress = 10\n}\n",
        ),
        (
            "events/e.txt",
            "scripted_effect my_fx = {\n\tscope:scheme = {\n\t\tadd_stress = 10\n\t}\n}\n",
        ),
    ] {
        let t = TempTree::new();
        t.write(path, src);
        let (server, _rx) = server_over(&t);
        let uri = uri_for(&t, path);
        let hover = server
            .hover(&uri, pos_of(src, "add_stress"))
            .expect("built-in effect hover inside inline scripted_effect");
        let lsp_types::HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup");
        };
        assert!(
            markup.value.contains("effect add_stress"),
            "{}",
            markup.value
        );
    }
}

#[cfg(feature = "ck3")]
#[test]
fn semantic_tokens_color_keys_values_and_literals() {
    // Legend indices from src/semantic.rs: property=0, variable=1, number=2,
    // string=3, keyword=4, comment=5, operator=7.
    let t = TempTree::new();
    let src = "a = 1\nb = \"x\"\nc = yes\n# note\nk = val\n";
    t.write("common/scripted_effects/s.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/s.txt");

    let tokens = server.semantic_tokens(&uri).expect("semantic tokens");
    let types: Vec<u32> = tokens.data.iter().map(|t| t.token_type).collect();
    assert_eq!(
        types,
        vec![
            0, 7, 2, /* a = 1 */ 0, 7, 3, /* b = "x" */ 0, 7, 4, /* c = yes */ 5,
            /* # note */ 0, 7, 1 /* k = val */
        ],
    );

    // First token: `a` at line 0, col 0, length 1, property, no modifiers.
    let first = tokens.data[0];
    assert_eq!(
        (
            first.delta_line,
            first.delta_start,
            first.length,
            first.token_type
        ),
        (0, 0, 1, 0)
    );
    // The `=` after it is a delta of +2 chars on the same line.
    assert_eq!(
        (tokens.data[1].delta_line, tokens.data[1].delta_start),
        (0, 2)
    );
    // The comment sits on its own line (delta_line advances).
    let comment = tokens.data.iter().find(|t| t.token_type == 5).unwrap();
    assert!(comment.delta_line >= 1);
}

#[cfg(feature = "eu5")]
#[test]
fn pdx_yml_completes_datafunctions_and_loc_keys() {
    let t = TempTree::new();
    let path = "main_menu/localization/english/x_l_english.yml";
    let src = "l_english:\n maona: \"Maona\"\n function: \"[ShowAdv\"\n reference: \"$mao\"\n";
    t.write(path, src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, path);

    let functions = server.completion(&uri, pos_after(src, "ShowAdv"));
    assert!(
        functions.iter().any(|item| item.label == "ShowAdvanceName"),
        "{functions:?}"
    );

    let keys = server.completion(&uri, pos_after(src, "$mao"));
    let maona = keys.iter().find(|item| item.label == "maona").unwrap();
    assert_eq!(maona.kind, Some(lsp_types::CompletionItemKind::REFERENCE));
    let Some(lsp_types::CompletionTextEdit::Edit(edit)) = &maona.text_edit else {
        panic!("expected key-prefix text edit: {maona:?}");
    };
    assert_eq!(edit.new_text, "maona");
}

#[cfg(feature = "eu5")]
#[test]
fn pdx_yml_game_concept_links_resolve() {
    let t = TempTree::new();
    let concept = "subject = { alias = { subjects } texture = x }\n";
    let loc = "l_english:\n test: \"A [subject|e], [Concept('subjects')], [ShowAdvanceName('maona_advance')], and [ROOT.GetCountry.Custom('common_string_positive')]\"\n";
    let concept_path = "main_menu/common/game_concepts/00.txt";
    let loc_path = "main_menu/localization/english/x_l_english.yml";
    let advance_path = "in_game/common/advances/x.txt";
    let custom_loc_path = "in_game/common/customizable_localization/x.txt";
    t.write(concept_path, concept);
    t.write(advance_path, "maona_advance = { age = age_1_traditions }\n");
    t.write(
        custom_loc_path,
        "common_string_positive = { text = { localization_key = yes } }\n",
    );
    t.write(loc_path, loc);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, loc_path);
    for needle in ["subject|e", "subjects')"] {
        let target = server
            .definition(&uri, pos_of(loc, needle))
            .unwrap_or_else(|| panic!("missing concept definition for {needle}"));
        assert!(target.uri.path().ends_with(concept_path), "{target:?}");
    }
    let advance = server
        .definition(&uri, pos_of(loc, "maona_advance')"))
        .expect("ShowAdvanceName argument resolves");
    assert!(advance.uri.path().ends_with(advance_path), "{advance:?}");
    let custom = server
        .definition(&uri, pos_of(loc, "common_string_positive')"))
        .expect("chained Custom argument resolves");
    assert!(custom.uri.path().ends_with(custom_loc_path), "{custom:?}");
    let custom_src = "common_string_positive = { text = { localization_key = yes } }\n";
    let backlinks = server.references(
        &uri_for(&t, custom_loc_path),
        pos_of(custom_src, "common_string_positive"),
        false,
    );
    assert_eq!(backlinks.len(), 1, "{backlinks:?}");
    assert_eq!(backlinks[0].uri, uri);
}

#[cfg(feature = "ck3")]
#[test]
fn pdx_yml_semantic_tokens_highlight_inline_dialect() {
    let t = TempTree::new();
    let src = "l_english:\n key: \"#bold $other$ $VAL|0$ [subject|e] [GetPlayer.GetName] [ShowAdvanceName('maona_advance')] [advance|e] @gold!#!\"\n other: \"Linked text\"\n VAL: \"Valencia\"\n";
    let path = if cfg!(feature = "eu5") {
        "main_menu/localization/english/x_l_english.yml"
    } else {
        "localization/english/x_l_english.yml"
    };
    t.write(path, src);
    if cfg!(feature = "eu5") {
        t.write(
            "in_game/common/advances/x.txt",
            "maona_advance = { age = age_1_traditions }\n",
        );
    }
    let (server, rx) = server_over(&t);
    assert!(
        rx.try_iter().any(|message| matches!(
            message,
            lsp_server::Message::Request(request)
                if request.method == "workspace/semanticTokens/refresh"
        )),
        "project readiness must invalidate first-pass semantic tokens"
    );
    let tokens = server
        .semantic_tokens(&uri_for(&t, path))
        .expect("pdx-yml semantic tokens");
    let types: Vec<_> = tokens.data.iter().map(|token| token.token_type).collect();
    assert!(types.contains(&0), "localization key: {types:?}");
    assert!(types.contains(&8), "dumped datafunction: {types:?}");
    assert!(types.contains(&9), "$key$ reference: {types:?}");
    if cfg!(feature = "eu5") {
        let wanted_col = src.lines().nth(1).unwrap().find("maona_advance").unwrap() as u32;
        let (mut line, mut col) = (0, 0);
        let mut entity_type = None;
        for token in &tokens.data {
            line += token.delta_line;
            col = if token.delta_line == 0 {
                col + token.delta_start
            } else {
                token.delta_start
            };
            if line == 1 && col == wanted_col {
                entity_type = Some(token.token_type);
            }
        }
        assert_eq!(entity_type, Some(11), "maona_advance token: {types:?}");
    }
    assert!(types.contains(&6), "icon markup: {types:?}");
    assert!(types.contains(&12), "runtime parameter: {types:?}");

    let uri = uri_for(&t, path);
    let target = server
        .definition(&uri, pos_of(src, "other$"))
        .expect("$key$ resolves to another localization entry");
    assert_eq!(target.range.start.line, 2);
    assert!(
        server.definition(&uri, pos_of(src, "VAL|0")).is_none(),
        "runtime VAL must not resolve to the Valencia localization key"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn semantic_tokens_are_schema_aware() {
    // Legend: property=0, variable=1, operator=7, function=8, type=9,
    // namespace=10; defaultLibrary modifier = bit 0.
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave x = scope:root }\n",
    );
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/e.txt");

    let data = server.semantic_tokens(&uri).expect("tokens").data;
    let types: Vec<u32> = data.iter().map(|t| t.token_type).collect();
    // e=prop  ==op  add_trait=function  ==op  brave=type(resolved ref)
    // x=prop  ==op  scope=namespace  :=op  root=variable
    assert_eq!(types, vec![0, 7, 8, 7, 9, 0, 7, 10, 7, 1]);

    // The builtin effect carries the defaultLibrary modifier.
    let add_trait = data[2];
    assert_eq!(add_trait.token_type, 8);
    assert_eq!(add_trait.token_modifiers_bitset, 1);
    // The resolved reference `brave` is a type, with no modifier.
    assert_eq!(data[4].token_type, 9);
    assert_eq!(data[4].token_modifiers_bitset, 0);
}

#[cfg(feature = "ck3")]
#[test]
fn code_lens_counts_references_over_every_definition() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\ncraven = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = { add_trait = brave has_trait = brave }\n",
    );
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/traits/00.txt");

    // A lens per definition (both traits), regardless of reference count.
    let lenses = server.code_lens(&uri);
    assert_eq!(lenses.len(), 2, "one lens per definition");
    assert!(
        lenses.iter().all(|l| l.command.is_none()),
        "phase 1 is title-free (lazy resolve)"
    );

    // Resolve the first lens (brave, referenced twice) → "2 references".
    let resolved = server.code_lens_resolve(lenses[0].clone());
    let cmd = resolved.command.expect("resolve fills the command");
    assert_eq!(cmd.title, "2 references");
    assert_eq!(cmd.command, "pdxl.showReferences");

    // craven has none.
    let craven = server.code_lens_resolve(lenses[1].clone());
    assert_eq!(craven.command.unwrap().title, "0 references");
}

#[cfg(feature = "ck3")]
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
            ": character (effect)",  // immediate — first surfaces character
            ": character (effect)",  // option — a struct whose loose keys are effects
            ": character (trigger)", // trigger — first surfaces it in this subtree
            // any_child inherits character (already shown by trigger) → no repeat
            ": landed_title", // title:e_test — scope change
            ": faith"         // title:e_test.faith — scope change
        ]
    );
}

#[cfg(feature = "eu5")]
#[test]
fn eu5_event_type_and_fixed_blocks_emit_scope_hints() {
    let t = TempTree::new();
    let src = "namespace = t\nt.1 = {\n type = location_event\n trigger = { always = yes }\n immediate = { add_location_modifier = x }\n option = { name = t.1.a add_location_modifier = x }\n major_trigger = { always = yes }\n}\n";
    let path = "in_game/events/e.txt";
    t.write(path, src);
    let (server, _rx) = server_over(&t);
    let hints = server.inlay_hints(
        &uri_for(&t, path),
        Range::new(Position::new(0, 0), Position::new(99, 0)),
    );
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
            ": location (trigger)",
            ": location (effect)",
            ": location (effect)",
            ": country (trigger)",
        ]
    );
}

#[cfg(feature = "ck3")]
#[test]
fn inlay_hints_no_repeat_for_inherited_scope() {
    // Nested effect blocks that don't change scope show the hint only once, on
    // the outermost (the user's random_list / add_trait_xp case).
    let t = TempTree::new();
    let src = "t.1 = {\n\ttype = character_event\n\toption = {\n\t\trandom_list = {\n\t\t\t25 = { add_trait_xp = { trait = brave } }\n\t\t}\n\t}\n}\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    let labels: Vec<String> = server
        .inlay_hints(&uri, Range::new(Position::new(0, 0), Position::new(99, 0)))
        .iter()
        .filter_map(|h| match &h.label {
            InlayHintLabel::String(l) => Some(l.clone()),
            InlayHintLabel::LabelParts(_) => None,
        })
        .collect();
    // Only `option` surfaces character; random_list / 25 / add_trait_xp inherit.
    assert_eq!(labels, [": character (effect)"]);
}

#[cfg(feature = "ck3")]
#[test]
fn inlay_hints_suppress_structural_inherited_scope() {
    // `cooldown` (only duration fields) and `right_portrait` (config) merely
    // inherit the character scope — no hint. `immediate` (effects) and a
    // `scope:`/`title:` scope change still get one.
    let t = TempTree::new();
    let src = "t.1 = {\n\ttype = character_event\n\tcooldown = { years = 1 }\n\tright_portrait = { character = root }\n\timmediate = { add_gold = 5 }\n}\n";
    t.write("events/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "events/e.txt");
    let labels: Vec<String> = server
        .inlay_hints(&uri, Range::new(Position::new(0, 0), Position::new(99, 0)))
        .iter()
        .filter_map(|h| match &h.label {
            InlayHintLabel::String(l) => Some(l.clone()),
            InlayHintLabel::LabelParts(_) => None,
        })
        .collect();
    // Only the effect block; cooldown and right_portrait are suppressed.
    assert_eq!(labels, [": character (effect)"]);
}

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
#[test]
fn completion_for_scope_prefix_offers_titles() {
    let (server, _rx, t) = completion_server();
    let src = "namespace = t\nt.5 = { immediate = { add_trait = title:e } }\n";
    let uri = uri_for(&t, "events/scope.txt");
    let items = server.completion(&uri, pos_after(src, "title:e"));
    let names = labels(&items);
    assert!(names.contains(&"e_test"));
}

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
#[test]
fn loc_concept_link_goto_definition() {
    let t = TempTree::new();
    // A concept def + an alias, and a loc line that links both an alias and a
    // canonical concept via the `|E` encyclopedia command.
    t.write(
        "common/game_concepts/00.txt",
        "vassal = { alias = { vassals } }\nruler = { }\n",
    );
    let yml = "\u{feff}l_english:\n k: \"A [ruler|E] and their [vassals|E].\"\n";
    t.write("localization/english/c_l_english.yml", yml);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "localization/english/c_l_english.yml");

    // Jump from the canonical link → the concept definition.
    let loc = server
        .definition(&uri, pos_of(yml, "ruler|E"))
        .expect("definition on [ruler|E]");
    assert!(
        loc.uri.path().ends_with("common/game_concepts/00.txt"),
        "target: {}",
        loc.uri
    );

    // The alias link resolves too (to the owning `vassal` concept).
    let loc = server
        .definition(&uri, pos_of(yml, "vassals|E"))
        .expect("definition on [vassals|E]");
    assert!(loc.uri.path().ends_with("common/game_concepts/00.txt"));
}

#[cfg(feature = "ck3")]
#[test]
fn situation_body_completion_hover_and_effect_context() {
    let t = TempTree::new();
    let src = "dynastic_cycle = {\n\twindow = dynastic_cycle\n\t\n\ton_start = {\n\t\t\n\t}\n}\n";
    t.write("common/situation/situations/00.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/situation/situations/00.txt");

    // Body completion offers documented situation fields.
    let items = server.completion(
        &uri,
        Position {
            line: 2,
            character: 1,
        },
    );
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for f in [
        "phases",
        "participant_groups",
        "is_unique",
        "situation_group_type",
    ] {
        assert!(labels.contains(&f), "missing field `{f}`: {labels:?}");
    }

    // Inside `on_start = { … }` the context is Effect, so effects complete.
    let eff = server.completion(
        &uri,
        Position {
            line: 4,
            character: 2,
        },
    );
    assert!(
        eff.iter()
            .any(|i| i.label == "add_gold" || i.label == "trigger_event"),
        "on_start body should complete effects"
    );

    // Hover on the `window` enum field documents it.
    let hover = hover_md(&server, &uri, pos_of(src, "window = "));
    assert!(hover.contains("situation_type field window"), "{hover}");
}

#[cfg(feature = "ck3")]
#[test]
fn scheme_body_offers_corpus_fields_and_enum_values() {
    let t = TempTree::new();
    // Body completion includes the corpus-only fields the .info omits.
    let src = "murder = {\n\t\n}\n";
    t.write("common/schemes/scheme_types/00.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/schemes/scheme_types/00.txt");
    let labels: Vec<String> = server
        .completion(&uri, pos_after(src, "murder = {\n\t"))
        .iter()
        .map(|i| i.label.clone())
        .collect();
    for f in [
        "on_start",
        "discovery_desc",
        "phases_per_agent_charge",
        "starting_agent_slots",
    ] {
        assert!(
            labels.iter().any(|l| l == f),
            "missing scheme field `{f}`: {labels:?}"
        );
    }

    // `category = ` value completion offers the enum, including the newer
    // `political` category found in the corpus.
    let vsrc = "murder = {\n\tcategory = \n}\n";
    t.write("common/schemes/scheme_types/01.txt", vsrc);
    let (server, _rx) = server_over(&t);
    let vuri = uri_for(&t, "common/schemes/scheme_types/01.txt");
    let vlabels: Vec<String> = server
        .completion(&vuri, pos_after(vsrc, "category = "))
        .iter()
        .map(|i| i.label.clone())
        .collect();
    for v in ["personal", "contract", "hostile", "political"] {
        assert!(
            vlabels.iter().any(|l| l == v),
            "category values missing `{v}`: {vlabels:?}"
        );
    }
}

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

/// Flattens a WorkspaceEdit into (file-suffix, new_text) pairs for assertions.
fn edit_pairs(edit: &lsp_types::WorkspaceEdit) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (uri, edits) in edit.changes.as_ref().expect("changes") {
        for e in edits {
            out.push((uri.path().to_string(), e.new_text.clone()));
        }
    }
    out.sort();
    out
}

#[cfg(feature = "ck3")]
#[test]
fn rename_loc_key_across_yml_and_script() {
    let t = TempTree::new();
    let yml = "\u{feff}l_english:\n my.1.t: \"A Title\"\n";
    t.write("localization/english/my_l_english.yml", yml);
    t.write(
        "events/my.txt",
        "namespace = my\nmy.1 = {\n\ttitle = my.1.t\n}\n",
    );
    let (server, _rx) = server_over(&t);
    let yml_uri = uri_for(&t, "localization/english/my_l_english.yml");
    let ev_uri = uri_for(&t, "events/my.txt");

    // Rename from the reference site (`title = my.1.t`).
    let ev_src = "namespace = my\nmy.1 = {\n\ttitle = my.1.t\n}\n";
    let edit = server
        .rename(&ev_uri, pos_of(ev_src, "my.1.t\n"), "my.1.title")
        .expect("workspace edit");
    let pairs = edit_pairs(&edit);
    // Both the yml key and the script ref are rewritten.
    assert!(
        pairs
            .iter()
            .any(|(f, n)| f.ends_with("my_l_english.yml") && n == "my.1.title"),
        "{pairs:?}"
    );
    assert!(
        pairs
            .iter()
            .any(|(f, n)| f.ends_with("events/my.txt") && n == "my.1.title"),
        "{pairs:?}"
    );

    // Rename from the definition site (the yml key) yields the same edit set.
    let from_def = server
        .rename(&yml_uri, Position::new(1, 1), "my.1.title")
        .expect("workspace edit");
    assert_eq!(edit_pairs(&from_def), pairs);
}

#[cfg(feature = "ck3")]
#[test]
fn rename_preserves_quoting_per_site() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\ncraven = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "unquoted = {\n\tadd_trait = brave\n}\nquoted = {\n\tadd_trait = \"brave\"\n}\n",
    );
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/traits/00.txt");

    // Rename the trait def `brave` → `daring`.
    let edit = server
        .rename(&uri, Position::new(0, 0), "daring")
        .expect("workspace edit");
    let texts: Vec<String> = edit_pairs(&edit).into_iter().map(|(_, n)| n).collect();
    // Unquoted ref + def become `daring`; the quoted ref becomes `"daring"`.
    assert!(texts.contains(&"daring".to_string()), "{texts:?}");
    assert!(texts.contains(&"\"daring\"".to_string()), "{texts:?}");
}

#[cfg(feature = "ck3")]
#[test]
fn prepare_rename_selects_identifier_and_rejects_non_symbols() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "e = {\n\tadd_trait = \"brave\"\n}\n",
    );
    let (server, _rx) = server_over(&t);
    let euri = uri_for(&t, "common/scripted_effects/e.txt");

    // On the quoted ref, prepareRename selects the inner identifier + name.
    let esrc = "e = {\n\tadd_trait = \"brave\"\n}\n";
    let resp = server
        .prepare_rename(&euri, pos_of(esrc, "brave\""))
        .expect("prepare response");
    match resp {
        lsp_types::PrepareRenameResponse::RangeWithPlaceholder { placeholder, range } => {
            assert_eq!(placeholder, "brave");
            // Range covers `brave`, not the quotes.
            assert_eq!(range.start.character, 14);
            assert_eq!(range.end.character, 19);
        }
        other => panic!("unexpected: {other:?}"),
    }

    // On a non-symbol position (the builtin `add_trait` keyword, not a
    // def/ref/call), prepareRename is None.
    assert!(
        server
            .prepare_rename(&euri, pos_of(esrc, "add_trait"))
            .is_none()
    );
}

#[cfg(feature = "ck3")]
#[test]
fn workspace_symbols_fuzzy_search() {
    let t = TempTree::new();
    t.write(
        "common/traits/00.txt",
        "brave = { }\nbrave_hearted = { }\ncraven = { }\n",
    );
    t.write(
        "localization/english/x_l_english.yml",
        "\u{feff}l_english:\n brave_desc: \"Brave\"\n",
    );
    let (server, _rx) = server_over(&t);

    // Substring query ranks the exact match first, then the longer name.
    let names: Vec<String> = server
        .workspace_symbols("brave")
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(
        names.first().map(String::as_str),
        Some("brave"),
        "{names:?}"
    );
    assert!(names.iter().any(|n| n == "brave_hearted"), "{names:?}");
    assert!(names.iter().any(|n| n == "brave_desc"), "{names:?}");
    // `craven` has no `brave` subsequence — excluded.
    assert!(!names.iter().any(|n| n == "craven"), "{names:?}");

    // Subsequence query (`cvn` ⊂ `craven`) matches; carries kind + location.
    let cvn = server.workspace_symbols("cvn");
    let craven = cvn.iter().find(|s| s.name == "craven").expect("craven");
    assert_eq!(craven.container_name.as_deref(), Some("trait"));
    assert!(craven.location.uri.path().ends_with("common/traits/00.txt"));

    // A query matching nothing returns empty.
    assert!(server.workspace_symbols("zzzzz").is_empty());
    // Empty query returns a (capped) listing of everything.
    assert!(server.workspace_symbols("").len() >= 4);
}

#[cfg(feature = "ck3")]
#[test]
fn rename_rejects_invalid_new_name() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/traits/00.txt");
    // Whitespace / structural chars are refused.
    assert!(
        server
            .rename(&uri, Position::new(0, 0), "bad name")
            .is_none()
    );
    assert!(server.rename(&uri, Position::new(0, 0), "").is_none());
    assert!(server.rename(&uri, Position::new(0, 0), "a=b").is_none());
}

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
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

#[cfg(feature = "ck3")]
#[test]
fn law_fields_get_documented_root_scope_hints() {
    let t = TempTree::new();
    let src = "crown_authority = {\n\
        \tcrown_authority_0 = {\n\
        \t\tcan_keep = { has_trait = brave }\n\
        \t\tcan_title_have = { tier = tier_kingdom }\n\
        \t\ton_pass = { add_gold = 5 }\n\
        \t}\n\
        }\n";
    t.write("common/laws/law.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/laws/law.txt");
    let whole = Range::new(Position::new(0, 0), Position::new(20, 0));
    let hints = server.inlay_hints(&uri, whole);
    let labels: Vec<String> = hints
        .iter()
        .filter_map(|h| match &h.label {
            InlayHintLabel::String(s) => Some(s.trim().to_string()),
            _ => None,
        })
        .collect();
    // Scope AND clause kind: can_keep is a character trigger, on_pass a
    // character effect, can_title_have a landed_title trigger.
    assert!(
        labels.iter().any(|l| l == ": character (trigger)"),
        "{labels:?}"
    );
    assert!(
        labels.iter().any(|l| l == ": character (effect)"),
        "{labels:?}"
    );
    assert!(
        labels.iter().any(|l| l == ": landed_title (trigger)"),
        "{labels:?}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn hover_on_a_law_field_key_describes_it() {
    let t = TempTree::new();
    let src = "crown_authority = {\n\tcrown_authority_0 = {\n\t\tcan_keep = { }\n\t}\n}\n";
    t.write("common/laws/law.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/laws/law.txt");
    let pos = pos_of(src, "can_keep");
    let hover = server.hover(&uri, pos).expect("hover");
    let lsp_types::HoverContents::Markup(m) = hover.contents else {
        panic!("markup expected")
    };
    assert!(m.value.contains("law field can_keep"), "{}", m.value);
    // Compact type line + the distilled _laws.info documentation.
    assert!(
        m.value.contains("*trigger · root scope `character`*"),
        "{}",
        m.value
    );
    assert!(
        m.value.contains("Requirements for keeping the law"),
        "{}",
        m.value
    );
}

#[cfg(feature = "ck3")]
#[test]
fn gui_template_definition_and_references() {
    let t = TempTree::new();
    t.write(
        "gui/shared_templates.gui",
        "template MyHeader {\n\tsize = { 100% 34 }\n}\n\
         types Widgets {\n\ttype my_marker = widget {\n\t\tblock \"label\" {}\n\t}\n}\n",
    );
    let src = "window = {\n\tusing = MyHeader\n\tmy_marker = {\n\t\tenabled = [Foo.Bar]\n\t}\n}\n";
    t.write("gui/window_test.gui", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "gui/window_test.gui");

    // Go-to-definition from the `using = MyHeader` value → the template def.
    let loc = server
        .definition(&uri, pos_of(src, "MyHeader"))
        .expect("template definition found");
    assert!(
        loc.uri
            .to_file_path()
            .unwrap()
            .ends_with("gui/shared_templates.gui")
    );
    assert_eq!(
        loc.range.start,
        Position {
            line: 0,
            character: 9
        }
    );

    // … and from the `my_marker = { … }` instantiation key → the type def.
    let loc = server
        .definition(&uri, pos_of(src, "my_marker"))
        .expect("type definition found");
    assert!(
        loc.uri
            .to_file_path()
            .unwrap()
            .ends_with("gui/shared_templates.gui")
    );

    // The datafunction value produces no parse diagnostics for gui files.
    let refs = server.references(&uri, pos_of(src, "MyHeader"), true);
    assert!(
        refs.len() >= 2,
        "definition + using site expected: {refs:?}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn gui_datafn_hover_and_diagnostics() {
    let t = TempTree::new();
    let src = "window = {\n\
         \tdatacontext = \"[GetPlayer.GetLiege]\"\n\
         \tvisible = [GetPlayer.NotARealFunction]\n\
         }\n";
    t.write("gui/window_x.gui", src);
    let (server, rx) = server_over(&t);
    let uri = uri_for(&t, "gui/window_x.gui");

    // Hover on a member segment shows its signature and return type.
    let hover = hover_md(&server, &uri, pos_of(src, "GetLiege"));
    assert!(hover.contains("Character.GetLiege"), "{hover}");
    assert!(hover.contains("Character"), "{hover}");

    // The bad member produced a datafunction warning for this mod file.
    let publishes = drain_publishes(&rx);
    let ours: Vec<_> = publishes
        .iter()
        .filter(|(p, _)| p.ends_with("gui/window_x.gui"))
        .collect();
    assert!(
        ours.iter().any(|(_, n)| *n >= 1),
        "expected a datafn warning publish: {publishes:?}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn gui_semantic_tokens_color_keywords_types_and_datafns() {
    // Legend: property=0, variable=1, string=3, keyword=4, function=8, type=9.
    let t = TempTree::new();
    let src = "types T {\n\
         \ttype my_marker = widget {\n\
         \t\tvisible = [GetPlayer.IsValid]\n\
         \t}\n\
         }\n\
         window = {\n\
         \tmy_marker = {}\n\
         }\n";
    t.write("gui/window_x.gui", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "gui/window_x.gui");
    let tokens = server.semantic_tokens(&uri).expect("tokens").data;

    // Reconstruct absolute (line, char, len, type) for assertions.
    let mut abs = Vec::new();
    let (mut line, mut ch) = (0u32, 0u32);
    for t in &tokens {
        if t.delta_line > 0 {
            line += t.delta_line;
            ch = t.delta_start;
        } else {
            ch += t.delta_start;
        }
        abs.push((line, ch, t.length, t.token_type));
    }
    let at = |l: u32, c: u32| abs.iter().find(|&&(al, ac, ..)| al == l && ac == c);

    // `types` and `type` keywords (4), names and base as types (9).
    assert_eq!(at(0, 0).unwrap().3, 4, "types keyword: {abs:?}");
    assert_eq!(at(0, 6).unwrap().3, 9, "types name T");
    assert_eq!(at(1, 1).unwrap().3, 4, "type keyword");
    assert_eq!(at(1, 6).unwrap().3, 9, "type name my_marker");
    assert_eq!(at(1, 18).unwrap().3, 9, "base widget");
    // Datafunction segments resolve as builtin functions (8).
    let fun_count = abs
        .iter()
        .filter(|&&(al, .., ty)| al == 2 && ty == 8)
        .count();
    assert_eq!(fun_count, 2, "GetPlayer + IsValid: {abs:?}");
    // The instantiation key `my_marker = {}` is a resolved gui ref (9).
    assert_eq!(at(6, 1).unwrap().3, 9, "instantiation ref: {abs:?}");
}

#[cfg(feature = "ck3")]
#[test]
fn gui_completion_keys_values_and_datafns() {
    let t = TempTree::new();
    // Corpus files the vocabulary is mined from.
    t.write(
        "gui/corpus.gui",
        "template Base {\n}\n\
         window = {\n\
         \ticon = {\n\t\tparentanchor = center\n\t\tsize = { 34 34 }\n\t\ttexture = \"x.dds\"\n\t}\n\
         \ticon = {\n\t\tparentanchor = top\n\t\ttexture = \"y.dds\"\n\t}\n\
         }\n",
    );
    let src = "window = {\n\ticon = {\n\t\t\n\t}\n\tvisible = [GetPl\n}\n";
    t.write("gui/edit.gui", src);
    let (mut server, _rx) = server_over(&t);
    let uri = uri_for(&t, "gui/edit.gui");
    server.did_open(uri.clone(), src.to_string());

    // Key position inside `icon = { … }`: mined icon properties, ranked.
    let items = server.completion(
        &uri,
        Position {
            line: 2,
            character: 2,
        },
    );
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for k in ["parentanchor", "texture", "size", "block", "using"] {
        assert!(labels.contains(&k), "missing `{k}`: {labels:?}");
    }

    // Value position: `parentanchor = ` offers mined values.
    let vsrc = "window = {\n\ticon = {\n\t\tparentanchor = \n\t}\n}\n";
    server.did_open(uri.clone(), vsrc.to_string());
    let items = server.completion(&uri, pos_after(vsrc, "parentanchor = "));
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"center"), "{labels:?}");
    assert!(labels.contains(&"top"), "{labels:?}");

    // `using = ` offers defined templates.
    let usrc = "window = {\n\tusing = \n}\n";
    server.did_open(uri.clone(), usrc.to_string());
    let items = server.completion(&uri, pos_after(usrc, "using = "));
    assert!(
        items.iter().any(|i| i.label == "Base"),
        "{:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );

    // Datafunction root: `[GetPl` offers GetPlayer (registry global).
    server.did_open(uri.clone(), src.to_string());
    let items = server.completion(&uri, pos_after(src, "[GetPl"));
    assert!(
        items.iter().any(|i| i.label == "GetPlayer"),
        "{:?}",
        items.iter().take(8).map(|i| &i.label).collect::<Vec<_>>()
    );

    // Datafunction member: `[GetPlayer.` offers Character members.
    let dsrc = "window = {\n\tvisible = [GetPlayer.\n}\n";
    server.did_open(uri.clone(), dsrc.to_string());
    let items = server.completion(&uri, pos_after(dsrc, "[GetPlayer."));
    assert!(
        items.iter().any(|i| i.label == "GetLiege"),
        "{:?}",
        items.iter().take(8).map(|i| &i.label).collect::<Vec<_>>()
    );
}

#[cfg(feature = "ck3")]
#[test]
fn gui_datafn_completion_inside_quoted_string() {
    let t = TempTree::new();
    let src = "window = {\n\tdatacontext = \"[Ge\n}\n";
    t.write("gui/edit.gui", src);
    let (mut server, _rx) = server_over(&t);
    let uri = uri_for(&t, "gui/edit.gui");
    server.did_open(uri.clone(), src.to_string());
    let items = server.completion(&uri, pos_after(src, "\"[Ge"));
    assert!(
        items.iter().any(|i| i.label == "GetPlayer"),
        "{:?}",
        items.iter().take(8).map(|i| &i.label).collect::<Vec<_>>()
    );
    // … and member completion after a dot inside the quotes.
    let src2 = "window = {\n\tdatacontext = \"[GetTitleByKey( 'k_x' ).\n}\n";
    server.did_open(uri.clone(), src2.to_string());
    let items = server.completion(&uri, pos_after(src2, ")."));
    assert!(
        items.iter().any(|i| i.label == "GetHolder"),
        "{:?}",
        items.iter().take(8).map(|i| &i.label).collect::<Vec<_>>()
    );
}

#[cfg(feature = "ck3")]
#[test]
fn gui_datafn_completion_with_closed_bracket() {
    // Auto-closing pairs mean the user types inside `"[GetTi]"` — the `]`
    // and `"` already sit after the cursor.
    let t = TempTree::new();
    let src = "text_single = {\n\tdatacontext = \"[GetTi]\"\n\tmax_width = 300\n}\n";
    t.write("gui/edit.gui", src);
    let (mut server, _rx) = server_over(&t);
    let uri = uri_for(&t, "gui/edit.gui");
    server.did_open(uri.clone(), src.to_string());
    let items = server.completion(&uri, pos_after(src, "[GetTi"));
    assert!(
        items.iter().any(|i| i.label == "GetTitleByKey"),
        "{:?}",
        items.iter().take(8).map(|i| &i.label).collect::<Vec<_>>()
    );
}

#[cfg(feature = "ck3")]
#[test]
fn gui_property_docs_in_hover_and_completion() {
    let t = TempTree::new();
    let src = "window = {\n\tparentanchor = center\n\ticon = {\n\t\tparentanchor = top\n\t}\n}\n";
    t.write("gui/edit.gui", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "gui/edit.gui");

    // Hover on a property key shows its curated doc.
    let hover = hover_md(&server, &uri, pos_of(src, "parentanchor"));
    assert!(hover.contains("Which point of the parent"), "{hover}");

    // Completion items carry the doc.
    let items = server.completion(
        &uri,
        Position {
            line: 3,
            character: 2,
        },
    );
    let pa = items
        .iter()
        .find(|i| i.label == "parentanchor")
        .expect("parentanchor offered");
    assert!(pa.documentation.is_some(), "doc attached");
}

#[cfg(feature = "ck3")]
#[test]
fn custom_description_text_resolves_across_kinds() {
    let t = TempTree::new();
    t.write(
        "common/trigger_localization/00.txt",
        "can_afford_rice = { first = I_CAN_AFFORD }\n",
    );
    t.write(
        "common/effect_localization/00.txt",
        "gain_rice = { first = I_GAIN_RICE }\n",
    );
    let src = "e = {\n\
         \tcustom_description = { text = can_afford_rice }\n\
         \tcustom_description = { text = gain_rice }\n\
         \tcustom_description = { text = missing_entry }\n\
         }\n";
    t.write("common/scripted_effects/e.txt", src);
    let (server, rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/e.txt");

    // Primary kind (trigger_loc) resolves…
    let loc = server
        .definition(&uri, pos_of(src, "can_afford_rice"))
        .expect("trigger_loc definition");
    assert!(
        loc.uri
            .to_file_path()
            .unwrap()
            .ends_with("common/trigger_localization/00.txt")
    );
    // … and the alternate kind (effect_loc) does too.
    let loc = server
        .definition(&uri, pos_of(src, "gain_rice"))
        .expect("effect_loc definition via alt kind");
    assert!(
        loc.uri
            .to_file_path()
            .unwrap()
            .ends_with("common/effect_localization/00.txt")
    );

    // Only the name defined in NO kind is diagnosed.
    let publishes = drain_publishes(&rx);
    let ours: Vec<_> = publishes
        .iter()
        .filter(|(p, _)| p.ends_with("common/scripted_effects/e.txt"))
        .collect();
    assert!(
        ours.iter().any(|(_, n)| *n == 1),
        "exactly the missing_entry diagnostic expected: {publishes:?}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn scripted_gui_definition_from_gui_datafn_arg() {
    let t = TempTree::new();
    t.write(
        "common/scripted_guis/sguis.txt",
        "can_pledge = {\n\tscope = character\n\tis_valid = { is_adult = yes }\n\teffect = { }\n}\n",
    );
    let src =
        "window = {\n\tvisible = \"[GetScriptedGui('can_pledge').IsShown( GuiScope.End )]\"\n}\n";
    t.write("gui/window_x.gui", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "gui/window_x.gui");

    let loc = server
        .definition(&uri, pos_of(src, "can_pledge"))
        .expect("scripted_gui definition from datafn argument");
    assert!(
        loc.uri
            .to_file_path()
            .unwrap()
            .ends_with("common/scripted_guis/sguis.txt")
    );
    assert_eq!(
        loc.range.start,
        Position {
            line: 0,
            character: 0
        }
    );
}

#[cfg(feature = "ck3")]
#[test]
fn decision_definition_from_gui_datafn_arg() {
    let t = TempTree::new();
    t.write(
        "common/decisions/rice.txt",
        "T4N_sell_rice_decision = {\n\tpicture = \"x.dds\"\n}\n",
    );
    let src = "button_standard = {\n\
         \tsize = { 120 30 }\n\
         \tdatacontext = \"[GetDecisionWithKey('T4N_sell_rice_decision')]\"\n\
         \tvisible = \"[Decision.IsShownForPlayer]\"\n\
         \tonclick = \"[OpenGameViewData( 'decision_detail', Decision.Self )]\"\n\
         }\n";
    t.write("gui/window_x.gui", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "gui/window_x.gui");

    let loc = server
        .definition(&uri, pos_of(src, "T4N_sell_rice_decision'"))
        .expect("decision definition from GetDecisionWithKey argument");
    assert!(
        loc.uri
            .to_file_path()
            .unwrap()
            .ends_with("common/decisions/rice.txt")
    );
}

#[cfg(feature = "ck3")]
#[test]
fn gui_text_and_tooltip_reference_loc_keys() {
    let t = TempTree::new();
    t.write(
        "localization/english/rice_l_english.yml",
        "l_english:\n T4N_RICE_SELL_BUTTON:0 \"Sell rice\"\n T4N_sell_rice_decision_tooltip:0 \"Sells rice\"\n",
    );
    let src = "button_standard = {\n\
         \ttext = \"T4N_RICE_SELL_BUTTON\"\n\
         \ttooltip = \"T4N_sell_rice_decision_tooltip\"\n\
         \traw_text = \"Just some words\"\n\
         }\n";
    t.write("gui/window_x.gui", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "gui/window_x.gui");

    let loc = server
        .definition(&uri, pos_of(src, "T4N_RICE_SELL_BUTTON"))
        .expect("loc key definition from gui text property");
    assert!(
        loc.uri
            .to_file_path()
            .unwrap()
            .ends_with("rice_l_english.yml")
    );
    let loc = server
        .definition(&uri, pos_of(src, "T4N_sell_rice_decision_tooltip"))
        .expect("loc key definition from gui tooltip property");
    assert!(
        loc.uri
            .to_file_path()
            .unwrap()
            .ends_with("rice_l_english.yml")
    );
    // Hover shows the localized text.
    let hover = hover_md(&server, &uri, pos_of(src, "T4N_RICE_SELL_BUTTON"));
    assert!(hover.contains("Sell rice"), "{hover}");
    // Prose in raw_text is not a reference.
    assert!(server.definition(&uri, pos_of(src, "Just some")).is_none());
}

#[cfg(feature = "ck3")]
#[test]
fn trait_body_completion_offers_fields_and_modifier_tags() {
    let t = TempTree::new();
    let src =
        "brave = {\n\tcategory = personality\n\t\n\ttrack = {\n\t\t50 = {\n\t\t\t\n\t\t}\n\t}\n}\n";
    t.write("common/traits/00.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/traits/00.txt");

    // Body completion: documented fields plus modifier tags (Fallback::Modifier).
    let items = server.completion(
        &uri,
        Position {
            line: 2,
            character: 1,
        },
    );
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for f in ["potential", "opposites", "inherit_chance", "tracks"] {
        assert!(labels.contains(&f), "missing field `{f}`: {}", labels.len());
    }
    assert!(
        labels.contains(&"monthly_prestige") || labels.contains(&"diplomacy"),
        "modifier tags expected via Fallback::Modifier"
    );

    // Inside an XP level (`50 = { … }`), unknown keys are modifiers too.
    let items = server.completion(
        &uri,
        Position {
            line: 5,
            character: 3,
        },
    );
    assert!(
        items
            .iter()
            .any(|i| i.label == "monthly_prestige" || i.label == "diplomacy"),
        "XP-level bodies complete modifier tags"
    );

    // Hover on `category` documents the field.
    let hover = hover_md(&server, &uri, pos_of(src, "category"));
    assert!(hover.contains("trait field category"), "{hover}");
}

#[cfg(feature = "ck3")]
#[test]
fn game_concept_body_completion_and_hover() {
    let t = TempTree::new();
    let src = "vassal = { }\ndirect_vassal = {\n\tparent = vassal\n\t\n}\n";
    t.write("common/game_concepts/00.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/game_concepts/00.txt");

    // Body completion offers the documented, closed field set.
    let items = server.completion(
        &uri,
        Position {
            line: 3,
            character: 1,
        },
    );
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for f in ["parent", "texture", "framesize", "shown_in_encyclopedia"] {
        assert!(labels.contains(&f), "missing field `{f}`: {labels:?}");
    }

    // Hover on `parent` documents the field.
    let hover = hover_md(&server, &uri, pos_of(src, "parent = "));
    assert!(hover.contains("game_concept field parent"), "{hover}");
}

#[cfg(feature = "ck3")]
#[test]
fn smart_doc_references_participate_in_find_references_and_code_lens() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    let src = "#! Grants ![brave] - see also ![trait:brave].\ne = { add_trait = brave }\n";
    t.write("common/scripted_effects/e.txt", src);
    let (server, _rx) = server_over(&t);

    // From the script reference site: the `add_trait` value plus both doc refs.
    let script = uri_for(&t, "common/scripted_effects/e.txt");
    let locs = server.references(&script, pos_of(src, "brave }"), false);
    assert_eq!(locs.len(), 3, "script ref + two doc refs: {locs:?}");

    // The count surfaced on the definition's lens agrees.
    let traits = uri_for(&t, "common/traits/00.txt");
    let lenses = server.code_lens(&traits);
    let cmd = server
        .code_lens_resolve(lenses[0].clone())
        .command
        .expect("resolve fills the command");
    assert_eq!(cmd.title, "3 references");
}

#[cfg(feature = "ck3")]
#[test]
fn subject_contract_levels_link_to_their_implicit_loc_keys() {
    let t = TempTree::new();
    let src = "feudal_government_taxes = {\n\
               \tobligation_levels = {\n\
               \t\tvassal_tax_normal = { tax = 0.2 }\n\
               \t}\n\
               }\n";
    t.write("common/subject_contracts/contracts/feudal.txt", src);
    t.write(
        "localization/english/x_l_english.yml",
        "\u{feff}l_english:\n \
         feudal_government_taxes: \"Taxes\"\n \
         vassal_tax_normal: \"Normal Tax\"\n \
         vassal_tax_normal_short: \"Normal\"\n \
         vassal_tax_normal_desc: \"A fifth of income.\"\n",
    );
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/subject_contracts/contracts/feudal.txt");

    // Hovering the level offers links to all three of its loc keys.
    let md = hover_md(&server, &uri, pos_of(src, "vassal_tax_normal = {"));
    for key in [
        "vassal_tax_normal",
        "vassal_tax_normal_short",
        "vassal_tax_normal_desc",
    ] {
        assert!(
            md.contains(&format!("[{key}]")),
            "{key} missing from:\n{md}"
        );
    }

    // Hovering the contract links to its own name key.
    let md = hover_md(&server, &uri, pos_of(src, "feudal_government_taxes"));
    assert!(
        md.contains("[feudal_government_taxes]"),
        "contract loc link missing from:\n{md}"
    );

    // The reverse edge: the loc key's references include the level using it.
    let loc = uri_for(&t, "localization/english/x_l_english.yml");
    let loc_src = std::fs::read_to_string(t.child("localization/english/x_l_english.yml")).unwrap();
    let refs = server.references(&loc, pos_of(&loc_src, "vassal_tax_normal_desc"), false);
    assert!(
        refs.iter()
            .any(|l| l.uri.to_file_path().unwrap().ends_with("feudal.txt")),
        "loc key should list the obligation level: {refs:?}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn casus_belli_links_to_its_implicit_loc_keys() {
    let t = TempTree::new();
    let src = "claim_cb = {\n\tgroup = claim\n}\n";
    t.write("common/casus_belli_types/00_cb.txt", src);
    // The CB's own key names it; the outcome descriptions suffix that key,
    // optionally by the side reading them. `_defeat_desc` is deliberately
    // absent — an unmatched pattern must simply not appear.
    t.write(
        "localization/english/x_l_english.yml",
        "\u{feff}l_english:\n \
         claim_cb: \"Claim War\"\n \
         claim_cb_victory_desc: \"You win.\"\n \
         claim_cb_victory_desc_attacker: \"We win.\"\n \
         claim_cb_white_peace_desc_defender: \"They relent.\"\n",
    );
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/casus_belli_types/00_cb.txt");

    let md = hover_md(&server, &uri, pos_of(src, "claim_cb"));
    for key in [
        "claim_cb",
        "claim_cb_victory_desc",
        "claim_cb_victory_desc_attacker",
        "claim_cb_white_peace_desc_defender",
    ] {
        assert!(
            md.contains(&format!("[{key}]")),
            "{key} missing from:\n{md}"
        );
    }
    assert!(
        !md.contains("[claim_cb_defeat_desc]"),
        "a pattern with no matching key must not be offered:\n{md}"
    );

    // The reverse edge: the loc key's references include the CB using it.
    let loc = uri_for(&t, "localization/english/x_l_english.yml");
    let loc_src = std::fs::read_to_string(t.child("localization/english/x_l_english.yml")).unwrap();
    let refs = server.references(&loc, pos_of(&loc_src, "claim_cb_victory_desc:"), false);
    assert!(
        refs.iter()
            .any(|l| l.uri.to_file_path().unwrap().ends_with("00_cb.txt")),
        "loc key should list the casus belli: {refs:?}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn smart_doc_anchors_are_symbols_referenced_across_files() {
    let t = TempTree::new();
    // The anchor names something the schema has no row for at all.
    let decl = "#! @todo:rebalance_piety rework the piety curve\n\
                fx = { add_trait = brave }\n";
    t.write("common/scripted_effects/a.txt", decl);
    t.write(
        "common/scripted_effects/b.txt",
        "#! blocked on ![todo:rebalance_piety]\ne = { }\n",
    );
    t.write("common/traits/00.txt", "brave = { }\n");
    let (server, _rx) = server_over(&t);
    let a = uri_for(&t, "common/scripted_effects/a.txt");
    let b = uri_for(&t, "common/scripted_effects/b.txt");

    // Hovering the declaration names the kind and shows the description.
    let md = hover_md(&server, &a, pos_of(decl, "todo:rebalance_piety"));
    assert!(md.contains("doc_anchor"), "kind missing from:\n{md}");
    assert!(
        md.contains("rework the piety curve"),
        "description missing from:\n{md}"
    );

    // A key alone on its line takes the `#!` lines beneath it instead — the
    // layout a multi-line note naturally falls into.
    let below = "#! @regency_system\n\
                 #! Diarchy, regents, and the removal war.\n\
                 #! Root is the regent.\n\
                 rg = { }\n";
    t.write("common/scripted_effects/c.txt", below);
    let (server2, _rx2) = server_over(&t);
    let c = uri_for(&t, "common/scripted_effects/c.txt");
    let md = hover_md(&server2, &c, pos_of(below, "regency_system"));
    assert!(
        md.contains("Diarchy, regents") && md.contains("Root is the regent"),
        "block description missing from:\n{md}"
    );

    // A reference in another file jumps back to the declaration.
    let b_src = std::fs::read_to_string(t.child("common/scripted_effects/b.txt")).unwrap();
    let loc = server
        .definition(&b, pos_of(&b_src, "todo:rebalance_piety"))
        .expect("anchor definition");
    assert!(
        loc.uri.to_file_path().unwrap().ends_with("a.txt"),
        "expected a.txt, got {loc:?}"
    );

    // And find-references from the declaration sees the other file's use.
    let refs = server.references(&a, pos_of(decl, "todo:rebalance_piety"), false);
    assert!(
        refs.iter()
            .any(|l| l.uri.to_file_path().unwrap().ends_with("b.txt")),
        "anchor reference not found: {refs:?}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn unresolved_anchor_reference_is_published_as_a_warning() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\n");
    t.write(
        "common/scripted_effects/e.txt",
        "#! @todo:real\n\
         #! blocked on ![@todo:typo]\n\
         e = { add_trait = missing }\n",
    );
    let (_server, rx) = server_over(&t);

    // The shared `drain_publishes` keeps only counts; severity is the point here.
    let mut severities = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let lsp_server::Message::Notification(n) = msg
            && n.method == "textDocument/publishDiagnostics"
        {
            let p: lsp_types::PublishDiagnosticsParams = serde_json::from_value(n.params).unwrap();
            for d in p.diagnostics {
                severities.push((d.severity, d.message));
            }
        }
    }
    let anchor = severities
        .iter()
        .find(|(_, m)| m.contains("doc_anchor"))
        .expect("anchor diagnostic");
    assert_eq!(
        anchor.0,
        Some(lsp_types::DiagnosticSeverity::WARNING),
        "a stale doc link must not read as an error: {anchor:?}"
    );
    // A genuine script reference in the same file stays an error.
    let script = severities
        .iter()
        .find(|(_, m)| m.contains("missing"))
        .expect("trait diagnostic");
    assert_eq!(script.0, Some(lsp_types::DiagnosticSeverity::ERROR));
}

#[cfg(feature = "ck3")]
#[test]
fn a_doc_anchor_outranks_an_entity_of_the_same_name() {
    let t = TempTree::new();
    // `brave` is a real trait; the author deliberately declares an anchor
    // shadowing it, and the deliberate declaration must win.
    t.write("common/traits/00.txt", "brave = { }\n");
    let src = "#! @brave the courage rework, not the trait\n\
               #! see ![brave]\n\
               e = { }\n";
    t.write("common/scripted_effects/a.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/a.txt");

    // `brave]` occurs only inside the reference, so this lands on the name.
    let md = hover_md(&server, &uri, pos_of(src, "brave]"));
    assert!(
        md.contains("doc_anchor"),
        "anchor should outrank the trait:\n{md}"
    );
}

#[cfg(feature = "ck3")]
#[test]
fn smart_doc_refs_prefer_entities_then_concepts_then_loc() {
    let t = TempTree::new();
    // One name that is a law group, a game concept, AND a loc key.
    t.write(
        "common/laws/00_laws.txt",
        "budget_allocation_military_law = {\n\tbudget_allocation_military_20 = { }\n}\n\
         another_group = { another_law = { } }\n",
    );
    t.write(
        "common/game_concepts/00.txt",
        "budget_allocation_military_law = { }\nsome_concept = { }\n",
    );
    t.write(
        "localization/english/x_l_english.yml",
        "\u{feff}l_english:\n \
         budget_allocation_military_law: \"Military Budget\"\n \
         some_concept: \"Concept\"\n \
         plain_text_key: \"Just Text\"\n",
    );
    // Cursor targets are the *name*, never the `kind:` qualifier — extraction
    // records only the name, so hovering the prefix finds nothing by design.
    let src = "#! Bare ![budget_allocation_military_law] and pinned \
               ![law_group:another_group] and ![law:budget_allocation_military_20].\n\
               #! Concept ![some_concept], loc-only ![plain_text_key].\n\
               e = { }\n";
    t.write("common/scripted_effects/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/e.txt");

    // Entity beats both the concept and the loc key sharing its name.
    let md = hover_md(
        &server,
        &uri,
        pos_of(src, "budget_allocation_military_law]"),
    );
    assert!(md.contains("law_group"), "entity should win:\n{md}");

    // Explicit prefixes pin the kind.
    let md = hover_md(&server, &uri, pos_of(src, "another_group]"));
    assert!(md.contains("law_group"), "law_group: prefix:\n{md}");
    let md = hover_md(&server, &uri, pos_of(src, "budget_allocation_military_20]"));
    assert!(md.contains("law "), "law: prefix:\n{md}");

    // Concept beats a loc key of the same name; a loc-only name still resolves.
    let md = hover_md(&server, &uri, pos_of(src, "some_concept]"));
    assert!(md.contains("game_concept"), "concept should win:\n{md}");
    let md = hover_md(&server, &uri, pos_of(src, "plain_text_key]"));
    assert!(md.contains("loc_key"), "loc fallback:\n{md}");
}

#[cfg(feature = "ck3")]
#[test]
fn smart_doc_completion_offers_prefixes_then_narrows_by_kind() {
    let t = TempTree::new();
    t.write("common/traits/00.txt", "brave = { }\nbrash = { }\n");
    t.write(
        "common/laws/00.txt",
        "crown_authority = { crown_authority_0 = { } }\n",
    );
    t.write(
        "localization/english/x_l_english.yml",
        "\u{feff}l_english:\n brave: \"Brave\"\n bravado_key: \"Bravado\"\n",
    );
    let labels = |server: &ServerState, uri: &Url, src: &str, needle: &str| -> Vec<String> {
        server
            .completion(uri, pos_after(src, needle))
            .into_iter()
            .map(|i| i.label)
            .collect()
    };

    // 1. `![` with nothing typed: only the `alias:` qualifiers, so the author
    //    can narrow before knowing a name.
    let src = "#! See ![\ne = { }\n";
    t.write("common/scripted_effects/a.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/a.txt");
    let l = labels(&server, &uri, src, "![");
    assert!(l.contains(&"trait:".to_string()), "{l:?}");
    assert!(l.contains(&"law:".to_string()), "{l:?}");
    assert!(
        l.iter().all(|s| s.ends_with(':')),
        "no bare names before narrowing: {l:?}"
    );

    // 2. Typing narrows to matching qualifiers *and* matching entities, but
    //    never localization — 279k loc keys would swamp the list.
    let src = "#! See ![bra\ne = { }\n";
    t.write("common/scripted_effects/b.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/b.txt");
    let l = labels(&server, &uri, src, "![bra");
    for want in ["brave", "brash"] {
        assert!(l.contains(&want.to_string()), "{want} missing: {l:?}");
    }
    assert!(
        !l.contains(&"bravado_key".to_string()),
        "loc keys excluded unless asked for: {l:?}"
    );

    // 3. A `kind:` qualifier restricts to that kind and drops the qualifiers.
    let src = "#! See ![law:\ne = { }\n";
    t.write("common/scripted_effects/c.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/c.txt");
    let l = labels(&server, &uri, src, "![law:");
    assert_eq!(l, vec!["crown_authority_0".to_string()], "{l:?}");

    // 4. `loc:` is how localization is reached deliberately.
    let src = "#! See ![loc:bra\ne = { }\n";
    t.write("common/scripted_effects/d.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/d.txt");
    let l = labels(&server, &uri, src, "![loc:bra");
    assert!(l.contains(&"bravado_key".to_string()), "{l:?}");

    // 5. Outside a `#!` comment nothing of this fires.
    let src = "# plain ![bra\ne = { }\n";
    t.write("common/scripted_effects/e.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/scripted_effects/e.txt");
    let l = labels(&server, &uri, src, "![bra");
    assert!(!l.contains(&"brave".to_string()), "plain comment: {l:?}");
}

#[cfg(feature = "ck3")]
#[test]
fn engine_intrinsic_message_explains_its_empty_reference_list() {
    let t = TempTree::new();
    // `msg_siege_won` is raised by the siege code — a string in the game
    // binary — so nothing in script ever names it.
    let src = "msg_siege_won = { icon = \"siege\" style = good }\n\
               msg_scripted = { icon = \"x\" style = good }\n";
    t.write("common/messages/00.txt", src);
    let (server, _rx) = server_over(&t);
    let uri = uri_for(&t, "common/messages/00.txt");

    let md = hover_md(&server, &uri, pos_of(src, "msg_siege_won"));
    assert!(
        md.contains("Engine intrinsic"),
        "intrinsic marking missing from:\n{md}"
    );
    // An ordinary message with no references says nothing extra — the marking
    // must distinguish the two cases, not label every message.
    let md = hover_md(&server, &uri, pos_of(src, "msg_scripted"));
    assert!(
        !md.contains("Engine intrinsic"),
        "ordinary message must not be marked:\n{md}"
    );
}
