//! The gui datafunction language: `[GetPlayer.MakeScope.ScriptValue('x')|0]`.
//!
//! Datafunctions appear in two positions in `.gui` files — a bare bracket
//! value (`enabled = [ArmyWindow.CanMerge]`) and embedded in quoted text
//! (`raw_text = "[Character.GetUIName] rules"`). Both reduce to a **chain**
//! of dot-separated segments, each a promote or function call, plus an
//! optional `|format` suffix.
//!
//! Typing rules (validated on the whole vanilla + T4N corpus, 25.5k
//! expressions, 3 genuine failures):
//! - segment 0 resolves as a **global promote/function/macro** or as a
//!   **registered type name** (datacontext access: `[Character.GetUIName]`
//!   reads the narrowest enclosing datacontext of that type);
//! - segment *n+1* resolves as a **member** of the previous return type;
//! - a return type of `[unregistered]` (or one the registry has no members
//!   for) ends checking — the tail is silently accepted.
//!
//! The registry is built from the game's `DumpDataTypes` dump (rendered into
//! a static table by `pdxl-gamedocs`' gen-tables; the row/kind types live
//! here so game crates and tables share one definition).

use std::collections::{HashMap, HashSet};

use pdxl_ast::{NodeKind, SyntaxTree};

/// What one `DumpDataTypes` entry defines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataFnKind {
    /// A registered data type.
    Type,
    /// A global promote (`GetPlayer`) — chain roots.
    GlobalPromote,
    /// A global function (`GetTitleByKey( Arg0 )`).
    GlobalFunction,
    /// A global macro (typed like a function).
    GlobalMacro,
    /// A member promote (`Character.GetLiege`).
    Promote,
    /// A member function (`Character.GetHeldTitle( Arg0 )`).
    Function,
}

impl DataFnKind {
    /// A short human label for hovers.
    pub fn label(self) -> &'static str {
        match self {
            DataFnKind::Type => "type",
            DataFnKind::GlobalPromote => "global promote",
            DataFnKind::GlobalFunction => "global function",
            DataFnKind::GlobalMacro => "global macro",
            DataFnKind::Promote => "promote",
            DataFnKind::Function => "function",
        }
    }
}

/// One `DumpDataTypes` entry: a type, a global promote/function, or a member
/// promote/function of one type. The generated tables are slices of these.
#[derive(Clone, Copy, Debug)]
pub struct DataFnRow {
    /// The owning type for members; empty for globals and types.
    pub owner: &'static str,
    pub name: &'static str,
    pub kind: DataFnKind,
    /// Declared argument count.
    pub args: u8,
    /// The return type; `"[unregistered]"` ends chain typing.
    pub ret: &'static str,
    pub desc: &'static str,
}

/// The compiled datafunction lookup: global names, per-type members, and the
/// registered type set. Build once from the generated table and share.
#[derive(Debug, Default)]
pub struct DataFnRegistry {
    globals: HashMap<&'static str, &'static DataFnRow>,
    members: HashMap<(&'static str, &'static str), &'static DataFnRow>,
    types: HashSet<&'static str>,
}

impl DataFnRegistry {
    pub fn from_rows(rows: &'static [DataFnRow]) -> DataFnRegistry {
        // The dump contains overloads (`GetPlayer` is both a Global promote →
        // Character and a Global function → `[unregistered]`); keep the entry
        // with the *registered* return type so chains stay typable.
        fn prefer(existing: &'static DataFnRow, new: &'static DataFnRow) -> &'static DataFnRow {
            if existing.ret != "[unregistered]" {
                existing
            } else {
                new
            }
        }
        let mut reg = DataFnRegistry::default();
        for row in rows {
            match row.kind {
                DataFnKind::Type => {
                    reg.types.insert(row.name);
                }
                DataFnKind::GlobalPromote
                | DataFnKind::GlobalFunction
                | DataFnKind::GlobalMacro => {
                    reg.globals
                        .entry(row.name)
                        .and_modify(|e| *e = prefer(e, row))
                        .or_insert(row);
                }
                DataFnKind::Promote | DataFnKind::Function => {
                    reg.members
                        .entry((row.owner, row.name))
                        .and_modify(|e| *e = prefer(e, row))
                        .or_insert(row);
                }
            }
        }
        reg
    }

    pub fn is_type(&self, name: &str) -> bool {
        self.types.contains(name)
    }

    pub fn global(&self, name: &str) -> Option<&'static DataFnRow> {
        self.globals.get(name).copied()
    }

    pub fn member(&self, owner: &str, name: &str) -> Option<&'static DataFnRow> {
        self.members.get(&(owner, name)).copied()
    }

    /// Every member promote/function of `owner` (unordered).
    pub fn members_of<'a>(&'a self, owner: &str) -> impl Iterator<Item = &'static DataFnRow> + 'a {
        let owner = owner.to_string();
        self.members
            .iter()
            .filter(move |((o, _), _)| *o == owner)
            .map(|(_, row)| *row)
    }

    /// Every global promote/function (unordered) — chain-root candidates.
    pub fn globals_iter(&self) -> impl Iterator<Item = &'static DataFnRow> + '_ {
        self.globals.values().copied()
    }

    /// Every registered type name (unordered).
    pub fn type_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.types.iter().copied()
    }

    /// Whether `ty` is a chain-typable receiver: registered and known to have
    /// members. An `[unregistered]` or member-less return type ends checking.
    fn can_type_through(&self, ty: &str) -> bool {
        self.types.contains(ty)
    }
}

/// One datafunction expression found in a file: the byte span of the text
/// between the brackets (exclusive of `[` and `]`).
#[derive(Clone, Copy, Debug)]
pub struct DataFnSpan {
    pub start: u32,
    pub end: u32,
}

/// Collects every datafunction expression span in a parsed `.gui` tree:
/// bare bracket scalars (`= [Fn.Chain]`) and `[…]` runs embedded in quoted
/// string scalars. Comments never reach the tree, so no comment handling.
pub fn datafn_spans(tree: &SyntaxTree) -> Vec<DataFnSpan> {
    let mut spans = Vec::new();
    let src = tree.source();
    for node in tree.nodes() {
        if node.kind != NodeKind::Scalar {
            continue;
        }
        let text = &src[node.range.start as usize..node.range.end as usize];
        let base = node.range.start;
        if text.starts_with(b"[") {
            // Bare form: the whole scalar is `[…]`.
            let end = if text.ends_with(b"]") {
                text.len() - 1
            } else {
                text.len()
            };
            spans.push(DataFnSpan {
                start: base + 1,
                end: base + end as u32,
            });
        } else if text.starts_with(b"\"") {
            // Embedded form(s) inside a quoted string.
            let mut i = 0;
            while i < text.len() {
                if text[i] == b'[' {
                    let open = i + 1;
                    let close = text[open..]
                        .iter()
                        .position(|&b| b == b']')
                        .map(|p| open + p);
                    let Some(close) = close else { break };
                    // `[[` escapes a literal bracket in text properties.
                    if text.get(open) != Some(&b'[') {
                        spans.push(DataFnSpan {
                            start: base + open as u32,
                            end: base + close as u32,
                        });
                    }
                    i = close + 1;
                } else {
                    i += 1;
                }
            }
        }
    }
    spans
}

/// One parsed chain segment, with the absolute byte span of its name.
#[derive(Clone, Debug)]
pub struct Segment {
    pub name: String,
    pub name_start: u32,
    pub name_end: u32,
    /// Number of top-level arguments in `( … )`, or `None` without parens.
    pub args: Option<u8>,
}

/// Parses the chain inside one datafunction span. `text` is the expression
/// text (between the brackets); `base` its absolute start offset. The
/// `|format` suffix and argument contents are not analyzed. Returns `None`
/// for expressions that are not chains (empty, or loc-style `'…'` starts).
pub fn parse_chain(text: &[u8], base: u32) -> Option<Vec<Segment>> {
    let end = top_level_pipe(text).unwrap_or(text.len());
    let expr = &text[..end];
    if expr.is_empty() || expr[0] == b'\'' {
        return None;
    }
    let mut segments = Vec::new();
    let mut i = 0;
    loop {
        // Identifier.
        let name_start = i;
        while i < expr.len() && (expr[i].is_ascii_alphanumeric() || expr[i] == b'_') {
            i += 1;
        }
        if i == name_start {
            return None; // not an identifier where one is required
        }
        let name_end = i;
        let name = String::from_utf8_lossy(&expr[name_start..name_end]).into_owned();
        let mut args = None;
        // Optional argument list; count top-level commas, skip quotes/parens.
        if i < expr.len() && expr[i] == b'(' {
            let mut depth = 1;
            let mut count: u8 = 0;
            let mut any = false;
            let mut quote = false;
            i += 1;
            while i < expr.len() && depth > 0 {
                let b = expr[i];
                if quote {
                    quote = b != b'\'';
                } else {
                    match b {
                        b'\'' => quote = true,
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        b',' if depth == 1 => count += 1,
                        b if depth >= 1 && !b.is_ascii_whitespace() => any = true,
                        _ => {}
                    }
                }
                i += 1;
            }
            args = Some(if any { count.saturating_add(1) } else { 0 });
        }
        segments.push(Segment {
            name,
            name_start: base + name_start as u32,
            name_end: base + name_end as u32,
            args,
        });
        if i < expr.len() && expr[i] == b'.' {
            i += 1;
            continue;
        }
        break;
    }
    Some(segments)
}

/// The offset of the first top-level `|` (format suffix), outside quotes and
/// parentheses.
fn top_level_pipe(text: &[u8]) -> Option<usize> {
    let mut depth = 0;
    let mut quote = false;
    for (i, &b) in text.iter().enumerate() {
        if quote {
            quote = b != b'\'';
            continue;
        }
        match b {
            b'\'' => quote = true,
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'|' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// One resolved chain segment for hover: its signature row (when known) and
/// the receiver type it was looked up on (empty for roots).
#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub row: Option<&'static DataFnRow>,
    /// The receiver type for member segments, `""` for chain roots.
    pub receiver: &'static str,
}

/// A typing error in a chain: the offending segment's span and message.
#[derive(Clone, Debug)]
pub struct DataFnError {
    pub start: u32,
    pub end: u32,
    pub msg: String,
}

/// Resolves a chain against the registry: per-segment info (for hover) plus
/// the first typing error, if any. Checking stops at an `[unregistered]` or
/// unknown return type — the tail resolves as unknown, never as an error.
pub fn resolve_chain(
    segments: &[Segment],
    registry: &DataFnRegistry,
) -> (Vec<Resolved>, Option<DataFnError>) {
    let mut resolved = Vec::with_capacity(segments.len());
    let mut cur: Option<&'static str> = None; // current receiver type
    for (i, seg) in segments.iter().enumerate() {
        if i == 0 {
            if let Some(row) = registry.global(&seg.name) {
                cur = Some(row.ret);
                resolved.push(Resolved {
                    row: Some(row),
                    receiver: "",
                });
            } else if registry.is_type(&seg.name) {
                // Datacontext access by type name — the next segment
                // resolves on the named type itself.
                resolved.push(Resolved {
                    row: None,
                    receiver: "",
                });
                cur = Some(stored_type_name(registry, &seg.name));
            } else {
                let err = DataFnError {
                    start: seg.name_start,
                    end: seg.name_end,
                    msg: format!("unknown datafunction \"{}\"", seg.name),
                };
                resolved.push(Resolved {
                    row: None,
                    receiver: "",
                });
                return (resolved, Some(err));
            }
            continue;
        }
        let Some(receiver) = cur else {
            // Unknown receiver — accept the tail silently.
            resolved.push(Resolved {
                row: None,
                receiver: "",
            });
            continue;
        };
        if let Some(row) = registry.member(receiver, &seg.name) {
            resolved.push(Resolved {
                row: Some(row),
                receiver,
            });
            cur = if registry.can_type_through(row.ret) {
                Some(row.ret)
            } else {
                None
            };
        } else if registry.can_type_through(receiver) {
            let err = DataFnError {
                start: seg.name_start,
                end: seg.name_end,
                msg: format!("\"{}\" is not a member of {receiver}", seg.name),
            };
            resolved.push(Resolved {
                row: None,
                receiver,
            });
            return (resolved, Some(err));
        } else {
            // Receiver type not registered — accept silently.
            resolved.push(Resolved {
                row: None,
                receiver: "",
            });
            cur = None;
        }
    }
    (resolved, None)
}

/// The `'static` copy of a registered type name (the set holds table strs).
fn stored_type_name(registry: &DataFnRegistry, name: &str) -> &'static str {
    registry
        .types
        .get(name)
        .copied()
        .expect("checked with is_type")
}

/// Validates every datafunction expression in a parsed `.gui` tree, returning
/// the typing errors (spans are absolute byte offsets).
pub fn validate_datafns(tree: &SyntaxTree, registry: &DataFnRegistry) -> Vec<DataFnError> {
    let mut errors = Vec::new();
    let src = tree.source();
    for span in datafn_spans(tree) {
        let text = &src[span.start as usize..span.end as usize];
        let Some(segments) = parse_chain(text, span.start) else {
            continue;
        };
        let (_, err) = resolve_chain(&segments, registry);
        if let Some(err) = err {
            errors.push(err);
        }
    }
    errors
}
