//! The server state and feature handlers — Go's `Server` struct, translated
//! into a single-threaded event-loop design.
//!
//! Where Go guards `proj`/`docs`/`published` with a non-reentrant mutex (and a
//! fragile `readFile` vs `readFileLocked` convention whose misuse deadlocks),
//! here exactly one thread — the event loop in `lib.rs` — owns `ServerState`.
//! Background work (the initial project build, debounce timers) communicates
//! by *sending events into the loop*, never by touching state. The class of
//! bug the Go comments warn about cannot be written: `&mut self` methods
//! cannot re-enter, and no other thread can reach the state at all.
//!
//! Outgoing messages go through an injected channel sender, so tests drive
//! handlers directly and inspect what would have been sent to the client
//! (mirroring Go's captured-notify test pattern).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crossbeam_channel::Sender;
use lsp_server::{Message, Notification};
use lsp_types::notification::{Notification as _, PublishDiagnostics};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, InlayHint, InlayHintKind, InlayHintTooltip, Location, Position,
    PublishDiagnosticsParams, Range, Url,
};
use pdxl_analysis::RefDiag;
use pdxl_analysis::context::ClauseKind;
use pdxl_project::Project;

use crate::position::{offset_to_position, path_to_uri, position_to_offset, uri_to_path};

/// Coalesces rapid edits before re-analyzing (Go: `debounceDelay`).
pub const DEBOUNCE_MS: u64 = 200;

/// Events posted into the main loop by background threads.
pub enum Event {
    /// The async initial build finished (Go: the goroutine in `initialized`).
    /// Boxed: `Project` is large and events travel through channels by value.
    ProjectReady(std::io::Result<Box<Project>>),
    /// A debounce timer fired for a document; stale generations are ignored.
    Debounce { path: PathBuf, generation: u64 },
}

/// One open document: its current buffer and debounce bookkeeping.
struct Doc {
    text: Vec<u8>,
    /// Debounce generation; a fired timer only acts if it carries the latest.
    generation: u64,
}

/// The language server's whole state. Owned by the event loop thread.
pub struct ServerState {
    /// The mod root from `initialize` (workspace root). Diagnostics are only
    /// published for files under it; empty = everything in scope (tests).
    mod_root: Option<PathBuf>,
    project: Option<Project>,
    /// Open documents keyed by cleaned path.
    docs: HashMap<PathBuf, Doc>,
    /// Files currently showing diagnostics (for the clear cycle).
    published: HashSet<PathBuf>,
    /// Outgoing messages to the client.
    out: Sender<Message>,
}

impl ServerState {
    pub fn new(mod_root: Option<PathBuf>, out: Sender<Message>) -> ServerState {
        ServerState {
            mod_root: mod_root.map(|p| PathBuf::from(pdxl_path::clean(&p.to_string_lossy()))),
            project: None,
            docs: HashMap::new(),
            published: HashSet::new(),
            out,
        }
    }

    /// The async build completed. Re-analyze any documents opened while it ran
    /// (their buffers override disk), then publish for every mod file.
    pub fn project_ready(&mut self, project: std::io::Result<Box<Project>>) {
        let mut project = match project {
            Ok(p) => *p,
            Err(e) => {
                log_error!("failed to build project: {e}");
                return;
            }
        };
        for (path, doc) in &self.docs {
            if let Err(e) = project.update_source(path, doc.text.clone()) {
                log_warn!("post-build update failed for {}: {e}", path.display());
            }
        }
        self.project = Some(project);
        log_info!(
            "project ready: {} symbols, {} diagnostics, {} open docs",
            self.project.as_ref().unwrap().table().total(),
            self.project.as_ref().unwrap().diags().len(),
            self.docs.len()
        );
        self.publish_project_diagnostics();
    }

    /// `textDocument/didOpen`: store the buffer and analyze immediately.
    pub fn did_open(&mut self, uri: Url, text: String) {
        let path = uri_to_path(&uri);
        log_debug!("didOpen {}", path.display());
        self.docs.insert(
            path.clone(),
            Doc {
                text: text.into_bytes(),
                generation: 0,
            },
        );
        self.analyze_and_publish(&path);
    }

    /// `textDocument/didChange` (full sync): store the buffer and bump the
    /// debounce generation. Returns the (path, generation) the caller should
    /// arm a timer for; analysis happens when that timer's event arrives.
    pub fn did_change(&mut self, uri: Url, text: String) -> Option<(PathBuf, u64)> {
        let path = uri_to_path(&uri);
        let doc = self.docs.entry(path.clone()).or_insert(Doc {
            text: Vec::new(),
            generation: 0,
        });
        doc.text = text.into_bytes();
        doc.generation += 1;
        log_debug!("didChange {} (gen {})", path.display(), doc.generation);
        Some((path, doc.generation))
    }

    /// A debounce timer fired; acts only if it carries the latest generation.
    pub fn debounce_fired(&mut self, path: &Path, generation: u64) {
        match self.docs.get(path) {
            Some(doc) if doc.generation == generation => {
                log_debug!("debounce fired for {} (gen {generation})", path.display());
                self.analyze_and_publish(path);
            }
            _ => log_debug!(
                "debounce stale for {} (gen {generation}) — skipped",
                path.display()
            ),
        }
    }

    /// `textDocument/didSave`: analyze immediately (Go parity).
    pub fn did_save(&mut self, uri: Url) {
        self.analyze_and_publish(&uri_to_path(&uri));
    }

    /// `textDocument/didClose`: drop the buffer and re-analyze from disk, so
    /// the file reverts to its on-disk diagnostics rather than being cleared.
    pub fn did_close(&mut self, uri: Url) {
        let path = uri_to_path(&uri);
        self.docs.remove(&path);
        if let Some(project) = &mut self.project {
            let _ = project.update(&path);
        }
        self.publish_project_diagnostics();
    }

    /// Re-analyzes the changed document from its buffer, then republishes for
    /// every mod file — an edit to a definition can change references in other
    /// files, opened or not (Go: `analyzeAndPublish`).
    fn analyze_and_publish(&mut self, path: &Path) {
        let Some(project) = &mut self.project else {
            log_debug!("analyze skipped, project not ready: {}", path.display());
            return; // project not ready; project_ready will catch up
        };
        if let Some(doc) = self.docs.get(path)
            && let Err(e) = project.update_source(path, doc.text.clone())
        {
            // The most common cause: a file created after the initial scan —
            // the FileSet doesn't track it (reload the window to rescan).
            log_warn!(
                "update failed for {}: {e} (new files need a window reload)",
                path.display()
            );
        }
        self.publish_project_diagnostics();
    }

    /// Publishes unresolved-reference diagnostics for every mod file that has
    /// them, clears files that no longer do, and records the current set.
    /// Vanilla files are analyzed but never flagged (Go parity).
    fn publish_project_diagnostics(&mut self) {
        let Some(project) = &self.project else {
            return;
        };
        // Group by file, mod-scoped. BTreeMap: deterministic publish order
        // (Go iterates a map here; order was unspecified — this is stricter).
        let mut by_file: BTreeMap<PathBuf, Vec<&RefDiag>> = BTreeMap::new();
        for d in project.diags() {
            let path = PathBuf::from(&d.file);
            if !self.under_mod_root(&path) {
                continue;
            }
            by_file.entry(path).or_default().push(d);
        }

        for (file, file_diags) in &by_file {
            let Ok(text) = self.read_file(file) else {
                continue;
            };
            let diags = file_diags
                .iter()
                .map(|d| Diagnostic {
                    range: Range {
                        start: offset_to_position(&text, d.start),
                        end: offset_to_position(&text, d.end),
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("pdxl".to_string()),
                    message: d.msg.clone(),
                    ..Diagnostic::default()
                })
                .collect();
            self.publish(file, diags);
        }

        // Clear files that had diagnostics last cycle but no longer do.
        let stale: Vec<PathBuf> = self
            .published
            .iter()
            .filter(|f| !by_file.contains_key(*f))
            .cloned()
            .collect();
        for file in stale {
            self.publish(&file, Vec::new());
        }

        log_debug!("published diagnostics for {} file(s)", by_file.len());
        self.published = by_file.into_keys().collect();
    }

    fn publish(&self, file: &Path, diagnostics: Vec<Diagnostic>) {
        let params = PublishDiagnosticsParams {
            uri: path_to_uri(file),
            diagnostics,
            version: None,
        };
        let _ = self.out.send(Message::Notification(Notification::new(
            PublishDiagnostics::METHOD.to_string(),
            params,
        )));
    }

    /// `textDocument/definition`: the reference under the cursor, resolved to
    /// its defining symbol's location. Every "no result" branch is `None`,
    /// which the editor treats as "no definition" (Go parity).
    pub fn definition(&self, uri: &Url, pos: Position) -> Option<Location> {
        let path = uri_to_path(uri);
        let project = self.project.as_ref()?;
        let src = self.read_file(&path).ok()?;
        let off = position_to_offset(&src, pos);

        let facts = project.facts_at(&path)?;
        let reference = facts.refs.iter().find(|r| r.start <= off && off < r.end)?;

        let symbol = project.table().lookup(reference.kind, &reference.name)?;
        let def_full = project.rel_to_full(&symbol.file)?.to_path_buf();
        let def_src = self.read_file(&def_full).ok()?;

        Some(Location {
            uri: path_to_uri(&def_full),
            range: Range {
                start: offset_to_position(&def_src, symbol.offset),
                end: offset_to_position(&def_src, symbol.end_offset),
            },
        })
    }

    /// `textDocument/references`: resolves the symbol under the cursor
    /// (definitions first — the cursor on a `NAME = {}` name finds that
    /// symbol's references) and returns every reference across the project in
    /// walk order, with the declaration appended last when requested
    /// (Go parity: `references` + `symbolAt` + `refsToLocations`).
    pub fn references(&self, uri: &Url, pos: Position, include_declaration: bool) -> Vec<Location> {
        let path = uri_to_path(uri);
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let Some(facts) = project.facts_at(&path) else {
            return Vec::new();
        };
        let Ok(src) = self.read_file(&path) else {
            return Vec::new();
        };
        let off = position_to_offset(&src, pos);
        let Some((kind, name)) = symbol_at(facts, off) else {
            return Vec::new();
        };
        let name = name.to_string();

        // Convert refs to locations, reading each file once (Go's srcCache).
        let mut src_cache: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();
        let mut locations = Vec::new();
        for r in project.references(kind, &name) {
            let file = PathBuf::from(&r.file);
            let text = src_cache
                .entry(file.clone())
                .or_insert_with(|| self.read_file(&file).ok());
            let Some(text) = text else { continue };
            locations.push(Location {
                uri: path_to_uri(&file),
                range: Range {
                    start: offset_to_position(text, r.start),
                    end: offset_to_position(text, r.end),
                },
            });
        }

        if include_declaration
            && let Some(symbol) = project.table().lookup(kind, &name)
            && let Some(def_full) = project.rel_to_full(&symbol.file)
            && let Ok(def_src) = self.read_file(def_full)
        {
            locations.push(Location {
                uri: path_to_uri(def_full),
                range: Range {
                    start: offset_to_position(&def_src, symbol.offset),
                    end: offset_to_position(&def_src, symbol.end_offset),
                },
            });
        }
        locations
    }

    /// `textDocument/documentSymbol`: the file's definitions as a flat outline.
    /// Built from `FileFacts.defs` — a feature the Go server does not have.
    pub fn document_symbol(&self, uri: &Url) -> Vec<lsp_types::DocumentSymbol> {
        let path = uri_to_path(uri);
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let Some(facts) = project.facts_at(&path) else {
            return Vec::new();
        };
        let Ok(src) = self.read_file(&path) else {
            return Vec::new();
        };
        facts
            .defs
            .iter()
            .map(|d| {
                let range = Range {
                    start: offset_to_position(&src, d.offset),
                    end: offset_to_position(&src, d.end_offset),
                };
                #[allow(deprecated)] // `deprecated` is a required struct field
                lsp_types::DocumentSymbol {
                    name: d.name.clone(),
                    detail: Some(d.kind.as_str().to_string()),
                    kind: lsp_symbol_kind(project.schema().icon(d.kind)),
                    tags: None,
                    deprecated: None,
                    // Facts carry the name span, not the block extent; both
                    // ranges are the name until the facts record block ends.
                    range,
                    selection_range: range,
                    children: None,
                }
            })
            .collect()
    }

    /// `textDocument/hover`: project symbols first, then built-in effects,
    /// triggers, and scope links from the generated game documentation.
    pub fn hover(&self, uri: &Url, pos: Position) -> Option<lsp_types::Hover> {
        let path = uri_to_path(uri);
        let project = self.project.as_ref()?;
        let facts = project.facts_at(&path)?;
        let src = self.read_file(&path).ok()?;
        let off = position_to_offset(&src, pos);

        if let Some((kind, name)) = symbol_at(facts, off) {
            let mut text = format!("```pdxscript\n{} {}\n```", kind.as_str(), name);
            if let Some(symbol) = project.table().lookup(kind, name) {
                text.push_str(&format!("\n\nDefined in `{}`", symbol.file));
                if !symbol.params.is_empty() {
                    text.push_str("\n\nParameters: ");
                    text.push_str(
                        &symbol
                            .params
                            .iter()
                            .map(|p| format!("`${p}$`"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                }
            } else {
                text.push_str("\n\n*(unresolved)*");
            }

            let span = span_at(facts, off)?;
            return Some(markdown_hover(&src, span, text));
        }

        builtin_hover(&src, off, project.rel_at(&path)?)
    }

    /// `textDocument/inlayHint`: lightweight dynamic-scope annotations at
    /// block openers. This is an editor query only; facts and diagnostics are
    /// deliberately unchanged.
    pub fn inlay_hints(&self, uri: &Url, range: Range) -> Vec<InlayHint> {
        let path = uri_to_path(uri);
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let Some(rel_path) = project.rel_at(&path) else {
            return Vec::new();
        };
        let Ok(src) = self.read_file(&path) else {
            return Vec::new();
        };
        scope_hints(&src, rel_path, range)
    }

    /// `textDocument/completion`: context-aware items. The enclosing-key
    /// chain is derived from the raw token stream (brace stack up to the
    /// cursor) rather than the parsed tree — parser node ranges cover keys
    /// only (Go parity), and a cursor inside an empty block sits in no node
    /// at all; tokens stay honest on half-typed input too. The chain's
    /// structural context picks the completion sources (struct fields /
    /// effects / triggers / keyword sets).
    pub fn completion(&self, uri: &Url, pos: Position) -> Vec<lsp_types::CompletionItem> {
        let path = uri_to_path(uri);
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let Some(rel) = project.rel_at(&path).map(str::to_string) else {
            return Vec::new();
        };
        let Ok(src) = self.read_file(&path) else {
            return Vec::new();
        };
        let off = position_to_offset(&src, pos);
        let cursor = cursor_context(&src, off);
        if let Some(prefix) = cursor.scope_prefix.as_deref() {
            return crate::completion::symbol_value_items(
                project.table(),
                project.schema(),
                project.schema().scope_prefix_kinds(prefix, &rel),
            );
        }
        if let Some(key) = cursor.value_key.as_deref() {
            return crate::completion::symbol_value_items(
                project.table(),
                project.schema(),
                project.schema().value_kinds(key, &rel),
            );
        }
        if let Some(key) = cursor
            .chain
            .last()
            .and_then(|key| std::str::from_utf8(key).ok())
        {
            let items = crate::completion::symbol_value_items(
                project.table(),
                project.schema(),
                project.schema().list_value_kinds(key, &rel),
            );
            if !items.is_empty() {
                return items;
            }
        }
        if cursor.chain.is_empty() {
            return crate::completion::top_level_items(&rel);
        }
        let ctx = pdxl_analysis::context::context_of_chain(
            cursor.chain.iter().map(Vec::as_slice),
            &rel,
            pdxl_ck3::contexts::context_schema(),
        );
        crate::completion::items_for(ctx, project.table(), scope_at(&src, &rel, off).as_deref())
    }

    /// Whether `path` lives under the mod root; no root = everything in scope.
    fn under_mod_root(&self, path: &Path) -> bool {
        let Some(root) = &self.mod_root else {
            return true;
        };
        let p = PathBuf::from(pdxl_path::clean(&path.to_string_lossy()));
        p == *root || p.starts_with(root)
    }

    /// The current text of a file: the open buffer if any, else disk
    /// (Go: `readFileLocked` — no lock needed here, the loop owns the state).
    fn read_file(&self, path: &Path) -> std::io::Result<Vec<u8>> {
        if let Some(doc) = self.docs.get(path) {
            return Ok(doc.text.clone());
        }
        std::fs::read(path)
    }

    /// Test/introspection: whether the project is built.
    pub fn is_ready(&self) -> bool {
        self.project.is_some()
    }
}

fn markdown_hover(src: &[u8], span: (u32, u32), text: String) -> lsp_types::Hover {
    lsp_types::Hover {
        contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value: text,
        }),
        range: Some(Range {
            start: offset_to_position(src, span.0),
            end: offset_to_position(src, span.1),
        }),
    }
}

#[derive(Clone, Default)]
struct ScopeFrame {
    key: Vec<u8>,
    scope: Option<String>,
}

/// Folds explicit scope declarations plus documented iterator and scope-link
/// transitions over raw tokens. As with completion, raw tokens keep this
/// useful inside incomplete blocks.
fn scope_hints(src: &[u8], rel_path: &str, requested: Range) -> Vec<InlayHint> {
    use pdxl_lexer::TokenKind as T;
    let is_scalar = |kind: T| {
        matches!(
            kind,
            T::Identifier
                | T::LiteralString
                | T::LiteralBoolean
                | T::LiteralDate
                | T::MacroParam
                | T::ScriptValue
        )
    };
    let is_op = |kind: T| {
        matches!(
            kind,
            T::Equal
                | T::QuestionEqual
                | T::EqualEqual
                | T::NotEqual
                | T::GreaterThan
                | T::GreaterEqual
                | T::LessThan
                | T::LessEqual
        )
    };
    let mut stack: Vec<ScopeFrame> = Vec::new();
    let mut recent = Vec::new();
    let mut hints = Vec::new();

    for token in pdxl_lexer::tokenize(src) {
        let is_event_body = event_body_context(&stack, rel_path);
        match token.kind {
            T::LBrace => {
                let key = block_key(src, &recent, &is_scalar, &is_op);
                let parent_scope = stack.last().and_then(|frame| frame.scope.clone());
                let scope = block_scope(src, &recent, &key, &stack, rel_path, parent_scope);
                if let Some(scope) = &scope {
                    let position = offset_to_position(src, token.range.end);
                    if position_in_range(position, requested) {
                        hints.push(InlayHint {
                            position,
                            label: format!(": {scope}").into(),
                            kind: Some(InlayHintKind::TYPE),
                            text_edits: None,
                            tooltip: Some(InlayHintTooltip::String(
                                "Current CK3 scope type (best effort)".into(),
                            )),
                            padding_left: Some(true),
                            padding_right: None,
                            data: None,
                        });
                    }
                }
                stack.push(ScopeFrame { key, scope });
                recent.clear();
            }
            T::RBrace => {
                stack.pop();
                recent.clear();
            }
            T::Comment => {}
            _ => {
                if is_scalar(token.kind)
                    && recent.len() >= 2
                    && is_op(recent[recent.len() - 1].kind)
                    && let Some(frame) = stack.last_mut()
                {
                    let key = token_text(src, recent[recent.len() - 2]);
                    if key == b"scope" {
                        frame.scope = std::str::from_utf8(token_text(src, token))
                            .ok()
                            .map(str::to_owned);
                    } else if key == b"type" && is_event_body {
                        frame.scope = event_type_scope(token_text(src, token)).map(str::to_owned);
                    }
                }
                if recent.len() == 4 {
                    recent.remove(0);
                }
                recent.push(token);
            }
        }
    }
    hints
}

/// Best-effort current scope at an arbitrary cursor offset. This is shared by
/// completion today and leaves room for scope-link completion and diagnostics.
pub(crate) fn scope_at(src: &[u8], rel_path: &str, off: u32) -> Option<String> {
    use pdxl_lexer::TokenKind as T;
    let is_scalar = |kind: T| {
        matches!(
            kind,
            T::Identifier
                | T::LiteralString
                | T::LiteralBoolean
                | T::LiteralDate
                | T::MacroParam
                | T::ScriptValue
        )
    };
    let is_op = |kind: T| {
        matches!(
            kind,
            T::Equal
                | T::QuestionEqual
                | T::EqualEqual
                | T::NotEqual
                | T::GreaterThan
                | T::GreaterEqual
                | T::LessThan
                | T::LessEqual
        )
    };
    let mut stack: Vec<ScopeFrame> = Vec::new();
    let mut recent = Vec::new();
    for token in pdxl_lexer::tokenize(src) {
        if token.range.start >= off {
            break;
        }
        let is_event_body = event_body_context(&stack, rel_path);
        match token.kind {
            T::LBrace => {
                let key = block_key(src, &recent, &is_scalar, &is_op);
                let inherited = stack.last().and_then(|frame| frame.scope.clone());
                let scope = block_scope(src, &recent, &key, &stack, rel_path, inherited);
                stack.push(ScopeFrame { key, scope });
                recent.clear();
            }
            T::RBrace => {
                stack.pop();
                recent.clear();
            }
            T::Comment => {}
            _ => {
                if is_scalar(token.kind)
                    && recent.len() >= 2
                    && is_op(recent[recent.len() - 1].kind)
                    && let Some(frame) = stack.last_mut()
                {
                    let key = token_text(src, recent[recent.len() - 2]);
                    if key == b"scope" {
                        frame.scope = std::str::from_utf8(token_text(src, token))
                            .ok()
                            .map(str::to_owned);
                    } else if key == b"type" && is_event_body {
                        frame.scope = event_type_scope(token_text(src, token)).map(str::to_owned);
                    }
                }
                if recent.len() == 4 {
                    recent.remove(0);
                }
                recent.push(token);
            }
        }
    }
    stack.last().and_then(|frame| frame.scope.clone())
}

/// CK3 event types that establish a character as the implicit root scope.
/// Other event types stay unknown until their scope semantics are documented.
fn event_type_scope(value: &[u8]) -> Option<&'static str> {
    matches!(value, b"character_event" | b"letter_event").then_some("character")
}

fn event_body_context(stack: &[ScopeFrame], rel_path: &str) -> bool {
    matches!(
        pdxl_analysis::context::context_of_chain(
            stack.iter().map(|frame| frame.key.as_slice()),
            rel_path,
            pdxl_ck3::contexts::context_schema(),
        ),
        ClauseKind::Struct(spec) if spec.name == "event"
    )
}

fn block_key(
    src: &[u8],
    recent: &[pdxl_lexer::Token],
    is_scalar: &impl Fn(pdxl_lexer::TokenKind) -> bool,
    is_op: &impl Fn(pdxl_lexer::TokenKind) -> bool,
) -> Vec<u8> {
    let n = recent.len();
    if n >= 2 && is_scalar(recent[n - 2].kind) && is_op(recent[n - 1].kind) {
        token_text(src, recent[n - 2]).to_vec()
    } else if n >= 3
        && is_scalar(recent[n - 3].kind)
        && is_op(recent[n - 2].kind)
        && is_scalar(recent[n - 1].kind)
    {
        token_text(src, recent[n - 3]).to_vec()
    } else {
        Vec::new()
    }
}

fn block_scope(
    src: &[u8],
    recent: &[pdxl_lexer::Token],
    key: &[u8],
    stack: &[ScopeFrame],
    rel_path: &str,
    inherited: Option<String>,
) -> Option<String> {
    let parent_keys = stack.iter().map(|frame| frame.key.as_slice());
    let context = pdxl_analysis::context::context_of_chain(
        parent_keys,
        rel_path,
        pdxl_ck3::contexts::context_schema(),
    );
    let key = std::str::from_utf8(key).ok()?;
    let rows = match context {
        ClauseKind::Effect => pdxl_ck3::tables::EFFECTS,
        ClauseKind::Trigger => pdxl_ck3::tables::TRIGGERS,
        _ => &[],
    };
    if let Some(row) = rows.iter().find(|row| row.name == key)
        && row.targets.len() == 1
    {
        return Some(row.targets[0].to_string());
    }
    let n = recent.len();
    if n >= 4
        && token_text(src, recent[n - 3]) == b":"
        && recent[n - 4].kind == pdxl_lexer::TokenKind::Identifier
    {
        let link_name = std::str::from_utf8(token_text(src, recent[n - 4])).ok()?;
        if let Some(link) = pdxl_ck3::tables::SCOPE_LINKS.iter().find(|link| {
            link.name == link_name
                && (link.global_link
                    || inherited
                        .as_deref()
                        .is_some_and(|scope| link.input_scopes.contains(&scope)))
        }) && link.output_scopes.len() == 1
        {
            return Some(link.output_scopes[0].to_string());
        }
    }
    inherited
}

fn token_text(src: &[u8], token: pdxl_lexer::Token) -> &[u8] {
    &src[token.range.start as usize..token.range.end as usize]
}

fn position_in_range(position: Position, range: Range) -> bool {
    position >= range.start && position <= range.end
}

/// Built-in documentation is intentionally a token query, not an AST query:
/// it remains useful while the user is typing incomplete script.
fn builtin_hover(src: &[u8], off: u32, rel_path: &str) -> Option<lsp_types::Hover> {
    use pdxl_lexer::TokenKind as T;

    let token = pdxl_lexer::tokenize(src)
        .into_iter()
        .find(|token| token.range.start <= off && off < token.range.end)?;
    if !matches!(
        token.kind,
        T::Identifier | T::LiteralString | T::LiteralBoolean | T::LiteralDate
    ) {
        return None;
    }
    let name =
        std::str::from_utf8(&src[token.range.start as usize..token.range.end as usize]).ok()?;

    if is_scope_link_token(src, token.range.start, token.range.end) {
        let link = pdxl_ck3::tables::SCOPE_LINKS
            .iter()
            .find(|link| link.name == name)?;
        let mut text = format!("```pdxscript\nscope link {name}\n```");
        if link.requires_data {
            text.push_str("\n\nTakes a `:key` argument.");
        }
        if link.global_link {
            text.push_str("\n\nUsable from any scope.");
        } else {
            text.push_str(&format!(
                "\n\nInput scopes: {}",
                link.input_scopes.join(", ")
            ));
        }
        text.push_str(&format!(
            "\n\nOutput scopes: {}",
            link.output_scopes.join(", ")
        ));
        return Some(markdown_hover(
            src,
            (token.range.start, token.range.end),
            text,
        ));
    }

    let ctx = pdxl_analysis::context::context_of_chain(
        cursor_context(src, off).chain.iter().map(Vec::as_slice),
        rel_path,
        pdxl_ck3::contexts::context_schema(),
    );
    let (label, row) = match ctx {
        ClauseKind::Effect => (
            "effect",
            pdxl_ck3::tables::EFFECTS
                .iter()
                .find(|row| row.name == name)?,
        ),
        ClauseKind::Trigger => (
            "trigger",
            pdxl_ck3::tables::TRIGGERS
                .iter()
                .find(|row| row.name == name)?,
        ),
        _ => return None,
    };
    let mut text = format!(
        "```pdxscript\n{label} {name}\n```\n\nSupported scopes: {}",
        row.scopes.join(", ")
    );
    if !row.description.is_empty() {
        text.push_str(&format!("\n\n{}", row.description));
    }
    if !row.targets.is_empty() {
        text.push_str(&format!(
            "\n\nSupported targets: {}",
            row.targets.join(", ")
        ));
    }
    Some(markdown_hover(
        src,
        (token.range.start, token.range.end),
        text,
    ))
}

fn is_scope_link_token(src: &[u8], start: u32, end: u32) -> bool {
    let tokens = pdxl_lexer::tokenize(src);
    let Some(index) = tokens
        .iter()
        .position(|token| token.range.start == start && token.range.end == end)
    else {
        return false;
    };
    matches!(
        tokens.get(index + 1).map(|token| token.kind),
        Some(pdxl_lexer::TokenKind::Colon)
    ) || matches!(
        tokens.get(index.wrapping_sub(1)).map(|token| token.kind),
        Some(pdxl_lexer::TokenKind::Dot)
    )
}

/// The keys of the blocks enclosing byte offset `off`, outermost first,
/// from a raw token scan: push on `{` (with the key inferred from the
/// preceding `key =` / `key = tag` tokens; empty for anonymous blocks),
/// pop on `}`. A token containing `off` (the word being typed) is excluded.
struct CursorContext {
    chain: Vec<Vec<u8>>,
    value_key: Option<String>,
    scope_prefix: Option<String>,
}

/// The enclosing brace-key chain plus the value syntax immediately before the
/// cursor. The token ring deliberately resets at braces, so a `key =` from an
/// outer block cannot leak into an inner list or block.
fn cursor_context(src: &[u8], off: u32) -> CursorContext {
    use pdxl_lexer::TokenKind as T;
    let is_scalar = |k: T| {
        matches!(
            k,
            T::Identifier
                | T::LiteralNumber
                | T::LiteralString
                | T::LiteralBoolean
                | T::LiteralDate
                | T::MacroParam
                | T::ScriptValue
        )
    };
    let is_op = |k: T| {
        matches!(
            k,
            T::Equal
                | T::QuestionEqual
                | T::EqualEqual
                | T::NotEqual
                | T::GreaterThan
                | T::GreaterEqual
                | T::LessThan
                | T::LessEqual
        )
    };

    let mut stack: Vec<Vec<u8>> = Vec::new();
    // The most recent non-comment tokens, enough to see `key = tag` behind
    // an opening brace.
    let mut recent: Vec<pdxl_lexer::Token> = Vec::new();
    for tok in pdxl_lexer::tokenize(src) {
        if tok.range.start >= off {
            break;
        }
        match tok.kind {
            T::LBrace => {
                let n = recent.len();
                // `key = tag {` (tagged block) or `key = {`.
                let key = if n >= 3
                    && is_scalar(recent[n - 1].kind)
                    && is_op(recent[n - 2].kind)
                    && is_scalar(recent[n - 3].kind)
                {
                    src[recent[n - 3].range.start as usize..recent[n - 3].range.end as usize]
                        .to_vec()
                } else if n >= 2 && is_op(recent[n - 1].kind) && is_scalar(recent[n - 2].kind) {
                    src[recent[n - 2].range.start as usize..recent[n - 2].range.end as usize]
                        .to_vec()
                } else {
                    Vec::new() // anonymous block (list item)
                };
                stack.push(key);
                recent.clear();
            }
            T::RBrace => {
                stack.pop();
                recent.clear();
            }
            T::Comment => {}
            _ => {
                if recent.len() == 3 {
                    recent.remove(0);
                }
                recent.push(tok);
            }
        }
    }
    let value_key = if recent.len() >= 2
        && is_scalar(recent[recent.len() - 2].kind)
        && is_op(recent[recent.len() - 1].kind)
    {
        std::str::from_utf8(
            &src[recent[recent.len() - 2].range.start as usize
                ..recent[recent.len() - 2].range.end as usize],
        )
        .ok()
        .map(str::to_owned)
    } else {
        None
    };
    let prefix_token = if recent.len() >= 2
        && is_scalar(recent[recent.len() - 2].kind)
        && recent[recent.len() - 1].kind == T::Colon
    {
        Some(&recent[recent.len() - 2])
    } else if recent.len() >= 3
        && is_scalar(recent[recent.len() - 3].kind)
        && recent[recent.len() - 2].kind == T::Colon
        && is_scalar(recent[recent.len() - 1].kind)
    {
        Some(&recent[recent.len() - 3])
    } else {
        None
    };
    let scope_prefix = if let Some(prefix) = prefix_token {
        std::str::from_utf8(&src[prefix.range.start as usize..prefix.range.end as usize])
            .ok()
            .map(str::to_owned)
    } else {
        None
    };
    CursorContext {
        chain: stack,
        value_key,
        scope_prefix,
    }
}

/// The (kind, name) of the definition name or reference spanning byte offset
/// `off`. Definitions are checked first so the cursor on a `NAME = {}` name
/// resolves to that symbol (Go's `symbolAt`).
fn symbol_at(
    facts: &pdxl_analysis::FileFacts,
    off: u32,
) -> Option<(pdxl_analysis::SymbolKind, &str)> {
    for d in &facts.defs {
        if d.offset <= off && off < d.end_offset {
            return Some((d.kind, &d.name));
        }
    }
    for r in &facts.refs {
        if r.start <= off && off < r.end {
            return Some((r.kind, &r.name));
        }
    }
    None
}

/// The byte span backing `symbol_at`'s answer (for hover highlighting).
fn span_at(facts: &pdxl_analysis::FileFacts, off: u32) -> Option<(u32, u32)> {
    for d in &facts.defs {
        if d.offset <= off && off < d.end_offset {
            return Some((d.offset, d.end_offset));
        }
    }
    for r in &facts.refs {
        if r.start <= off && off < r.end {
            return Some((r.start, r.end));
        }
    }
    None
}

/// Presentation mapping from pdxl symbol kinds to LSP outline icons.
/// Maps the schema's neutral presentation hint onto the LSP vocabulary —
/// the only place that knows both sides. New kinds pick an [`IconHint`] in
/// their `KindSpec` row; this function changes only if the hint enum grows.
fn lsp_symbol_kind(icon: pdxl_analysis::IconHint) -> lsp_types::SymbolKind {
    use pdxl_analysis::IconHint as I;
    match icon {
        I::Function => lsp_types::SymbolKind::FUNCTION,
        I::Event => lsp_types::SymbolKind::EVENT,
        I::Tag => lsp_types::SymbolKind::ENUM_MEMBER,
        I::Action => lsp_types::SymbolKind::METHOD,
        I::Object => lsp_types::SymbolKind::OBJECT,
        I::Hierarchy => lsp_types::SymbolKind::NAMESPACE, // hierarchical, like titles
    }
}
