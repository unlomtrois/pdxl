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
    Diagnostic, DiagnosticSeverity, Location, Position, PublishDiagnosticsParams, Range, Url,
};
use pdxl_analysis::RefDiag;
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
                eprintln!("pdxl-lsp: failed to build project: {e}");
                return;
            }
        };
        for (path, doc) in &self.docs {
            let _ = project.update_source(path, doc.text.clone());
        }
        self.project = Some(project);
        self.publish_project_diagnostics();
    }

    /// `textDocument/didOpen`: store the buffer and analyze immediately.
    pub fn did_open(&mut self, uri: Url, text: String) {
        let path = uri_to_path(&uri);
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
        Some((path, doc.generation))
    }

    /// A debounce timer fired; acts only if it carries the latest generation.
    pub fn debounce_fired(&mut self, path: &Path, generation: u64) {
        match self.docs.get(path) {
            Some(doc) if doc.generation == generation => self.analyze_and_publish(path),
            _ => {} // superseded by a newer edit, or closed
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
            return; // project not ready; project_ready will catch up
        };
        if let Some(doc) = self.docs.get(path) {
            let _ = project.update_source(path, doc.text.clone());
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
                    kind: lsp_symbol_kind(d.kind),
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

    /// `textDocument/hover`: kind, name, defining file, and macro parameters
    /// of the symbol under the cursor (definitions first, like references).
    pub fn hover(&self, uri: &Url, pos: Position) -> Option<lsp_types::Hover> {
        let path = uri_to_path(uri);
        let project = self.project.as_ref()?;
        let facts = project.facts_at(&path)?;
        let src = self.read_file(&path).ok()?;
        let off = position_to_offset(&src, pos);
        let (kind, name) = symbol_at(facts, off)?;

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

        // Highlight the exact span the hover describes.
        let span = span_at(facts, off)?;
        Some(lsp_types::Hover {
            contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: text,
            }),
            range: Some(Range {
                start: offset_to_position(&src, span.0),
                end: offset_to_position(&src, span.1),
            }),
        })
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
fn lsp_symbol_kind(kind: pdxl_analysis::SymbolKind) -> lsp_types::SymbolKind {
    use pdxl_analysis::SymbolKind as K;
    match kind {
        K::ScriptedTrigger | K::ScriptedEffect => lsp_types::SymbolKind::FUNCTION,
        K::Event | K::OnAction => lsp_types::SymbolKind::EVENT,
        K::Trait => lsp_types::SymbolKind::ENUM_MEMBER,
        K::Decision => lsp_types::SymbolKind::METHOD,
        K::Character => lsp_types::SymbolKind::OBJECT,
    }
}
