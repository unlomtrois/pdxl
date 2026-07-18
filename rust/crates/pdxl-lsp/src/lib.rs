//! PDXScript language server — port of `internal/lsp` (Milestone 8a scope:
//! handshake, full-text document sync, debounced mod-scoped diagnostics,
//! go-to-definition).
//!
//! Architecture: a **single event loop thread** owns all state
//! ([`state::ServerState`]) and selects over two channels — LSP messages from
//! the client (via `lsp-server`'s stdio transport) and internal [`Event`]s from
//! background threads (the async initial project build; per-edit debounce
//! timers). Handlers are sub-millisecond in-memory lookups, so no async
//! runtime is used; the one slow operation (the initial build, seconds at CK3
//! scale) runs off-thread exactly like Go's `initialized` goroutine, keeping
//! the handshake fast.
//!
//! M8b adds references (Go parity), plus document outline and hover — the
//! first features the Go server does not have. Deviations from Go
//! (documented): no AST/facts caches in the build (measured in
//! `docs/BASELINE.md`: cold build ≈ 4 s once per session; the caches don't
//! pay).

#[macro_use]
mod log;
mod completion;
mod position;
mod state;

pub use position::{offset_to_position, position_to_offset};
pub use state::{DEBOUNCE_MS, Event, ServerState};

use std::path::PathBuf;
use std::time::Duration;

use crossbeam_channel::{Sender, unbounded};
use lsp_server::{Connection, Message, Response};
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, InitializeParams,
    OneOf, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
};
use pdxl_ck3::schema;
use pdxl_fileset::{FileKind, FileSet};
use pdxl_project::Project;

/// Server configuration (Go `Options`, minus the config file for now).
#[derive(Default)]
pub struct Options {
    /// Vanilla game directory; may also arrive via `initializationOptions`.
    pub game_path: Option<String>,
    /// Log level: error | warn | info | debug (stderr → "pdxl (server)").
    pub log_level: String,
}

/// Go `config.Default()` scan ignores (shared with the CLI's `check`).
const IGNORE_DIRS: &[&str] = &["licenses"];
const IGNORE_FILES: &[&str] = &[
    "credits.txt",
    "checksum_manifest.txt",
    "guids.txt",
    "license.txt",
    "ofl.txt",
];

/// Builds the project FileSet: vanilla + mod root, default ignores
/// (Go `buildProject`, cache-free per the measured decision).
pub fn build_project(game: Option<&str>, mod_dir: Option<&str>) -> std::io::Result<Project> {
    let mut fs = FileSet::new();
    fs.set_ignore(IGNORE_DIRS, IGNORE_FILES);
    fs.set_localization_language(pdxl_project::DEFAULT_LOC_LANGUAGE);
    if let Some(game) = game {
        fs.add(game, FileKind::Vanilla)?;
    }
    if let Some(mod_dir) = mod_dir {
        fs.add(mod_dir, FileKind::Mod)?;
    }
    Project::new(&fs, schema())
}

/// Serves the LSP over stdio until the client disconnects. Blocks.
pub fn run_stdio(opts: Options) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    log::init(&opts.log_level);
    log_info!("pdxl-lsp starting (rust)");
    let (connection, io_threads) = Connection::stdio();

    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(lsp_types::CompletionOptions {
            // Scope links are written as `title:name`; request completion as
            // soon as the colon is typed instead of waiting for Ctrl+Space.
            trigger_characters: Some(vec!["=".to_string(), ":".to_string(), ".".to_string()]),
            ..lsp_types::CompletionOptions::default()
        }),
        definition_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        // Reference-count lenses over every definition; two-phase (resolve
        // fills the count lazily for on-screen lenses only).
        code_lens_provider: Some(lsp_types::CodeLensOptions {
            resolve_provider: Some(true),
        }),
        document_symbol_provider: Some(OneOf::Left(true)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        inlay_hint_provider: Some(OneOf::Right(
            lsp_types::InlayHintServerCapabilities::Options(lsp_types::InlayHintOptions {
                resolve_provider: Some(false),
                work_done_progress_options: Default::default(),
            }),
        )),
        ..ServerCapabilities::default()
    };
    // `initialize` must answer fast (clients time out); the project build is
    // deferred to a background thread below.
    //
    // NOTE: use the two-step handshake, NOT `Connection::initialize` — that
    // helper wraps its argument in `{"capabilities": ...}` itself. Passing a
    // pre-wrapped InitializeResult double-nests the capabilities, and a
    // spec-respecting client (vscode-languageclient) then sees no declared
    // sync/providers and never sends a single textDocument notification.
    // (Field-tested the hard way; hand-rolled smoke clients that ignore the
    // handshake cannot catch this.)
    let (init_id, init_params) = connection.initialize_start()?;
    let init: InitializeParams = serde_json::from_value(init_params)?;
    connection.initialize_finish(
        init_id,
        serde_json::json!({
            "capabilities": capabilities,
            "serverInfo": { "name": "pdxl" },
        }),
    )?;

    // Game dir: --game flag, overridable by initializationOptions.gamePath.
    let mut game = opts.game_path.clone();
    if let Some(g) = init
        .initialization_options
        .as_ref()
        .and_then(|o| o.get("gamePath"))
        .and_then(|v| v.as_str())
        && !g.is_empty()
    {
        game = Some(g.to_string());
    }
    // Mod dir: the workspace root (Go parity).
    #[allow(deprecated)]
    let mod_dir: Option<PathBuf> = init.root_uri.as_ref().map(position::uri_to_path);

    let (events_tx, events_rx) = unbounded::<Event>();
    let mut server = ServerState::new(mod_dir.clone(), connection.sender.clone());

    log_info!(
        "initialize: game={} mod={}",
        game.as_deref().unwrap_or("(none)"),
        mod_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(none)".into())
    );

    // Async initial build (Go: the goroutine in `initialized`).
    {
        let events_tx = events_tx.clone();
        let game = game.clone();
        let mod_dir = mod_dir.clone();
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let project = build_project(
                game.as_deref(),
                mod_dir.as_ref().map(|p| p.to_string_lossy()).as_deref(),
            );
            log_info!("project build finished in {:.1?}", started.elapsed());
            let _ = events_tx.send(Event::ProjectReady(project.map(Box::new)));
        });
    }

    // The event loop: LSP traffic and internal events, one thread, no locks.
    loop {
        crossbeam_channel::select! {
            recv(connection.receiver) -> msg => {
                let Ok(msg) = msg else { break };
                match msg {
                    Message::Request(req) => {
                        if connection.handle_shutdown(&req)? {
                            break;
                        }
                        handle_request(&mut server, &connection.sender, req);
                    }
                    Message::Notification(note) => {
                        handle_notification(&mut server, &events_tx, note);
                    }
                    Message::Response(_) => {}
                }
            }
            recv(events_rx) -> event => {
                let Ok(event) = event else { break };
                match event {
                    Event::ProjectReady(project) => server.project_ready(project),
                    Event::Debounce { path, generation } => {
                        server.debounce_fired(&path, generation)
                    }
                }
            }
        }
    }

    // Release every clone of the outgoing sender before joining: the writer
    // thread exits only when the channel closes, and ServerState holds a
    // Sender (as do any still-sleeping debounce threads via events_tx, which
    // is fine — events_rx is ours and already out of scope after the loop).
    drop(server);
    drop(connection);
    io_threads.join()?;
    Ok(())
}

fn handle_request(server: &mut ServerState, out: &Sender<Message>, req: lsp_server::Request) {
    match req.method.as_str() {
        lsp_types::request::GotoDefinition::METHOD => {
            let resp = match serde_json::from_value::<GotoDefinitionParams>(req.params) {
                Ok(params) => {
                    let location = server.definition(
                        &params.text_document_position_params.text_document.uri,
                        params.text_document_position_params.position,
                    );
                    let result = location.map(GotoDefinitionResponse::Scalar);
                    Response::new_ok(req.id, result)
                }
                Err(e) => Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    e.to_string(),
                ),
            };
            let _ = out.send(Message::Response(resp));
        }
        lsp_types::request::References::METHOD => {
            let resp = match serde_json::from_value::<lsp_types::ReferenceParams>(req.params) {
                Ok(params) => {
                    let locations = server.references(
                        &params.text_document_position.text_document.uri,
                        params.text_document_position.position,
                        params.context.include_declaration,
                    );
                    // Go parity: an empty result is null, not [].
                    let result = (!locations.is_empty()).then_some(locations);
                    Response::new_ok(req.id, result)
                }
                Err(e) => Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    e.to_string(),
                ),
            };
            let _ = out.send(Message::Response(resp));
        }
        lsp_types::request::CodeLensRequest::METHOD => {
            let resp = match serde_json::from_value::<lsp_types::CodeLensParams>(req.params) {
                Ok(params) => {
                    let lenses = server.code_lens(&params.text_document.uri);
                    let result = (!lenses.is_empty()).then_some(lenses);
                    Response::new_ok(req.id, result)
                }
                Err(e) => Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    e.to_string(),
                ),
            };
            let _ = out.send(Message::Response(resp));
        }
        lsp_types::request::CodeLensResolve::METHOD => {
            let resp = match serde_json::from_value::<lsp_types::CodeLens>(req.params) {
                Ok(lens) => Response::new_ok(req.id, server.code_lens_resolve(lens)),
                Err(e) => Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    e.to_string(),
                ),
            };
            let _ = out.send(Message::Response(resp));
        }
        lsp_types::request::DocumentSymbolRequest::METHOD => {
            let resp = match serde_json::from_value::<lsp_types::DocumentSymbolParams>(req.params) {
                Ok(params) => {
                    let symbols = server.document_symbol(&params.text_document.uri);
                    let result = (!symbols.is_empty())
                        .then_some(lsp_types::DocumentSymbolResponse::Nested(symbols));
                    Response::new_ok(req.id, result)
                }
                Err(e) => Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    e.to_string(),
                ),
            };
            let _ = out.send(Message::Response(resp));
        }
        lsp_types::request::Completion::METHOD => {
            let resp = match serde_json::from_value::<lsp_types::CompletionParams>(req.params) {
                Ok(params) => {
                    let items = server.completion(
                        &params.text_document_position.text_document.uri,
                        params.text_document_position.position,
                    );
                    Response::new_ok(req.id, lsp_types::CompletionResponse::Array(items))
                }
                Err(e) => Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    e.to_string(),
                ),
            };
            let _ = out.send(Message::Response(resp));
        }
        lsp_types::request::Formatting::METHOD => {
            let resp =
                match serde_json::from_value::<lsp_types::DocumentFormattingParams>(req.params) {
                    Ok(params) => {
                        let edits = server.formatting(&params.text_document.uri);
                        Response::new_ok(req.id, edits)
                    }
                    Err(e) => Response::new_err(
                        req.id,
                        lsp_server::ErrorCode::InvalidParams as i32,
                        e.to_string(),
                    ),
                };
            let _ = out.send(Message::Response(resp));
        }
        lsp_types::request::HoverRequest::METHOD => {
            let resp = match serde_json::from_value::<lsp_types::HoverParams>(req.params) {
                Ok(params) => {
                    let hover = server.hover(
                        &params.text_document_position_params.text_document.uri,
                        params.text_document_position_params.position,
                    );
                    Response::new_ok(req.id, hover)
                }
                Err(e) => Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    e.to_string(),
                ),
            };
            let _ = out.send(Message::Response(resp));
        }
        lsp_types::request::InlayHintRequest::METHOD => {
            let resp = match serde_json::from_value::<lsp_types::InlayHintParams>(req.params) {
                Ok(params) => Response::new_ok(
                    req.id,
                    server.inlay_hints(&params.text_document.uri, params.range),
                ),
                Err(e) => Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    e.to_string(),
                ),
            };
            let _ = out.send(Message::Response(resp));
        }
        _ => {
            // Politely refuse anything we didn't declare a capability for.
            let resp = Response::new_err(
                req.id,
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("unhandled method {}", req.method),
            );
            let _ = out.send(Message::Response(resp));
        }
    }
}

fn handle_notification(
    server: &mut ServerState,
    events_tx: &Sender<Event>,
    note: lsp_server::Notification,
) {
    match note.method.as_str() {
        lsp_types::notification::DidOpenTextDocument::METHOD => {
            if let Ok(p) = serde_json::from_value::<DidOpenTextDocumentParams>(note.params) {
                server.did_open(p.text_document.uri, p.text_document.text);
            }
        }
        lsp_types::notification::DidChangeTextDocument::METHOD => {
            if let Ok(mut p) = serde_json::from_value::<DidChangeTextDocumentParams>(note.params) {
                // Full sync: the last change carries the whole document.
                let Some(change) = p.content_changes.pop() else {
                    return;
                };
                if let Some((path, generation)) =
                    server.did_change(p.text_document.uri, change.text)
                {
                    // Arm the debounce: a sleeper thread posts back into the
                    // loop; stale generations are ignored on arrival.
                    let events_tx = events_tx.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(DEBOUNCE_MS));
                        let _ = events_tx.send(Event::Debounce { path, generation });
                    });
                }
            }
        }
        lsp_types::notification::DidSaveTextDocument::METHOD => {
            if let Ok(p) = serde_json::from_value::<DidSaveTextDocumentParams>(note.params) {
                server.did_save(p.text_document.uri);
            }
        }
        lsp_types::notification::DidCloseTextDocument::METHOD => {
            if let Ok(p) = serde_json::from_value::<DidCloseTextDocumentParams>(note.params) {
                server.did_close(p.text_document.uri);
            }
        }
        _ => {}
    }
}
