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
use lsp_server::{Message, Notification, Request as ServerRequest, RequestId};
use lsp_types::notification::{Notification as _, PublishDiagnostics};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, InlayHint, InlayHintKind, InlayHintTooltip, Location, Position,
    PrepareRenameResponse, PublishDiagnosticsParams, Range, SymbolInformation, TextEdit, Url,
    WorkspaceEdit,
};
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::{KindId, LOC_KEY, RefDiag, Schema};
use pdxl_project::Project;

use crate::position::{
    offset_to_position, offsets_to_positions, path_to_uri, position_to_offset, uri_to_path,
};

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

    /// The loaded project, if the build has completed (read-only; benchmarks
    /// and tests peek at facts through this).
    pub fn project(&self) -> Option<&Project> {
        self.project.as_ref()
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
            "project ready ({}): {} symbols, {} diagnostics, {} open docs",
            pdxl_game::GAME,
            self.project.as_ref().unwrap().table().total(),
            self.project.as_ref().unwrap().diags().len(),
            self.docs.len()
        );
        self.publish_project_diagnostics();

        // Editors commonly request semantic tokens while the project is still
        // building. Those first-pass tokens cannot color resolved references;
        // explicitly invalidate them now that the symbol table is available.
        let _ = self.out.send(Message::Request(ServerRequest::new(
            RequestId::from("pdxl-semantic-refresh".to_string()),
            "workspace/semanticTokens/refresh".to_string(),
            (),
        )));
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
            let path = PathBuf::from(d.file.as_ref());
            if !self.under_mod_root(&path) {
                continue;
            }
            by_file.entry(path).or_default().push(d);
        }

        // Interface scripts: datafunction typing errors (mod files only),
        // recomputed from the current buffer/disk state. Warnings, not
        // errors — the registry is a snapshot of one game version.
        let mut gui_diags: BTreeMap<PathBuf, Vec<Diagnostic>> = BTreeMap::new();
        let registry = pdxl_game::datafn_registry();
        for path in project.gui_file_paths() {
            if !self.under_mod_root(&path) {
                continue;
            }
            let Ok(text) = self.read_file(&path) else {
                continue;
            };
            let parsed = pdxl_gui::parse(String::new(), text.clone());
            let errs = pdxl_gui::datafn::validate_datafns(parsed.tree(), registry);
            if errs.is_empty() {
                continue;
            }
            let diags = errs
                .into_iter()
                .map(|e| Diagnostic {
                    range: Range {
                        start: offset_to_position(&text, e.start),
                        end: offset_to_position(&text, e.end),
                    },
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("pdxl".to_string()),
                    message: e.msg,
                    ..Diagnostic::default()
                })
                .collect();
            gui_diags.insert(path, diags);
        }

        for (file, file_diags) in &by_file {
            let Ok(text) = self.read_file(file) else {
                continue;
            };
            let mut diags: Vec<Diagnostic> = file_diags
                .iter()
                .map(|d| Diagnostic {
                    range: Range {
                        start: offset_to_position(&text, d.start),
                        end: offset_to_position(&text, d.end),
                    },
                    severity: Some(match d.severity {
                        pdxl_analysis::Severity::Warning => DiagnosticSeverity::WARNING,
                        pdxl_analysis::Severity::Error => DiagnosticSeverity::ERROR,
                    }),
                    source: Some("pdxl".to_string()),
                    message: d.msg.clone(),
                    ..Diagnostic::default()
                })
                .collect();
            if let Some(extra) = gui_diags.remove(file) {
                diags.extend(extra);
            }
            self.publish(file, diags);
        }
        // Gui-only files (no unresolved refs, but datafn warnings).
        let gui_files: Vec<PathBuf> = gui_diags.keys().cloned().collect();
        for (file, diags) in gui_diags {
            self.publish(&file, diags);
        }

        // Clear files that had diagnostics last cycle but no longer do.
        let mut current: std::collections::HashSet<PathBuf> = by_file.into_keys().collect();
        current.extend(gui_files);
        let stale: Vec<PathBuf> = self
            .published
            .iter()
            .filter(|f| !current.contains(*f))
            .cloned()
            .collect();
        for file in stale {
            self.publish(&file, Vec::new());
        }

        log_debug!("published diagnostics for {} file(s)", current.len());
        self.published = current.into_iter().collect();
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
        // A ref OR a call-by-name site (`my_effect = yes`) under the cursor
        // jumps to its definition; a cursor on a definition name does not.
        let reference = facts
            .refs
            .iter()
            .chain(facts.calls.iter())
            .chain(facts.constant_refs.iter())
            .find(|r| r.start <= off && off < r.end)?;

        // Script constants are file-local: resolve against this file's own
        // `@name = …` definitions, never the global table.
        if reference.kind == pdxl_analysis::SCRIPT_CONSTANT {
            let symbol = facts.constants.iter().find(|c| c.name == reference.name)?;
            return Some(Location {
                uri: path_to_uri(&path),
                range: Range {
                    start: offset_to_position(&src, symbol.offset),
                    end: offset_to_position(&src, symbol.end_offset),
                },
            });
        }

        // An unqualified `![Name]` carries the DOC_REF sentinel rather than a
        // real kind, so it searches the same anchor → entity → concept → loc
        // order hover and the link path use. Without this every bare doc ref
        // looked up a kind no table holds, and jumping from one did nothing.
        let symbol = if reference.kind == pdxl_analysis::DOC_REF {
            doc_ref_lookup_order(project.schema())
                .find_map(|k| project.table().lookup(k, &reference.name))?
        } else {
            // Primary kind first, then the rule's alternates (multi-kind refs
            // like custom_description's text resolve to whichever kind defines
            // the name).
            std::iter::once(reference.kind)
                .chain(reference.alt.iter().copied())
                .find_map(|k| project.table().lookup(k, &reference.name))?
        };
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

        // Script constants are file-local: only this file's `@name` uses (and
        // optionally its definition) are returned.
        if kind == pdxl_analysis::SCRIPT_CONSTANT {
            let mut locations: Vec<Location> = facts
                .constant_refs
                .iter()
                .filter(|r| r.name == name)
                .map(|r| Location {
                    uri: path_to_uri(&path),
                    range: Range {
                        start: offset_to_position(&src, r.start),
                        end: offset_to_position(&src, r.end),
                    },
                })
                .collect();
            if include_declaration
                && let Some(symbol) = facts.constants.iter().find(|c| c.name == name)
            {
                locations.push(Location {
                    uri: path_to_uri(&path),
                    range: Range {
                        start: offset_to_position(&src, symbol.offset),
                        end: offset_to_position(&src, symbol.end_offset),
                    },
                });
            }
            return locations;
        }

        // Convert refs to locations: group by file, read each file once, and
        // map all of its offsets in ONE linear pass (offsets_to_positions).
        // A per-ref offset_to_position call rescans the file from byte 0 each
        // time — O(refs × file size), which took ~5s per popular named color
        // in the multi-MB coat-of-arms files.
        let matched = project.references(kind, &name);
        let mut by_file: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, r) in matched.iter().enumerate() {
            by_file.entry(r.file.as_ref()).or_default().push(i);
        }
        let mut slots: Vec<Option<Location>> = vec![None; matched.len()];
        for (file, idxs) in by_file {
            // Script refs traditionally carry full paths; localization refs
            // are extracted with their stable project-relative path. Resolve
            // either representation before reading and producing locations.
            let file = project
                .rel_to_full(file)
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(file));
            let Ok(text) = self.read_file(&file) else {
                continue;
            };
            let offsets: Vec<u32> = idxs
                .iter()
                .flat_map(|&i| [matched[i].start, matched[i].end])
                .collect();
            let positions = offsets_to_positions(&text, &offsets);
            let uri = path_to_uri(&file);
            for (j, &i) in idxs.iter().enumerate() {
                slots[i] = Some(Location {
                    uri: uri.clone(),
                    range: Range {
                        start: positions[2 * j],
                        end: positions[2 * j + 1],
                    },
                });
            }
        }
        // Walk order preserved (slot order = matched order).
        let mut locations: Vec<Location> = slots.into_iter().flatten().collect();

        // An entity whose key exactly matches a localization key implicitly
        // consumes that key as its display name. Model the reverse edge here
        // so the loc definition's CodeLens/references include those entities,
        // even though no explicit reference token exists in script.
        if kind == LOC_KEY {
            for pattern in project.schema().all_implicit_loc_patterns() {
                let Some(entity_name) = pattern.entity_name(&name) else {
                    continue;
                };
                // Empty suffix always strips successfully; reject an empty
                // entity name for suffix-only localization keys.
                if entity_name.is_empty() {
                    continue;
                }
                let Some(symbol) = project.table().lookup(pattern.kind, entity_name) else {
                    continue;
                };
                let Some(full) = project.rel_to_full(&symbol.file) else {
                    continue;
                };
                let Ok(def_src) = self.read_file(full) else {
                    continue;
                };
                locations.push(Location {
                    uri: path_to_uri(full),
                    range: Range {
                        start: offset_to_position(&def_src, symbol.offset),
                        end: offset_to_position(&def_src, symbol.end_offset),
                    },
                });
            }
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

    /// `textDocument/prepareRename`: validates that the cursor sits on a
    /// renameable symbol and returns the identifier span (unquoted) plus the
    /// current name as the placeholder, so the editor pre-selects `brave`, not
    /// `"brave"`. Returns `None` on a non-symbol position (F2 is rejected).
    pub fn prepare_rename(&self, uri: &Url, pos: Position) -> Option<PrepareRenameResponse> {
        let path = uri_to_path(uri);
        let project = self.project.as_ref()?;
        let src = self.read_file(&path).ok()?;
        let off = position_to_offset(&src, pos);
        let facts = project.facts_at(&path)?;
        let (_, _, name) = symbol_at_with_alts(facts, off)?;
        let (start, end) = span_at(facts, off)?;
        // A quoted value's span covers the quotes; select only the identifier.
        let (start, end) = unquoted_span(&src, start, end);
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: Range {
                start: offset_to_position(&src, start),
                end: offset_to_position(&src, end),
            },
            placeholder: name.to_string(),
        })
    }

    /// `textDocument/rename`: resolves the symbol under the cursor (definition
    /// name or reference site) and returns a project-wide [`WorkspaceEdit`]
    /// rewriting its definition and every reference to `new_name`.
    ///
    /// Reuses the same resolution as [`Self::references`] / [`Self::definition`]
    /// (multi-kind refs resolve to whichever kind defines the name). Quoted
    /// reference sites keep their quotes.
    ///
    /// Limitations: loc keys are renamed only in the loaded language (english);
    /// other-language `.yml` definitions are left untouched. A name reachable
    /// only through an alias (a trait *group*) renames the reference sites but
    /// not the `group = X` declaration (which is not a source-backed symbol).
    /// Returns `None` when `new_name` is invalid or nothing resolves.
    pub fn rename(&self, uri: &Url, pos: Position, new_name: &str) -> Option<WorkspaceEdit> {
        if !is_valid_rename(new_name) {
            return None;
        }
        let path = uri_to_path(uri);
        let project = self.project.as_ref()?;
        let src = self.read_file(&path).ok()?;
        let off = position_to_offset(&src, pos);
        let facts = project.facts_at(&path)?;
        let (kind, alt, name) = symbol_at_with_alts(facts, off)?;
        let name = name.to_string();

        // Script constants rename file-locally: every `@name` use in this
        // file plus the `@name = …` definition.
        if kind == pdxl_analysis::SCRIPT_CONSTANT {
            let mut edits = Vec::new();
            let mut push = |start: u32, end: u32| {
                edits.push(TextEdit {
                    range: Range {
                        start: offset_to_position(&src, start),
                        end: offset_to_position(&src, end),
                    },
                    new_text: new_name.to_string(),
                });
            };
            for r in facts.constant_refs.iter().filter(|r| r.name == name) {
                push(r.start, r.end);
            }
            if let Some(d) = facts.constants.iter().find(|c| c.name == name) {
                push(d.offset, d.end_offset);
            }
            let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
            changes.insert(path_to_uri(&path), edits);
            return Some(WorkspaceEdit {
                changes: Some(changes),
                ..WorkspaceEdit::default()
            });
        }

        // Resolve to the kind that actually defines the name (definition()'s
        // rule); fall back to the primary kind so an unresolved ref still
        // renames its sites.
        let def = std::iter::once(kind)
            .chain(alt.iter().copied())
            .find_map(|k| project.table().lookup(k, &name).map(|s| (k, s)));
        let ref_kind = def.map(|(k, _)| k).unwrap_or(kind);

        // Collect every edit span, then convert per file in ONE linear pass
        // (offsets_to_positions) — a per-span offset_to_position rescans the
        // file from byte 0 each time, quadratic on many-ref symbols.
        let mut spans: Vec<(PathBuf, u32, u32)> = project
            .references(ref_kind, &name)
            .into_iter()
            .map(|r| (PathBuf::from(r.file.as_ref()), r.start, r.end))
            .collect();
        // The definition, unless it is a zero-width alias marker (no real span).
        if let Some((_, symbol)) = def
            && symbol.offset != symbol.end_offset
            && let Some(def_full) = project.rel_to_full(&symbol.file)
        {
            spans.push((def_full.to_path_buf(), symbol.offset, symbol.end_offset));
        }

        let mut by_file: HashMap<PathBuf, Vec<(u32, u32)>> = HashMap::new();
        for (file, start, end) in spans {
            by_file.entry(file).or_default().push((start, end));
        }
        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        for (file, spans) in by_file {
            let Ok(text) = self.read_file(&file) else {
                continue;
            };
            let offsets: Vec<u32> = spans.iter().flat_map(|&(s, e)| [s, e]).collect();
            let positions = offsets_to_positions(&text, &offsets);
            let edits = changes.entry(path_to_uri(&file)).or_default();
            for (j, &(start, _)) in spans.iter().enumerate() {
                // Preserve the author's quoting: a quoted span stays quoted.
                let quoted = text.get(start as usize) == Some(&b'"');
                let new_text = if quoted {
                    format!("\"{new_name}\"")
                } else {
                    new_name.to_string()
                };
                edits.push(TextEdit {
                    range: Range {
                        start: positions[2 * j],
                        end: positions[2 * j + 1],
                    },
                    new_text,
                });
            }
        }

        (!changes.is_empty()).then(|| WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }

    /// `textDocument/semanticTokens/full`: lexer-driven syntax highlighting.
    /// Pure over the buffer (no project needed), so it works immediately, even
    /// before the async build completes.
    pub fn semantic_tokens(&self, uri: &Url) -> Option<lsp_types::SemanticTokens> {
        let path = uri_to_path(uri);
        let src = self.read_file(&path).ok()?;
        if path.extension().is_some_and(|e| e == "yml") {
            let resolved = self
                .project
                .as_ref()
                .and_then(|project| {
                    project.facts_at(&path).map(|facts| {
                        let mut spans = facts
                            .refs
                            .iter()
                            // These facts exist only because the schema says
                            // the datafunction argument is an entity key. Its
                            // semantic class must not depend on whether the
                            // target happened to be loaded when tokens were
                            // requested; resolution still controls navigation
                            // and diagnostics independently.
                            .filter(|reference| reference.kind != LOC_KEY)
                            .map(|reference| (reference.start, reference.end))
                            .collect::<Vec<_>>();
                        // Loc scanners append one reference family at a time
                        // (concepts, then datafunction args), not source order.
                        // `apply_overrides` requires sorted spans.
                        spans.sort_unstable();
                        spans
                    })
                })
                .unwrap_or_default();
            return Some(lsp_types::SemanticTokens {
                result_id: None,
                data: crate::semantic::tokens_yml(&src, &resolved),
            });
        }
        // Value ranges the analyzer resolved to a defined symbol → colored as
        // references. Unresolved refs are left as plain values (they already
        // carry a diagnostic). Empty when the project isn't built yet.
        let is_gui = path.extension().is_some_and(|e| e == "gui");
        let resolved = self
            .project
            .as_ref()
            .and_then(|project| {
                project.facts_at(&path).map(|facts| {
                    // Gui name-gated refs (template/type instantiations,
                    // `using` values) live in `calls`; include them for gui
                    // files only — script call coloring is unchanged.
                    let gui_calls = facts.calls.iter().filter(|_| is_gui);
                    let mut spans: Vec<(u32, u32)> = facts
                        .refs
                        .iter()
                        .chain(gui_calls)
                        .filter(|r| {
                            std::iter::once(r.kind)
                                .chain(r.alt.iter().copied())
                                .any(|k| project.table().lookup(k, &r.name).is_some())
                        })
                        .map(|r| (r.start, r.end))
                        .collect();

                    // Smart-doc refs are analysis facts now, living in `calls`
                    // (soft: navigable, never diagnosed). They share this
                    // resolved-span channel for semantic coloring. Keep only
                    // refs which resolve through the symbol table; unresolved
                    // `![name]` remains ordinary comment text.
                    let mut cache: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();
                    for r in doc_refs(&src, facts) {
                        if self
                            .resolve_doc_ref(pinned_kind(r), &r.name, project, &mut cache)
                            .is_some()
                        {
                            spans.push((r.start, r.end));
                        }
                    }
                    spans.sort_by_key(|&(start, _)| start);
                    spans
                })
            })
            .unwrap_or_default();
        // The schema (present once built) lets `![kind:Name]` doc refs color
        // only the name past the qualifier.
        let schema = self.project.as_ref().map(Project::schema);
        let data = if is_gui {
            crate::semantic::tokens_gui(&src, &resolved)
        } else {
            crate::semantic::tokens(&src, &resolved, schema)
        };
        Some(lsp_types::SemanticTokens {
            result_id: None,
            data,
        })
    }

    /// `textDocument/codeLens`: one lens above every definition in the file,
    /// regardless of kind — the reference-count feature is generic over the
    /// symbol table, not per-entity. This phase is deliberately cheap: it
    /// emits only the anchor positions (batched in a single pass) and stashes
    /// the document URI; the count is computed lazily in [`Self::code_lens_resolve`]
    /// for the handful of lenses the editor actually shows.
    pub fn code_lens(&self, uri: &Url) -> Vec<lsp_types::CodeLens> {
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
        let offsets: Vec<u32> = facts.defs.iter().map(|d| d.offset).collect();
        let positions = offsets_to_positions(&src, &offsets);
        positions
            .into_iter()
            .map(|start| lsp_types::CodeLens {
                range: Range { start, end: start },
                command: None, // filled in on resolve
                data: Some(serde_json::json!({ "uri": uri.as_str() })),
            })
            .collect()
    }

    /// `codeLens/resolve`: fill in the "N references" title and a click action
    /// (peek references) for one lens. Runs the reference search only now, so
    /// off-screen lenses cost nothing.
    pub fn code_lens_resolve(&self, mut lens: lsp_types::CodeLens) -> lsp_types::CodeLens {
        let Some(uri) = lens
            .data
            .as_ref()
            .and_then(|d| d.get("uri"))
            .and_then(|v| v.as_str())
            .and_then(|s| Url::parse(s).ok())
        else {
            return lens;
        };
        // The lens anchors on the definition name; resolve references there.
        let locations = self.references(&uri, lens.range.start, false);
        let title = match locations.len() {
            1 => "1 reference".to_string(),
            n => format!("{n} references"),
        };
        lens.command = Some(lsp_types::Command {
            title,
            // A client-side shim (see editor/vscode) — it converts these
            // protocol-JSON arguments into native vscode.Uri/Position/Location
            // objects before delegating to the built-in
            // `editor.action.showReferences`, whose handler validates its
            // arguments with `instanceof` and rejects raw JSON.
            command: "pdxl.showReferences".to_string(),
            arguments: Some(vec![
                serde_json::to_value(&uri).unwrap_or_default(),
                serde_json::to_value(lens.range.start).unwrap_or_default(),
                serde_json::to_value(&locations).unwrap_or_default(),
            ]),
        });
        lens
    }

    /// `textDocument/documentColor`: every color literal in the file (located
    /// via [`ClauseKind::Color`] contexts), so the editor renders inline
    /// swatches. Works from the live buffer.
    ///
    /// [`ClauseKind::Color`]: pdxl_analysis::context::ClauseKind::Color
    pub fn document_color(&self, uri: &Url) -> Vec<lsp_types::ColorInformation> {
        let path = uri_to_path(uri);
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let Some(rel) = project.rel_at(&path).map(str::to_owned) else {
            return Vec::new();
        };
        let Ok(src) = self.read_file(&path) else {
            return Vec::new();
        };
        let (tree, _) =
            pdxl_parser::parse(path.to_string_lossy().into_owned(), src.clone()).into_parts();
        let spans =
            crate::color::document_colors(&tree, &src, &rel, pdxl_game::contexts::context_schema());
        // One linear pass for every span boundary.
        let offsets: Vec<u32> = spans.iter().flat_map(|s| [s.start, s.end]).collect();
        let positions = offsets_to_positions(&src, &offsets);
        spans
            .iter()
            .enumerate()
            .map(|(i, s)| lsp_types::ColorInformation {
                range: Range {
                    start: positions[2 * i],
                    end: positions[2 * i + 1],
                },
                color: s.color,
            })
            .collect()
    }

    /// `textDocument/colorPresentation`: the text a picked color should write
    /// back — rendered in the same form (`hsv`/`hsv360`/`rgb`/implicit) as
    /// the literal it replaces.
    pub fn color_presentation(
        &self,
        params: &lsp_types::ColorPresentationParams,
    ) -> Vec<lsp_types::ColorPresentation> {
        let path = uri_to_path(&params.text_document.uri);
        let Ok(src) = self.read_file(&path) else {
            return Vec::new();
        };
        let start = position_to_offset(&src, params.range.start) as usize;
        let end = (position_to_offset(&src, params.range.end) as usize).min(src.len());
        if start >= end {
            return Vec::new();
        }
        let label = crate::color::present(&src[start..end], &params.color);
        vec![lsp_types::ColorPresentation {
            label: label.clone(),
            text_edit: Some(lsp_types::TextEdit {
                range: params.range,
                new_text: label,
            }),
            additional_text_edits: None,
        }]
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
        // One linear pass for all name spans (both offsets per def), not one
        // full scan per offset — the file can hold hundreds of definitions.
        let offsets: Vec<u32> = facts
            .defs
            .iter()
            .flat_map(|d| [d.offset, d.end_offset])
            .collect();
        let positions = offsets_to_positions(&src, &offsets);
        facts
            .defs
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let range = Range {
                    start: positions[2 * i],
                    end: positions[2 * i + 1],
                };
                #[allow(deprecated)] // `deprecated` is a required struct field
                lsp_types::DocumentSymbol {
                    name: d.name.clone(),
                    detail: Some(d.kind.name().to_string()),
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

    /// `workspace/symbol`: fuzzy-search every project definition by name.
    /// Returns the best [`WORKSPACE_SYMBOL_LIMIT`] matches (name, kind, and a
    /// jump location), ranked by match quality then name length. An empty
    /// query returns an arbitrary capped slice (the client narrows as you type).
    pub fn workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let schema = project.schema();

        let mut scored: Vec<(i32, &pdxl_analysis::Symbol)> = project
            .table()
            .iter()
            .filter_map(|s| fuzzy_score(query, &s.name).map(|sc| (sc, s)))
            .collect();
        // Higher score first, then shorter names (tighter match), then name.
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.name.len().cmp(&b.1.name.len()))
                .then_with(|| a.1.name.cmp(&b.1.name))
        });
        scored.truncate(WORKSPACE_SYMBOL_LIMIT);

        // Resolve each winner's jump location, reading each file once.
        let mut src_cache: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();
        scored
            .into_iter()
            .filter_map(|(_, sym)| {
                let full = project.rel_to_full(&sym.file)?.to_path_buf();
                let text = src_cache
                    .entry(full.clone())
                    .or_insert_with(|| self.read_file(&full).ok())
                    .as_ref()?;
                #[allow(deprecated)] // `deprecated`/`tags` are required fields
                Some(SymbolInformation {
                    name: sym.name.clone(),
                    kind: lsp_symbol_kind(schema.icon(sym.kind)),
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: path_to_uri(&full),
                        range: Range {
                            start: offset_to_position(text, sym.offset),
                            end: offset_to_position(text, sym.end_offset),
                        },
                    },
                    container_name: Some(sym.kind.name().to_string()),
                })
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

        // Interface scripts: hovering inside a `[…]` datafunction shows the
        // segment's signature from the DumpDataTypes registry; hovering a
        // property key shows its curated documentation.
        if path.extension().is_some_and(|e| e == "gui") {
            if let Some(hover) = gui_datafn_hover(&src, off) {
                return Some(hover);
            }
            if let Some(hover) = gui_property_hover(&src, off) {
                return Some(hover);
            }
        }

        if let Some((kind0, alts, name)) = symbol_at_with_alts(facts, off) {
            // The kind that actually defines the name wins the display (a
            // multi-kind ref like custom_description text shows as whichever
            // localization kind it resolved to). An unqualified `![Name]`
            // carries the DOC_REF sentinel instead of a real kind, so it
            // searches the same entity → concept → loc order the link and
            // highlight paths use; without this it always read "unresolved".
            let kind = if kind0 == pdxl_analysis::DOC_REF {
                doc_ref_lookup_order(project.schema())
                    .find(|&k| project.table().lookup(k, name).is_some())
                    .unwrap_or(kind0)
            } else {
                std::iter::once(kind0)
                    .chain(alts.iter().copied())
                    .find(|&k| project.table().lookup(k, name).is_some())
                    .unwrap_or(kind0)
            };
            let mut text = format!("```pdxscript\n{} {}\n```", kind.name(), name);
            if let Some(symbol) = project.table().lookup(kind, name) {
                let mut implicit_loc = Vec::new();
                // Loc keys carry their user-visible text — show it.
                if kind == LOC_KEY
                    && let Some(loc_text) = self.loc_text(project, symbol)
                {
                    text.push_str(&format!("\n\n> {loc_text}"));
                } else if kind != LOC_KEY {
                    for pattern in project.schema().implicit_loc_patterns(kind) {
                        let loc_name = pattern.loc_name(name);
                        let Some(loc_symbol) = project.table().lookup(LOC_KEY, &loc_name) else {
                            continue;
                        };
                        let Some(full) = project.rel_to_full(&loc_symbol.file) else {
                            continue;
                        };
                        let Ok(loc_src) = self.read_file(full) else {
                            continue;
                        };
                        let line = offset_to_position(&loc_src, loc_symbol.offset).line + 1;
                        implicit_loc.push(format!("[{loc_name}]({}#L{line})", path_to_uri(full)));
                    }
                }
                // An anchor documents itself on its declaring line; every other
                // kind takes the `#!` block above its definition (resolved even
                // when hovering a reference, so a call site shows the target's
                // doc).
                let authored = if kind == pdxl_analysis::DOC_ANCHOR {
                    self.anchor_description(project, symbol)
                } else {
                    self.doc_comment_for(project, symbol)
                };
                if let Some(doc) = authored {
                    text.push_str("\n\n");
                    text.push_str(&self.render_doc(&doc, project));
                }
                // Keep the implicit display-name link after authored smart
                // documentation but before the source-location footer.
                if !implicit_loc.is_empty() {
                    text.push_str("\n\n**Localization:** ");
                    text.push_str(&implicit_loc.join(", "));
                }
                // A symbol the engine uses itself has no call site in script.
                // Say so, or its empty reference list reads as dead content.
                if project.schema().is_intrinsic(kind, name) {
                    text.push_str(
                        "\n\n*Engine intrinsic — raised by the game itself, \
                         so it has no reference in script.*",
                    );
                }
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

    /// The localized text of a loc-key symbol, re-read from its defining
    /// `.yml` (symbols store name+location only; the text lives on disk and
    /// one line-scan per hover is cheap).
    fn loc_text(&self, project: &Project, symbol: &pdxl_analysis::Symbol) -> Option<String> {
        let full = project.rel_to_full(&symbol.file)?;
        let src = self.read_file(full).ok()?;
        let line_end = src[symbol.offset as usize..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(src.len(), |p| symbol.offset as usize + p);
        let line = String::from_utf8_lossy(&src[symbol.offset as usize..line_end]);
        let open = line.find('"')?;
        let close = line.rfind('"')?;
        (close > open).then(|| line[open + 1..close].to_string())
    }

    /// The `#!` doc block directly above a symbol's definition, if any. Re-reads
    /// the defining file (like [`Self::loc_text`]) — one line-scan per hover.
    fn doc_comment_for(&self, project: &Project, symbol: &pdxl_analysis::Symbol) -> Option<String> {
        let full = project.rel_to_full(&symbol.file)?;
        let src = self.read_file(full).ok()?;
        extract_doc_block(&src, symbol.offset)
    }

    /// An anchor's description: the text after `#! @key`, or — when the key
    /// stands alone on its line — the `#!` lines that follow it.
    ///
    /// Anchors need their own reader because [`extract_doc_block`] collects the
    /// `#!` lines *above* a definition. That is right for a symbol the comment
    /// documents, but an anchor's declaration *is* the comment, so that walk
    /// starts one line too high and returns the wrong block entirely.
    ///
    /// Both layouts appear in practice — a one-line note reads well inline,
    /// while a paragraph naturally goes underneath the name it labels.
    fn anchor_description(
        &self,
        project: &Project,
        symbol: &pdxl_analysis::Symbol,
    ) -> Option<String> {
        let full = project.rel_to_full(&symbol.file)?;
        let src = self.read_file(full).ok()?;
        let from = (symbol.end_offset as usize).min(src.len());
        let mut line_end = src[from..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(src.len(), |i| from + i);
        let inline = std::str::from_utf8(&src[from..line_end]).ok()?.trim();
        if !inline.is_empty() {
            return Some(inline.to_string());
        }
        // Nothing after the key: take the `#!` lines below, stopping at the
        // first ordinary line or at another declaration (which owns its own).
        let mut lines: Vec<String> = Vec::new();
        while line_end < src.len() {
            let start = line_end + 1;
            let end = src[start..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(src.len(), |i| start + i);
            let line = &src[start..end];
            let trimmed = line.trim_ascii_start();
            let Some(rest) = trimmed.strip_prefix(b"#!") else {
                break;
            };
            // `doc_anchor_span` expects a range starting at `#`, as the lexer
            // hands them over — not the line start, which may be indented.
            let hash = start + (line.len() - trimmed.len());
            if pdxl_analysis::doc_anchor_span(&src, hash as u32, end as u32).is_some() {
                break;
            }
            let Ok(text) = std::str::from_utf8(rest) else {
                break;
            };
            lines.push(
                text.strip_prefix(' ')
                    .unwrap_or(text)
                    .trim_end()
                    .to_string(),
            );
            line_end = end;
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// Renders a doc block to hover markdown, turning each `![Name]` into a
    /// go-to-definition link when `Name` resolves (first matching kind), or a
    /// plain code span otherwise. Prose is passed through as markdown.
    fn render_doc(&self, doc: &str, project: &Project) -> String {
        let mut out = String::with_capacity(doc.len());
        let mut cache: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();
        let mut rest = doc;
        while let Some(pos) = rest.find("![") {
            out.push_str(&rest[..pos]);
            let after = &rest[pos + 2..];
            if let Some(close) = after.find(']') {
                out.push_str(&self.render_doc_ref(&after[..close], project, &mut cache));
                rest = &after[close + 1..];
            } else {
                out.push_str("![");
                rest = after;
            }
        }
        out.push_str(rest);
        out
    }

    /// A single `![…]` reference (its inner text, possibly `kind:name`): a link
    /// to the definition, else `` `name` ``.
    fn render_doc_ref(
        &self,
        content: &str,
        project: &Project,
        cache: &mut HashMap<PathBuf, Option<Vec<u8>>>,
    ) -> String {
        let (kind, off) = parse_doc_ref(content.as_bytes(), project.schema());
        let name = &content[off..];
        match self.resolve_doc_ref(kind, name, project, cache) {
            Some((full, line)) => format!("[{name}]({}#L{line})", path_to_uri(&full)),
            None => format!("`{name}`"),
        }
    }

    /// `textDocument/documentLink`: makes every `![Name]` inside a `#!` doc
    /// comment clickable, targeting `Name`'s definition (resolved refs only).
    /// The link covers the name, not any `kind:` qualifier.
    pub fn document_links(&self, uri: &Url) -> Vec<lsp_types::DocumentLink> {
        let path = uri_to_path(uri);
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let Ok(src) = self.read_file(&path) else {
            return Vec::new();
        };
        let Some(facts) = project.facts_at(&path) else {
            return Vec::new();
        };
        let mut cache: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();
        let mut links = Vec::new();
        for r in doc_refs(&src, facts) {
            let Some((full, line)) =
                self.resolve_doc_ref(pinned_kind(r), &r.name, project, &mut cache)
            else {
                continue;
            };
            let mut target = path_to_uri(&full);
            target.set_fragment(Some(&format!("L{line}")));
            let name = &r.name;
            links.push(lsp_types::DocumentLink {
                range: Range {
                    start: offset_to_position(&src, r.start),
                    end: offset_to_position(&src, r.end),
                },
                target: Some(target),
                tooltip: Some(format!("Go to {name}")),
                data: None,
            });
        }
        links
    }

    /// Resolves a doc ref to its target's file + 1-based line. The ref text is
    /// `Symbol` or `Symbol.field.path`; because symbol names contain dots
    /// (`test.0001`), the symbol is the **longest prefix that resolves** and the
    /// remainder is a field path walked into the definition. A pinned `kind`
    /// looks up only that kind; otherwise every definition kind is tried before
    /// `LocKey` (a loc string usually just shadows its object). An unfound field
    /// falls back to the definition's own line.
    fn resolve_doc_ref(
        &self,
        kind: Option<KindId>,
        name: &str,
        project: &Project,
        cache: &mut HashMap<PathBuf, Option<Vec<u8>>>,
    ) -> Option<(PathBuf, u32)> {
        let mut end = name.len();
        loop {
            if let Some((full, offset)) = self.lookup_symbol(kind, &name[..end], project) {
                let src = cache
                    .entry(full.clone())
                    .or_insert_with(|| self.read_file(&full).ok())
                    .as_ref()?;
                let field_path = name[end..].strip_prefix('.').unwrap_or("");
                let target = if field_path.is_empty() {
                    offset
                } else {
                    field_offset_in(src, offset, field_path).unwrap_or(offset)
                };
                return Some((full, offset_to_position(src, target).line + 1));
            }
            end = name[..end].rfind('.')?;
        }
    }

    /// The def file + name offset of `name`, honoring a pinned kind or the
    /// loc-last default order. No file read (table lookup only).
    fn lookup_symbol(
        &self,
        kind: Option<KindId>,
        name: &str,
        project: &Project,
    ) -> Option<(PathBuf, u32)> {
        let find = |k: KindId| {
            let sym = project.table().lookup(k, name)?;
            let full = project.rel_to_full(&sym.file)?;
            Some((full.to_path_buf(), sym.offset))
        };
        match kind {
            Some(k) => find(k),
            None => doc_ref_lookup_order(project.schema()).find_map(find),
        }
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
        let mut hints = scope_hints(&src, rel_path, range);
        hints.extend(self.loc_text_hints(project, &path, &src, range));
        hints
    }

    /// Loc-text inlay hints: every resolved loc-key reference in `range`
    /// gets its localized text appended after the value (truncated; the
    /// full text rides in the hint tooltip).
    fn loc_text_hints(
        &self,
        project: &Project,
        path: &Path,
        src: &[u8],
        range: Range,
    ) -> Vec<InlayHint> {
        const MAX_HINT_CHARS: usize = 72;
        let Some(facts) = project.facts_at(path) else {
            return Vec::new();
        };
        let start = position_to_offset(src, range.start);
        let end = position_to_offset(src, range.end);
        let mut hints = Vec::new();
        for r in &facts.refs {
            if r.kind != LOC_KEY || r.start < start || r.end > end {
                continue;
            }
            let Some(symbol) = project.table().lookup(r.kind, &r.name) else {
                continue; // unresolved keys already get a diagnostic
            };
            let Some(text) = self.loc_text(project, symbol) else {
                continue;
            };
            let mut label: String = text.chars().take(MAX_HINT_CHARS).collect();
            if label.len() < text.len() {
                label.push('…');
            }
            hints.push(InlayHint {
                position: offset_to_position(src, r.end),
                label: lsp_types::InlayHintLabel::String(label),
                kind: None, // free-form text, neither Type nor Parameter
                text_edits: None,
                tooltip: Some(InlayHintTooltip::String(text)),
                padding_left: Some(true),
                padding_right: None,
                data: None,
            });
        }
        hints
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
        if path.extension().is_some_and(|e| e == "yml") {
            return crate::yml_completion::items(project, &src, off);
        }
        // Interface scripts use their own contexts (widget properties, mined
        // values, datafunction chains) — nothing below applies to them.
        if path.extension().is_some_and(|e| e == "gui") {
            return crate::gui_completion::items(project, &src, off);
        }
        // Smart-doc references. Checked before the script contexts because a
        // `#!` comment is not script — none of them apply inside one.
        if let Some(doc) = doc_ref_cursor(&src, off, project.schema()) {
            let range = Range::new(
                offset_to_position(&src, doc.name_start),
                offset_to_position(&src, off),
            );
            let mut items = Vec::new();
            // Unqualified: lead with the `alias:` qualifiers, so an author who
            // does not know the name can narrow by kind first.
            if !doc.qualified {
                items.extend(crate::completion::doc_ref_prefix_items(
                    project.schema(),
                    &doc.name_prefix,
                ));
            }
            // Names: one kind when pinned, else the resolution order minus
            // localization — 279k loc keys would swamp everything, and `loc:`
            // reaches them deliberately.
            let kinds: Vec<KindId> = match doc.pinned {
                Some(k) => vec![k],
                None => doc_ref_lookup_order(project.schema())
                    .filter(|k| *k != LOC_KEY)
                    .collect(),
            };
            // With nothing typed an unqualified request would offer every
            // entity in the project; make the author narrow first.
            if doc.pinned.is_some() || !doc.name_prefix.is_empty() {
                items.extend(crate::completion::doc_ref_symbol_items(
                    project.table(),
                    project.schema(),
                    kinds,
                    &doc.name_prefix,
                ));
            }
            // Edit only the name, so accepting a suggestion keeps `kind:`.
            for item in &mut items {
                if item.text_edit.is_none() {
                    item.text_edit =
                        Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
                            range,
                            new_text: item.label.clone(),
                        }));
                }
            }
            log_info!(
                "doc-ref completion: pinned={:?} prefix={:?} path={rel} items={}",
                doc.pinned.map(KindId::name),
                doc.name_prefix,
                items.len()
            );
            return items;
        }
        let cursor = cursor_context(&src, off);
        if let Some(member) = scope_member_context(&src, off) {
            let mut items = crate::completion::scope_link_items(&member.scope, &member.name_prefix);
            let range = Range::new(
                offset_to_position(&src, member.name_start),
                offset_to_position(&src, off),
            );
            for item in &mut items {
                item.filter_text = Some(format!("{}{}", member.filter_prefix, item.label));
                item.text_edit = Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
                    range,
                    new_text: item.label.clone(),
                }));
            }
            return items;
        }
        if let Some(prefix) = cursor.scope_prefix.as_deref() {
            let name_prefix = cursor.scope_name_prefix.as_deref().unwrap_or_default();
            let mut items = crate::completion::symbol_value_items_matching(
                project.table(),
                project.schema(),
                project.schema().scope_prefix_kinds(prefix, &rel),
                name_prefix,
            );
            // VS Code filters completion candidates using the whole
            // `title:` expression, while the label itself is only the symbol
            // name. Make that filter text explicit, and edit just the name
            // suffix so accepting `k_france` preserves `title:`.
            let suffix_start = cursor.scope_name_start.unwrap_or(off);
            let suffix_range = Range::new(
                offset_to_position(&src, suffix_start),
                offset_to_position(&src, off),
            );
            for item in &mut items {
                item.filter_text = Some(format!("{prefix}:{}", item.label));
                item.text_edit = Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
                    range: suffix_range,
                    new_text: item.label.clone(),
                }));
            }
            log_info!(
                "scope completion: prefix={prefix:?} name_prefix={name_prefix:?} path={rel} items={}",
                items.len()
            );
            return items;
        }
        if let Some(key) = cursor.value_key.as_deref() {
            let items = crate::completion::symbol_value_items(
                project.table(),
                project.schema(),
                project.schema().value_kinds(key, &rel),
            );
            if !items.is_empty() {
                return items;
            }
            // No symbol kind resolves this key: if the enclosing struct
            // declares an enum-like vocabulary for it (`slot = helmet/…`),
            // offer those values.
            let ctx = pdxl_analysis::context::context_of_chain_rooted(
                cursor.chain.iter().map(Vec::as_slice),
                cursor.root_override,
                &rel,
                pdxl_game::contexts::context_schema(),
            );
            if let pdxl_analysis::context::ClauseKind::Struct(spec) = ctx
                && let Some(field) = spec.field(key)
                && let Some(values) = field.values
            {
                return crate::completion::enum_value_items(key, values);
            }
            return Vec::new();
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
        let ctx = pdxl_analysis::context::context_of_chain_rooted(
            cursor.chain.iter().map(Vec::as_slice),
            cursor.root_override,
            &rel,
            pdxl_game::contexts::context_schema(),
        );
        crate::completion::items_for(ctx, project.table(), scope_at(&src, &rel, off).as_deref())
    }

    /// `textDocument/formatting`: one whole-document edit, or an empty list
    /// when already formatted. Files with parse errors return `None` — the
    /// formatter refuses error-recovered trees, and diagnostics already mark
    /// the syntax errors. Client tab options are ignored: PDXScript style
    /// here is always tabs.
    pub fn formatting(&self, uri: &Url) -> Option<Vec<lsp_types::TextEdit>> {
        let path = uri_to_path(uri);
        let src = self.read_file(&path).ok()?;
        let formatted = pdxl_fmt::format(&path.to_string_lossy(), &src).ok()?;
        if formatted.as_bytes() == src.as_slice() {
            return Some(Vec::new());
        }
        Some(vec![lsp_types::TextEdit {
            range: Range {
                start: Position::new(0, 0),
                end: offset_to_position(&src, src.len() as u32),
            },
            new_text: formatted,
        }])
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

/// Collects the contiguous `#!` doc lines directly above the line containing
/// `def_offset`, top-to-bottom, with the `#!` marker and one optional following
/// space stripped. A blank or ordinary line ends the block; `None` if there is
/// no doc line immediately above the definition.
fn extract_doc_block(src: &[u8], def_offset: u32) -> Option<String> {
    let def = (def_offset as usize).min(src.len());
    // Start of the line the definition sits on (past any leading keyword).
    let mut end = src[..def]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);

    let mut docs: Vec<String> = Vec::new();
    while end > 0 {
        // `end - 1` is the '\n' ending the previous line; find that line's start.
        let prev_nl = src[..end - 1]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1);
        let line = &src[prev_nl..end - 1];
        let Some(rest) = line.trim_ascii_start().strip_prefix(b"#!") else {
            break;
        };
        let Ok(s) = std::str::from_utf8(rest) else {
            break;
        };
        docs.push(s.strip_prefix(' ').unwrap_or(s).trim_end().to_string());
        end = prev_nl;
    }
    if docs.is_empty() {
        return None;
    }
    docs.reverse();
    Some(docs.join("\n"))
}

/// Splits a `![…]` ref's inner text into an optional explicit kind and the byte
/// offset where the referenced name begins.
///
/// Owned by `pdxl-analysis` since extraction records doc refs itself; re-exported
/// here for the LSP's own hover/link paths, which rung 5 folds into facts.
pub(crate) use pdxl_analysis::parse_doc_ref;

/// Kinds to try for an unqualified `![Name]`, in decreasing order of how much
/// the author probably meant them: **the smart-doc anchor, then entities, then
/// the game concept, then the localization key.**
///
/// An anchor leads because it is the only kind an author declares *for this
/// purpose* — `#! @name` exists solely to be linked to, so it should win over
/// an entity that merely happens to share the name.
///
/// Both tail kinds are things an entity's own name tends to shadow — a game
/// concept explains a mechanic the entity implements, and a loc string is
/// usually just the entity's display text — so a real definition should win
/// over either. Between the two, a concept is the more specific answer, and its
/// encyclopedia entry is more useful to jump to than the raw string.
fn doc_ref_lookup_order(schema: &Schema) -> impl Iterator<Item = KindId> + '_ {
    let concept = schema.loc_concept_kind();
    std::iter::once(pdxl_analysis::DOC_ANCHOR)
        .chain(schema.kinds().iter().copied().filter(move |k| {
            *k != LOC_KEY && *k != pdxl_analysis::DOC_ANCHOR && Some(*k) != concept
        }))
        .chain(concept)
        .chain(std::iter::once(LOC_KEY))
}

/// A cursor sitting inside an unclosed `![…]` of a `#!` doc comment.
pub(crate) struct DocRefCursor {
    /// Byte offset where the referenced *name* starts — past any `kind:`.
    pub name_start: u32,
    /// What has been typed of the name so far.
    pub name_prefix: String,
    /// The kind a valid `alias:` qualifier pins, when one precedes the cursor.
    pub pinned: Option<KindId>,
    /// Whether any `:` was typed, valid alias or not. Suppresses offering more
    /// qualifiers once the author has clearly chosen one.
    pub qualified: bool,
}

/// Locates a `![…]` under construction at `off`.
///
/// Doc comments are lexer tokens, so this asks the lexer which token contains
/// the cursor rather than re-deriving comment bounds — a `#` inside a string
/// can never be mistaken for one. Returns `None` unless the cursor is inside a
/// `![` that has no closing `]` before it.
pub(crate) fn doc_ref_cursor(src: &[u8], off: u32, schema: &Schema) -> Option<DocRefCursor> {
    let tok = pdxl_lexer::tokenize_all(src).into_iter().find(|t| {
        t.kind == pdxl_lexer::TokenKind::DocComment && t.range.start <= off && off <= t.range.end
    })?;
    // The nearest `![` at or before the cursor, with no `]` closing it first.
    let (lo, hi) = (tok.range.start as usize, off as usize);
    let open = src[lo..hi]
        .windows(2)
        .rposition(|w| w == b"![")
        .map(|i| lo + i + 2)?;
    if src[open..hi].contains(&b']') {
        return None;
    }
    let typed = std::str::from_utf8(&src[open..hi]).ok()?;
    // `![@key]` pins the anchor kind. Checked before the alias split, or a
    // `:`-namespaced key under construction (`![@todo:reb`) would be read as
    // the alias `@todo` and lose everything typed before the colon.
    if let Some(key) = typed.strip_prefix('@') {
        return Some(DocRefCursor {
            name_start: (open + 1) as u32,
            name_prefix: key.to_string(),
            pinned: Some(pdxl_analysis::DOC_ANCHOR),
            qualified: true,
        });
    }
    match typed.split_once(':') {
        Some((alias, name)) => Some(DocRefCursor {
            name_start: (open + alias.len() + 1) as u32,
            name_prefix: name.to_string(),
            pinned: schema.kind_by_alias(alias),
            qualified: true,
        }),
        None => Some(DocRefCursor {
            name_start: open as u32,
            name_prefix: typed.to_string(),
            pinned: None,
            qualified: false,
        }),
    }
}

/// The kind a doc ref pins, or `None` for a bare `![Name]` whose kind is
/// resolved by trying every kind in turn.
fn pinned_kind(r: &pdxl_analysis::Ref) -> Option<KindId> {
    (r.kind != pdxl_analysis::DOC_REF).then_some(r.kind)
}

/// The smart-doc references of one file, in source order.
///
/// Extraction records them in `FileFacts::calls` alongside call-by-name and
/// soft scope refs, so they are picked out by position: a call inside a `#!`
/// token is a doc ref. The lexer hands back those token ranges directly, which
/// is why this needs no gap reconstruction and cannot mistake a `#` inside a
/// string for a comment.
fn doc_refs<'f>(src: &[u8], facts: &'f pdxl_analysis::FileFacts) -> Vec<&'f pdxl_analysis::Ref> {
    if facts.calls.is_empty() && facts.refs.is_empty() {
        return Vec::new();
    }
    let (_, docs) = pdxl_lexer::tokenize_with_docs(src);
    if docs.is_empty() {
        return Vec::new();
    }
    // Anchor references live in `refs` (they are diagnosable) and every other
    // doc ref in `calls`, so both streams are searched. Position decides: a
    // script reference can never fall inside a comment token.
    let mut out: Vec<&pdxl_analysis::Ref> = facts
        .calls
        .iter()
        .chain(facts.refs.iter())
        .filter(|r| {
            docs.binary_search_by(|d| {
                if r.start < d.start {
                    std::cmp::Ordering::Greater
                } else if r.start >= d.end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
        })
        .collect();
    out.sort_by_key(|r| r.start);
    out
}

/// The byte offset of the field reached by walking `field_path` (dot-separated
/// keys) into the definition at `def_offset`, or `None` if any segment is
/// missing. Parses `src` on demand — only reached for a `Symbol.field` ref.
fn field_offset_in(src: &[u8], def_offset: u32, field_path: &str) -> Option<u32> {
    let parsed = pdxl_parser::parse(String::new(), src.to_vec());
    let tree = parsed.tree();
    let mut node = find_field_at(tree, tree.root(), def_offset)?;
    for segment in field_path.split('.') {
        let kids = tree.child_ids(node);
        if kids.len() != 2 {
            return None; // not a `key = { … }` block to descend into
        }
        node = find_child_field(tree, kids[1], segment.as_bytes())?;
    }
    let kids = tree.child_ids(node);
    Some(tree.node(*kids.first()?).range.start)
}

/// The `Field` node whose start is exactly `offset` (a definition's node).
fn find_field_at(
    tree: &pdxl_ast::SyntaxTree,
    node_id: pdxl_ast::NodeId,
    offset: u32,
) -> Option<pdxl_ast::NodeId> {
    let node = tree.node(node_id);
    if node.kind == pdxl_ast::NodeKind::Field && node.range.start == offset {
        return Some(node_id);
    }
    for child in tree.children(node_id) {
        if let Some(found) = find_field_at(tree, child, offset) {
            return Some(found);
        }
    }
    None
}

/// The direct-child `Field` of `block_id` whose key equals `key`.
fn find_child_field(
    tree: &pdxl_ast::SyntaxTree,
    block_id: pdxl_ast::NodeId,
    key: &[u8],
) -> Option<pdxl_ast::NodeId> {
    for child in tree.children(block_id) {
        if tree.node(child).kind != pdxl_ast::NodeKind::Field {
            continue;
        }
        let kids = tree.child_ids(child);
        if kids.first().is_some_and(|&k| tree.node_text(k) == key) {
            return Some(child);
        }
    }
    None
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
    /// The resolved scope inside this block (inherited unless changed).
    scope: Option<String>,
    /// The scope actually *displayed* by an inlay hint at or above this frame,
    /// so a descendant with the same scope doesn't repeat it. `None` in
    /// `scope_at` (which has no hints).
    shown: Option<String>,
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

    let tokens = pdxl_lexer::tokenize(src);
    for (i, &token) in tokens.iter().enumerate() {
        let is_event_body = event_body_context(&stack, rel_path);
        match token.kind {
            T::LBrace => {
                let key = block_key(src, &recent, &is_scalar, &is_op);
                let parent_scope = stack.last().and_then(|frame| frame.scope.clone());
                let parent_shown = stack.last().and_then(|frame| frame.shown.clone());
                let scope = block_scope(
                    &tokens[..i],
                    src,
                    &recent,
                    &key,
                    &stack,
                    rel_path,
                    parent_scope.clone(),
                );
                let shown = if let Some(scope) = &scope {
                    // The clause kind of this block's contents, used both to
                    // decide whether a scope hint is meaningful and to suffix
                    // it: `: character (trigger)`, `: landed_title (effect)`.
                    let clause = {
                        let mut chain: Vec<&[u8]> =
                            stack.iter().map(|f| f.key.as_slice()).collect();
                        chain.push(&key);
                        pdxl_analysis::context::context_of_chain(
                            chain,
                            rel_path,
                            pdxl_game::contexts::context_schema(),
                        )
                    };
                    // Show a hint when this block *changes* the scope (a
                    // `title:`/`scope:` literal, an iterator) — always the useful
                    // signal — or when a scope-bearing clause surfaces a scope
                    // not yet displayed above, so nested effect/trigger blocks
                    // that merely inherit it (`random_list`, `add_trait_xp`) and
                    // structural blocks (`cooldown`) stay quiet.
                    let changed = parent_scope.as_deref() != Some(scope.as_str());
                    let already_shown = parent_shown.as_deref() == Some(scope.as_str());
                    let show = changed || (scope_hint_meaningful(clause) && !already_shown);
                    if show {
                        let position = offset_to_position(src, token.range.end);
                        if position_in_range(position, requested) {
                            let label = match clause_suffix(clause) {
                                Some(c) => format!(": {scope} ({c})"),
                                None => format!(": {scope}"),
                            };
                            hints.push(InlayHint {
                                position,
                                label: label.into(),
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
                        Some(scope.clone())
                    } else {
                        parent_shown
                    }
                } else {
                    parent_shown
                };
                stack.push(ScopeFrame { key, scope, shown });
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
    let tokens = pdxl_lexer::tokenize(src);
    for (i, &token) in tokens.iter().enumerate() {
        if token.range.start >= off {
            break;
        }
        let is_event_body = event_body_context(&stack, rel_path);
        match token.kind {
            T::LBrace => {
                let key = block_key(src, &recent, &is_scalar, &is_op);
                let inherited = stack.last().and_then(|frame| frame.scope.clone());
                let scope = block_scope(
                    &tokens[..i],
                    src,
                    &recent,
                    &key,
                    &stack,
                    rel_path,
                    inherited,
                );
                // `shown` is a hint-display concern; unused in scope_at.
                stack.push(ScopeFrame {
                    key,
                    scope,
                    shown: None,
                });
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

/// Event `type` values establish the implicit root inherited by trigger and
/// effect blocks. `age_event` remains unknown: EU5's dumped effect/trigger
/// tables expose no `age` scope, and the lone vanilla event does not prove
/// whether its script root is an age or the viewing country.
fn event_type_scope(value: &[u8]) -> Option<&'static str> {
    match value {
        b"character_event" | b"letter_event" => Some("character"),
        b"country_event" => Some("country"),
        b"location_event" => Some("location"),
        b"unit_event" => Some("unit"),
        b"exploration_event" => Some("exploration"),
        _ => None,
    }
}

fn event_body_context(stack: &[ScopeFrame], rel_path: &str) -> bool {
    matches!(
        pdxl_analysis::context::context_of_chain(
            stack.iter().map(|frame| frame.key.as_slice()),
            rel_path,
            pdxl_game::contexts::context_schema(),
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
    before: &[pdxl_lexer::Token],
    src: &[u8],
    recent: &[pdxl_lexer::Token],
    key: &[u8],
    stack: &[ScopeFrame],
    rel_path: &str,
    inherited: Option<String>,
) -> Option<String> {
    if let Some(scope) = scope_link_chain_scope(before, src) {
        return Some(scope);
    }
    let parent_keys = stack.iter().map(|frame| frame.key.as_slice());
    let context = pdxl_analysis::context::context_of_chain(
        parent_keys,
        rel_path,
        pdxl_game::contexts::context_schema(),
    );
    let key = std::str::from_utf8(key).ok()?;
    // A structural field can pin a fixed root scope its script can't infer
    // (law `can_keep` → character, `can_title_have` → landed_title).
    if let ClauseKind::Struct(spec) = context
        && let Some(field) = spec.field(key)
        && let Some(scope) = field.scope
    {
        return Some(scope.to_string());
    }
    let rows = match context {
        ClauseKind::Effect => pdxl_game::tables::EFFECTS,
        ClauseKind::Trigger => pdxl_game::tables::TRIGGERS,
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
        if let Some(link) = pdxl_game::tables::SCOPE_LINKS.iter().find(|link| {
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

/// Resolves a complete `prefix:data.member…` chain immediately before `off`.
/// It powers both scope hints and scope-aware completions at block openers.
fn scope_link_chain_scope(before: &[pdxl_lexer::Token], src: &[u8]) -> Option<String> {
    use pdxl_lexer::TokenKind as T;
    let scalar = |kind: T| {
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
    // The chain is the run of scalar/`:`/`.` tokens immediately before the
    // brace. Scanned *backward* from the brace so this is O(chain), not
    // O(file) — the previous forward scan re-walked every token before the
    // brace, which is quadratic when called once per block (inlay hints on a
    // large file). When the brace is preceded by `=`/`?=` (the
    // `scope:x = { }` form) the run sits before the operator, and only counts
    // as a scope chain if it contains a `:`.
    let mut end = before.len();
    while end > 0 && before[end - 1].kind == T::Comment {
        end -= 1;
    }
    let require_colon = end > 0 && matches!(before[end - 1].kind, T::Equal | T::QuestionEqual);
    if require_colon {
        end -= 1;
        while end > 0 && before[end - 1].kind == T::Comment {
            end -= 1;
        }
    }
    let mut start = end;
    while start > 0
        && (scalar(before[start - 1].kind)
            || matches!(before[start - 1].kind, T::Colon | T::Dot | T::Comment))
    {
        start -= 1;
    }
    let chain: Vec<pdxl_lexer::Token> = before[start..end]
        .iter()
        .copied()
        .filter(|token| token.kind != T::Comment)
        .collect();
    if require_colon && !chain.iter().any(|token| token.kind == T::Colon) {
        return None;
    }
    if chain.len() < 3
        || !scalar(chain[0].kind)
        || chain[1].kind != T::Colon
        || !scalar(chain[2].kind)
    {
        return None;
    }
    let prefix = std::str::from_utf8(token_text(src, chain[0])).ok()?;
    let first = pdxl_game::tables::SCOPE_LINKS
        .iter()
        .find(|link| link.name == prefix && link.requires_data && link.output_scopes.len() == 1)?;
    let mut scope = first.output_scopes[0];
    let mut i = 3;
    while i < chain.len() {
        if chain[i].kind != T::Dot || i + 1 >= chain.len() || !scalar(chain[i + 1].kind) {
            return None;
        }
        let name = std::str::from_utf8(token_text(src, chain[i + 1])).ok()?;
        let link = pdxl_game::tables::SCOPE_LINKS.iter().find(|link| {
            link.name == name
                && !link.requires_data
                && !link.global_link
                && link.input_scopes.contains(&scope)
                && link.output_scopes.len() == 1
        })?;
        scope = link.output_scopes[0];
        i += 2;
    }
    Some(scope.to_string())
}

fn token_text(src: &[u8], token: pdxl_lexer::Token) -> &[u8] {
    &src[token.range.start as usize..token.range.end as usize]
}

fn position_in_range(position: Position, range: Range) -> bool {
    position >= range.start && position <= range.end
}

/// Built-in documentation is intentionally a token query, not an AST query:
/// it remains useful while the user is typing incomplete script.
/// Short clause-kind tag for scope inlay hints (`character (trigger)`), or
/// `None` for non-clause blocks (structs, config) where a tag adds no value.
fn clause_suffix(ctx: ClauseKind) -> Option<&'static str> {
    use pdxl_analysis::context::Fallback;
    match ctx {
        ClauseKind::Effect => Some("effect"),
        ClauseKind::Trigger => Some("trigger"),
        ClauseKind::ScriptValue => Some("value"),
        ClauseKind::ScriptedModifier => Some("modifier"),
        // A struct whose loose keys are effects/triggers reads as that clause
        // (an event `option` holds effects → `: character (effect)`).
        ClauseKind::Struct(spec) => match spec.fallback {
            Fallback::Effect => Some("effect"),
            Fallback::Trigger => Some("trigger"),
            _ => None,
        },
        _ => None,
    }
}

/// Whether a block's clause can carry scope-navigating script (`root`,
/// `scope:x`, `.holder` chains), so a scope inlay hint is useful there. Pure
/// structural blocks — an event `cooldown` (only `days/months/years`), a
/// `portrait`/`widget` config — inherit a scope but never use it, so labeling
/// them is noise.
fn scope_hint_meaningful(clause: ClauseKind) -> bool {
    use pdxl_analysis::context::Fallback;
    match clause {
        ClauseKind::Effect
        | ClauseKind::Trigger
        | ClauseKind::ScriptValue
        | ClauseKind::ScriptedModifier => true,
        // A struct block bears scope only through an effect/trigger fallback
        // (an event `option`'s loose effects); a Deny/data struct does not.
        ClauseKind::Struct(spec) => matches!(spec.fallback, Fallback::Effect | Fallback::Trigger),
        _ => false,
    }
}

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
        let link = pdxl_game::tables::SCOPE_LINKS
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

    let cursor = cursor_context(src, off);
    let ctx = pdxl_analysis::context::context_of_chain_rooted(
        cursor.chain.iter().map(Vec::as_slice),
        cursor.root_override,
        rel_path,
        pdxl_game::contexts::context_schema(),
    );
    // A structural field key (law `can_keep`, event `immediate`, …): describe
    // the clause it opens and the root scope, if pinned.
    if let ClauseKind::Struct(spec) = ctx
        && let Some(field) = spec.field(name)
    {
        let mut text = format!("```pdxscript\n{} field {name}\n```", spec.name);
        // A compact type line (clause it opens + fixed scope), then the
        // distilled `_*.info` documentation when we have it.
        let mut tags = Vec::new();
        if let Some(kind) = field.block
            && let Some(short) = clause_suffix(kind)
        {
            tags.push(short.to_string());
        }
        if let Some(scope) = field.scope {
            tags.push(format!("root scope `{scope}`"));
        }
        if !tags.is_empty() {
            text.push_str(&format!("\n\n*{}*", tags.join(" · ")));
        }
        if let Some(doc) = field.doc {
            text.push_str(&format!("\n\n{doc}"));
        }
        if let Some(values) = field.values {
            let list = values
                .iter()
                .map(|v| format!("`{v}`"))
                .collect::<Vec<_>>()
                .join(" · ");
            text.push_str(&format!("\n\nValues: {list}"));
        }
        return Some(markdown_hover(
            src,
            (token.range.start, token.range.end),
            text,
        ));
    }
    // Classify the key itself: within a struct context, an unknown key may
    // resolve through its fallback (effects in event options, modifier tags in
    // advances, etc.). Scalar/block form does not affect those two fallbacks.
    let key_ctx = pdxl_analysis::context::resolve_key(ctx, name, false);
    // In a static-modifier body (including a Modifier struct fallback), a key
    // is a built-in modifier tag.
    if matches!(ctx, ClauseKind::StaticModifier) || matches!(key_ctx, ClauseKind::StaticModifier) {
        let row = pdxl_game::tables::MODIFIERS
            .iter()
            .find(|row| row.tag == name)?;
        let text = format!(
            "```pdxscript\nmodifier {name}\n```\n\nUsed in: {}",
            row.use_areas.join(", ")
        );
        return Some(markdown_hover(
            src,
            (token.range.start, token.range.end),
            text,
        ));
    }
    let (label, row) = match key_ctx {
        ClauseKind::Effect => (
            "effect",
            pdxl_game::tables::EFFECTS
                .iter()
                .find(|row| row.name == name)?,
        ),
        ClauseKind::Trigger => (
            "trigger",
            pdxl_game::tables::TRIGGERS
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
    /// Body clause of the outermost enclosing block when it is an inline typed
    /// definition (`scripted_effect NAME = { … }`), overriding the file's
    /// directory-derived root. `None` for ordinary blocks.
    root_override: Option<ClauseKind>,
    value_key: Option<String>,
    scope_prefix: Option<String>,
    scope_name_prefix: Option<String>,
    scope_name_start: Option<u32>,
}

/// The `.member` suffix of a scope-link chain, including the scope reached
/// immediately before that suffix. `title:k_france.` therefore has the
/// `landed_title` scope and offers links such as `holder`.
struct ScopeMemberContext {
    scope: String,
    name_prefix: String,
    name_start: u32,
    filter_prefix: String,
}

fn scope_member_context(src: &[u8], off: u32) -> Option<ScopeMemberContext> {
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
    let mut chain = Vec::new();
    for token in pdxl_lexer::tokenize(src) {
        if token.range.start >= off {
            break;
        }
        if is_scalar(token.kind) || matches!(token.kind, T::Colon | T::Dot) {
            chain.push(token);
        } else if token.kind != T::Comment {
            chain.clear();
        }
    }
    // `prefix:data(.link)*` must end in `.` or a partially typed member.
    if chain.len() < 4 || !is_scalar(chain[0].kind) || chain[1].kind != T::Colon {
        return None;
    }
    let mut index = 2;
    if !is_scalar(chain[index].kind) {
        return None;
    }
    let link_name = std::str::from_utf8(token_text(src, chain[0])).ok()?;
    let link = pdxl_game::tables::SCOPE_LINKS.iter().find(|link| {
        link.name == link_name && link.requires_data && link.output_scopes.len() == 1
    })?;
    let mut scope = link.output_scopes[0];
    index += 1;
    while index < chain.len() && chain[index].kind == T::Dot {
        let dot = chain[index];
        index += 1;
        if index == chain.len() {
            return Some(ScopeMemberContext {
                scope: scope.to_string(),
                name_prefix: String::new(),
                name_start: dot.range.end,
                filter_prefix: String::from_utf8(
                    src[chain[0].range.start as usize..off as usize].to_vec(),
                )
                .ok()?,
            });
        }
        if !is_scalar(chain[index].kind) {
            return None;
        }
        // A final scalar is what the user is completing; earlier members
        // advance the chain's scope before the next dot.
        if index + 1 == chain.len() {
            return Some(ScopeMemberContext {
                scope: scope.to_string(),
                name_prefix: std::str::from_utf8(token_text(src, chain[index]))
                    .ok()?
                    .to_string(),
                name_start: chain[index].range.start,
                filter_prefix: String::from_utf8(
                    src[chain[0].range.start as usize..chain[index].range.start as usize].to_vec(),
                )
                .ok()?,
            });
        }
        let name = std::str::from_utf8(token_text(src, chain[index])).ok()?;
        let link = pdxl_game::tables::SCOPE_LINKS.iter().find(|link| {
            link.name == name
                && !link.requires_data
                && !link.global_link
                && link.input_scopes.contains(&scope)
                && link.output_scopes.len() == 1
        })?;
        scope = link.output_scopes[0];
        index += 1;
    }
    None
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
    let mut root_override: Option<ClauseKind> = None;
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
                // The outermost enclosing block sets the root clause. A typed-
                // def keyword before its key (`scripted_effect NAME = {`) makes
                // that body an Effect/Trigger clause regardless of directory.
                if stack.is_empty() {
                    root_override = (n >= 3 && is_op(recent[n - 1].kind))
                        .then(|| {
                            let kw = &src[recent[n - 3].range.start as usize
                                ..recent[n - 3].range.end as usize];
                            match kw {
                                b"scripted_effect" => Some(ClauseKind::Effect),
                                b"scripted_trigger" => Some(ClauseKind::Trigger),
                                _ => None,
                            }
                        })
                        .flatten();
                }
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
    let scope_name_prefix = if recent.len() >= 3
        && is_scalar(recent[recent.len() - 3].kind)
        && recent[recent.len() - 2].kind == T::Colon
        && is_scalar(recent[recent.len() - 1].kind)
    {
        std::str::from_utf8(
            &src[recent[recent.len() - 1].range.start as usize
                ..recent[recent.len() - 1].range.end as usize],
        )
        .ok()
        .map(str::to_owned)
    } else {
        None
    };
    let scope_name_start = if recent.len() >= 3
        && is_scalar(recent[recent.len() - 3].kind)
        && recent[recent.len() - 2].kind == T::Colon
        && is_scalar(recent[recent.len() - 1].kind)
    {
        Some(recent[recent.len() - 1].range.start)
    } else if recent.len() >= 2
        && is_scalar(recent[recent.len() - 2].kind)
        && recent[recent.len() - 1].kind == T::Colon
    {
        Some(recent[recent.len() - 1].range.end)
    } else {
        None
    };
    CursorContext {
        chain: stack,
        root_override,
        value_key,
        scope_prefix,
        scope_name_prefix,
        scope_name_start,
    }
}

/// The (kind, name) of the definition name or reference spanning byte offset
/// `off`. Definitions are checked first so the cursor on a `NAME = {}` name
/// resolves to that symbol (Go's `symbolAt`).
fn symbol_at(facts: &pdxl_analysis::FileFacts, off: u32) -> Option<(pdxl_analysis::KindId, &str)> {
    symbol_at_with_alts(facts, off).map(|(k, _, n)| (k, n))
}

/// Like [`symbol_at`], but keeps the reference's alternate-kind list so
/// consumers can resolve against whichever kind defines the name.
fn symbol_at_with_alts(
    facts: &pdxl_analysis::FileFacts,
    off: u32,
) -> Option<(
    pdxl_analysis::KindId,
    &'static [pdxl_analysis::KindId],
    &str,
)> {
    for d in &facts.defs {
        if d.offset <= off && off < d.end_offset {
            return Some((d.kind, &[], &d.name));
        }
    }
    for r in &facts.refs {
        if r.start <= off && off < r.end {
            return Some((r.kind, r.alt, &r.name));
        }
    }
    // Call-by-name sites (`my_effect = yes`): the key resolves to the scripted
    // effect/trigger, enabling go-to-definition and find-references from a call.
    for r in &facts.calls {
        if r.start <= off && off < r.end {
            return Some((r.kind, r.alt, &r.name));
        }
    }
    // File-local script constants: both the `@name = …` definition and its
    // `= @name` uses answer, so find-references works from either end.
    for d in &facts.constants {
        if d.offset <= off && off < d.end_offset {
            return Some((d.kind, &[], &d.name));
        }
    }
    for r in &facts.constant_refs {
        if r.start <= off && off < r.end {
            return Some((r.kind, r.alt, &r.name));
        }
    }
    // Gap-fill names (`DefShape::ScopedChildrenOf`, alias field keys) answer
    // last, mirroring `SymbolTable::add_alias`: a real definition or reference
    // at the same position always wins.
    for a in &facts.aliases {
        if a.offset <= off && off < a.end_offset {
            return Some((a.kind, &[], &a.name));
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
    for r in &facts.calls {
        if r.start <= off && off < r.end {
            return Some((r.start, r.end));
        }
    }
    for d in &facts.constants {
        if d.offset <= off && off < d.end_offset {
            return Some((d.offset, d.end_offset));
        }
    }
    for r in &facts.constant_refs {
        if r.start <= off && off < r.end {
            return Some((r.start, r.end));
        }
    }
    // Same last-place ordering as `symbol_at_with_alts`.
    for a in &facts.aliases {
        if a.offset <= off && off < a.end_offset {
            return Some((a.offset, a.end_offset));
        }
    }
    None
}

/// Narrows a value span to exclude surrounding quotes, so a rename selects /
/// rewrites the identifier rather than `"identifier"`. Reference spans cover the
/// whole value node (quotes included); definition spans never are.
fn unquoted_span(src: &[u8], start: u32, end: u32) -> (u32, u32) {
    if end > start + 1
        && src.get(start as usize) == Some(&b'"')
        && src.get(end as usize - 1) == Some(&b'"')
    {
        (start + 1, end - 1)
    } else {
        (start, end)
    }
}

/// Cap on `workspace/symbol` results — enough to be useful, bounded so a broad
/// query over a CK3-scale table (tens of thousands of symbols) stays cheap.
const WORKSPACE_SYMBOL_LIMIT: usize = 256;

/// Fuzzy match `query` against a symbol `name` (case-insensitive), returning a
/// rank score (higher = better) or `None` when `query` is not even a
/// subsequence. Bands: exact > prefix > word-boundary substring > substring >
/// subsequence. An empty query matches everything at a flat score.
fn fuzzy_score(query: &str, name: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let q = query.to_ascii_lowercase();
    let n = name.to_ascii_lowercase();
    if n == q {
        return Some(1000);
    }
    if let Some(pos) = n.find(&q) {
        // Substring: prefer early matches, with prefix / word-boundary bonuses.
        let mut score = 500 - (pos as i32).min(300);
        if pos == 0 {
            score += 300;
        } else if n.as_bytes().get(pos - 1) == Some(&b'_') {
            score += 150;
        }
        return Some(score);
    }
    subsequence_score(q.as_bytes(), n.as_bytes())
}

/// Scores an in-order subsequence match (`sc` in `scarab` → yes) or `None`.
/// Rewards adjacency and matches at word boundaries so `sf` favors
/// `scripted_effect` over a scattered hit.
fn subsequence_score(q: &[u8], n: &[u8]) -> Option<i32> {
    let mut qi = 0;
    let mut score = 0i32;
    let mut last: Option<usize> = None;
    for (i, &c) in n.iter().enumerate() {
        if qi < q.len() && c == q[qi] {
            if last == Some(i.wrapping_sub(1)) {
                score += 5; // contiguous run
            }
            if i == 0 || n[i - 1] == b'_' {
                score += 3; // word boundary
            }
            last = Some(i);
            qi += 1;
        }
    }
    (qi == q.len()).then_some(score)
}

/// Whether `name` is a usable rename target: a non-empty identifier with no
/// whitespace or PDXScript-structural characters (quotes, braces, `=`, `#`).
fn is_valid_rename(name: &str) -> bool {
    !name.is_empty()
        && !name
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '"' | '=' | '{' | '}' | '#'))
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
        I::Text => lsp_types::SymbolKind::STRING,
    }
}

/// Hover for a `[…]` datafunction segment in a `.gui` source: parses the
/// file with the interface dialect, finds the expression span containing
/// `off`, resolves the chain against the DumpDataTypes registry, and renders
/// the segment under the cursor.
fn gui_datafn_hover(src: &[u8], off: u32) -> Option<lsp_types::Hover> {
    use pdxl_gui::datafn;
    let parsed = pdxl_gui::parse(String::new(), src.to_vec());
    let tree = parsed.tree();
    let registry = pdxl_game::datafn_registry();
    for span in datafn::datafn_spans(tree) {
        if off < span.start || off >= span.end {
            continue;
        }
        let text = &src[span.start as usize..span.end as usize];
        let segments = datafn::parse_chain(text, span.start)?;
        let (resolved, _err) = datafn::resolve_chain(&segments, registry);
        let idx = segments
            .iter()
            .position(|s| off >= s.name_start && off < s.name_end)?;
        let seg = &segments[idx];
        let info = resolved.get(idx)?;
        let mut text = match info.row {
            Some(row) => {
                let owner = if row.owner.is_empty() {
                    String::new()
                } else {
                    format!("{}.", row.owner)
                };
                let args = if row.args > 0 {
                    let names: Vec<String> = (0..row.args).map(|i| format!("Arg{i}")).collect();
                    format!("( {} )", names.join(", "))
                } else {
                    String::new()
                };
                let mut t = format!(
                    "```pdxscript\n{owner}{}{args} → {}\n```\n\n*{}*",
                    row.name,
                    row.ret,
                    row.kind.label()
                );
                if !row.desc.is_empty() {
                    t.push_str(&format!("\n\n{}", row.desc));
                }
                t
            }
            None if idx == 0 && registry.is_type(&seg.name) => format!(
                "```pdxscript\n{}\n```\n\n*data type* — reads the narrowest enclosing \
                 datacontext of this type",
                seg.name
            ),
            None => return None,
        };
        text.push('\n');
        return Some(markdown_hover(src, (seg.name_start, seg.name_end), text));
    }
    None
}

/// Hover for a gui property key: the identifier under the cursor when it is
/// in key position (followed by `=`), documented in the curated table.
fn gui_property_hover(src: &[u8], off: u32) -> Option<lsp_types::Hover> {
    let off = off as usize;
    if off >= src.len() {
        return None;
    }
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    if !(is_word(src[off]) || (off > 0 && is_word(src[off - 1]))) {
        return None;
    }
    let mut start = off;
    while start > 0 && is_word(src[start - 1]) {
        start -= 1;
    }
    let mut end = off;
    while end < src.len() && is_word(src[end]) {
        end += 1;
    }
    // Key position: the next non-space byte is `=`.
    let mut j = end;
    while j < src.len() && (src[j] == b' ' || src[j] == b'\t') {
        j += 1;
    }
    if src.get(j) != Some(&b'=') {
        return None;
    }
    let key = std::str::from_utf8(&src[start..end]).ok()?;
    let doc = pdxl_gui::docs::property_doc(key)?;
    let text = format!("```pdxscript\n{key}\n```\n\n{doc}\n");
    Some(markdown_hover(src, (start as u32, end as u32), text))
}

#[cfg(test)]
mod tests {
    use super::extract_doc_block;

    /// Offset of `needle`'s start in `src`.
    fn off(src: &str, needle: &str) -> u32 {
        src.find(needle).unwrap() as u32
    }

    #[test]
    fn doc_block_above_typed_def_and_no_space_marker() {
        // Offset points at the NAME, mid-line after the keyword; the scan must
        // back up to the line start, then collect the `#!` lines above.
        let src = "#!first\n#! second\nscripted_effect my_fx = { }\n";
        let got = extract_doc_block(src.as_bytes(), off(src, "my_fx"));
        assert_eq!(got.as_deref(), Some("first\nsecond"));
    }

    #[test]
    fn no_block_without_marker_or_across_blank_line() {
        let src = "#! detached\n\nfx = { }\n";
        assert_eq!(extract_doc_block(src.as_bytes(), off(src, "fx")), None);
        let plain = "# just a comment\nfx = { }\n";
        assert_eq!(extract_doc_block(plain.as_bytes(), off(plain, "fx")), None);
    }
}
