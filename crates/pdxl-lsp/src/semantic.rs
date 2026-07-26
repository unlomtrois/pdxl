//! Semantic tokens (`textDocument/semanticTokens/full`) — schema-aware syntax
//! highlighting driven by pdxl's own toolchain instead of a tree-sitter
//! grammar.
//!
//! Phase 1 colors from the lexer token stream (keys vs values, literals,
//! comments, macro params, operators). **Phase 2** layers meaning that only
//! pdxl knows:
//! - **builtin keys** — a key that names a documented effect/trigger
//!   (`add_trait`, `is_adult`) → `function` + `defaultLibrary`. Pure static
//!   lookup, no project needed.
//! - **scope prefixes** — an identifier before `:` (`scope`, `title`, …) →
//!   `namespace`. Pure, no project.
//! - **resolved references** — value ranges the analyzer resolved to a defined
//!   symbol → `type`. These come from `FileFacts.refs` (caller passes the
//!   ranges that resolve), so this part needs the project; unresolved refs are
//!   left as plain values (they already carry a diagnostic).
//!
//! [`TOKEN_TYPES`] / [`TOKEN_MODIFIERS`] index order **is** the wire encoding —
//! keep them in sync with the classifier below and [`legend`].

use std::collections::HashSet;
use std::sync::OnceLock;

use lsp_types::{SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend};
use pdxl_analysis::Schema;
use pdxl_lexer::TokenKind as T;

use crate::position::offsets_to_positions;

// Legend indices — the value the client receives for each token.
const PROPERTY: u32 = 0;
const VARIABLE: u32 = 1;
const NUMBER: u32 = 2;
const STRING: u32 = 3;
const KEYWORD: u32 = 4;
const COMMENT: u32 = 5;
const MACRO: u32 = 6;
const OPERATOR: u32 = 7;
const FUNCTION: u32 = 8;
const TYPE: u32 = 9;
const NAMESPACE: u32 = 10;
const ENUM_MEMBER: u32 = 11;
const PARAMETER: u32 = 12;

/// Legend, in wire-index order (see the constants above).
pub const TOKEN_TYPES: [SemanticTokenType; 13] = [
    SemanticTokenType::PROPERTY,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::NUMBER,
    SemanticTokenType::STRING,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::COMMENT,
    SemanticTokenType::MACRO,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::TYPE,
    SemanticTokenType::NAMESPACE,
    SemanticTokenType::ENUM_MEMBER,
    SemanticTokenType::PARAMETER,
];

/// `defaultLibrary` marks documented builtins; `unresolved` lets clients render
/// broken smart-doc references as a faded variant of their normal key color.
pub const TOKEN_MODIFIERS: [SemanticTokenModifier; 2] = [
    SemanticTokenModifier::DEFAULT_LIBRARY,
    SemanticTokenModifier::new("unresolved"),
];
const MOD_DEFAULT_LIBRARY: u32 = 1 << 0;
const MOD_UNRESOLVED: u32 = 1 << 1;

/// The legend advertised in `ServerCapabilities`.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: TOKEN_MODIFIERS.to_vec(),
    }
}

/// Documented effect + trigger names — a key matching one is a builtin.
fn builtins() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        pdxl_game::tables::EFFECTS
            .iter()
            .map(|row| row.name)
            .chain(pdxl_game::tables::TRIGGERS.iter().map(|row| row.name))
            .collect()
    })
}

/// Operators that mark the identifier before them as a *key*.
fn is_key_operator(kind: T) -> bool {
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
}

/// Tokens colored as `@operator`.
fn is_operator(kind: T) -> bool {
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
            | T::Plus
            | T::Minus
            | T::Multiply
            | T::Divide
            | T::Dot
            | T::Colon
            | T::Pipe
            | T::Percent
            | T::At
            | T::Dollar
    )
}

/// Base legend index for a token from its kind alone, or `None` to leave it
/// uncovered (braces, brackets, invalid, EOF).
fn base_type(kind: T, is_key: bool) -> Option<u32> {
    Some(match kind {
        T::Identifier => {
            if is_key {
                PROPERTY
            } else {
                VARIABLE
            }
        }
        T::LiteralNumber | T::LiteralDate => NUMBER,
        T::LiteralString => STRING,
        T::LiteralBoolean => KEYWORD,
        T::MacroParam | T::ScriptValue | T::ScriptMath => MACRO,
        k if is_operator(k) => OPERATOR,
        _ => return None,
    })
}

/// True if `off` falls inside one of the sorted, non-overlapping resolved-ref
/// intervals (binary search on the greatest start ≤ `off`).
fn in_resolved(resolved: &[(u32, u32)], off: u32) -> bool {
    let i = resolved.partition_point(|&(start, _)| start <= off);
    i > 0 && resolved[i - 1].1 > off
}

/// Computes delta-encoded semantic tokens. `resolved` is the sorted list of
/// value byte-ranges the analyzer resolved to a defined symbol (empty when no
/// project is available yet — builtins and scope prefixes still work).
/// Emits semantic tokens for a single `#!` doc comment run `[start, end)`:
/// each `![Name]` is colored as a reference ([`TYPE`]); unresolved names carry
/// [`MOD_UNRESOLVED`] so clients can fade them. Everything else stays comment.
fn emit_doc_comment(
    src: &[u8],
    start: usize,
    end: usize,
    resolved: &[(u32, u32)],
    schema: Option<&Schema>,
    emit: &mut Vec<(u32, u32, u32, u32)>,
) {
    let mut seg = start; // start of the current pending comment segment
    let mut k = start;
    while k + 1 < end {
        if src[k] == b'!' && src[k + 1] == b'[' {
            let bracket = k + 2;
            if let Some(rel) = src[bracket..end].iter().position(|&b| b == b']') {
                let content_end = bracket + rel;
                // Color only the name; a `kind:` qualifier stays comment.
                let off = schema.map_or(0, |sc| {
                    crate::state::parse_doc_ref(&src[bracket..content_end], sc).1
                });
                let name_start = bracket + off;
                emit.push((seg as u32, name_start as u32, COMMENT, 0));
                if content_end > name_start {
                    let modifiers = if in_resolved(resolved, name_start as u32) {
                        0
                    } else {
                        MOD_UNRESOLVED
                    };
                    emit.push((name_start as u32, content_end as u32, TYPE, modifiers));
                }
                seg = content_end; // the `]` and beyond resume as comment
                k = content_end;
                continue;
            }
        }
        k += 1;
    }
    if seg < end {
        emit.push((seg as u32, end as u32, COMMENT, 0));
    }
}

pub fn tokens(src: &[u8], resolved: &[(u32, u32)], schema: Option<&Schema>) -> Vec<SemanticToken> {
    tokens_impl(src, resolved, schema, false)
}

/// Semantic tokens for interface scripts (`.gui`): the script classifier plus
/// a gui layer — dialect keywords (`types`/`type`/`template`/`block`/…),
/// template/type definition names and bases, and datafunction chain segments
/// resolved against the `DumpDataTypes` registry.
/// Semantic highlighting for Paradox localization `.yml` files.
pub fn tokens_yml(src: &[u8], resolved: &[(u32, u32)]) -> Vec<SemanticToken> {
    use pdxl_yml::TokenKind as Y;
    let functions: HashSet<&str> = pdxl_game::tables::DATA_FNS
        .iter()
        .map(|row| row.name)
        .collect();
    let emit: Vec<(u32, u32, u32, u32)> = pdxl_yml::tokens(src)
        .into_iter()
        .map(|token| {
            let (ty, mods) = match token.kind {
                Y::Header => (NAMESPACE, 0),
                Y::Key => (PROPERTY, 0),
                Y::Comment => (COMMENT, 0),
                Y::Text => (STRING, 0),
                Y::LocReference => (TYPE, 0),
                Y::RuntimeParameter => (PARAMETER, 0),
                Y::Format => (KEYWORD, 0),
                Y::Icon => (MACRO, 0),
                Y::FunctionCandidate => {
                    let name = std::str::from_utf8(&src[token.start as usize..token.end as usize])
                        .unwrap_or("");
                    if functions.contains(name) {
                        (FUNCTION, MOD_DEFAULT_LIBRARY)
                    } else {
                        (VARIABLE, 0)
                    }
                }
            };
            (token.start, token.end, ty, mods)
        })
        .collect();
    // Entity keys are closed, schema-selected names rather than variables.
    // `enumMember` also gives themes a distinct class from both surrounding
    // prose and the datafunction itself. `$loc_key$` remains `type`.
    let overrides = resolved
        .iter()
        .map(|&(start, end)| (start, end, ENUM_MEMBER, 0))
        .collect();
    let mut emit = apply_overrides(emit, overrides);
    emit.sort_by_key(|&(start, ..)| start);
    encode(src, &emit)
}

pub fn tokens_gui(src: &[u8], resolved: &[(u32, u32)]) -> Vec<SemanticToken> {
    tokens_impl(src, resolved, None, true)
}

/// The gui dialect's structural keywords when standing alone (not keys).
fn is_gui_keyword(text: &[u8]) -> bool {
    matches!(
        text,
        b"types" | b"type" | b"template" | b"local_template" | b"block" | b"blockoverride"
    )
}

/// Collects gui-layer override ranges: keywords, definition names/bases, and
/// datafunction segments. Overrides win over the base classification.
fn gui_overrides(src: &[u8], lexed: &[pdxl_lexer::Token]) -> Vec<(u32, u32, u32, u32)> {
    let mut out: Vec<(u32, u32, u32, u32)> = Vec::new();
    let text_of = |t: &pdxl_lexer::Token| &src[t.range.start as usize..t.range.end as usize];

    // Token-stream pass: keywords and the def-name/base shapes around them.
    let toks: Vec<&pdxl_lexer::Token> = lexed.iter().filter(|t| t.kind != T::Comment).collect();
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i];
        if t.kind == T::Identifier && is_gui_keyword(text_of(t)) {
            // A key position (`type = X`) is a property, not a keyword.
            let next_is_op = toks.get(i + 1).is_some_and(|n| is_key_operator(n.kind));
            if !next_is_op {
                out.push((t.range.start, t.range.end, KEYWORD, 0));
                match text_of(t) {
                    // `types NAME {` / `template NAME {` — NAME is a type decl.
                    b"types" | b"template" | b"local_template" => {
                        if let Some(name) = toks.get(i + 1)
                            && name.kind == T::Identifier
                            && toks.get(i + 2).is_some_and(|b| b.kind == T::LBrace)
                        {
                            out.push((name.range.start, name.range.end, TYPE, 0));
                        }
                    }
                    // `type NAME = BASE {` — both NAME and BASE are types.
                    b"type" => {
                        if let Some(name) = toks.get(i + 1)
                            && name.kind == T::Identifier
                            && toks.get(i + 2).is_some_and(|o| o.kind == T::Equal)
                            && let Some(base) = toks.get(i + 3)
                            && base.kind == T::Identifier
                            && toks.get(i + 4).is_some_and(|b| b.kind == T::LBrace)
                        {
                            out.push((name.range.start, name.range.end, TYPE, 0));
                            out.push((base.range.start, base.range.end, TYPE, 0));
                            i += 4;
                            continue;
                        }
                    }
                    _ => {}
                }
            }
        }
        i += 1;
    }

    // Datafunction pass: chain segments (bare and embedded-in-string forms),
    // resolved against the DumpDataTypes registry.
    let parsed = pdxl_gui::parse(String::new(), src.to_vec());
    let registry = pdxl_game::datafn_registry();
    for span in pdxl_gui::datafn::datafn_spans(parsed.tree()) {
        let text = &src[span.start as usize..span.end as usize];
        let Some(segments) = pdxl_gui::datafn::parse_chain(text, span.start) else {
            continue;
        };
        let (infos, _err) = pdxl_gui::datafn::resolve_chain(&segments, registry);
        for (seg, info) in segments.iter().zip(infos.iter()) {
            let ty = if info.row.is_some() {
                FUNCTION
            } else if registry.is_type(&seg.name) {
                TYPE
            } else {
                continue; // unresolved tail — leave the base coloring
            };
            out.push((seg.name_start, seg.name_end, ty, MOD_DEFAULT_LIBRARY));
        }
    }

    out.sort_by_key(|&(start, ..)| start);
    out.dedup_by_key(|&mut (start, ..)| start);
    out
}

/// Splices sorted, non-overlapping `overrides` into `emit`: any base emission
/// overlapping an override is split around it, and the overrides are added.
fn apply_overrides(
    emit: Vec<(u32, u32, u32, u32)>,
    overrides: Vec<(u32, u32, u32, u32)>,
) -> Vec<(u32, u32, u32, u32)> {
    if overrides.is_empty() {
        return emit;
    }
    let mut out = Vec::with_capacity(emit.len() + overrides.len());
    for (start, end, ty, mods) in emit {
        let mut cursor = start;
        // Overrides intersecting [start, end).
        let from = overrides.partition_point(|&(_, oe, ..)| oe <= start);
        for &(os, oe, ..) in &overrides[from..] {
            if os >= end {
                break;
            }
            if os > cursor {
                out.push((cursor, os, ty, mods));
            }
            cursor = cursor.max(oe);
        }
        if cursor < end {
            out.push((cursor, end, ty, mods));
        }
    }
    out.extend(overrides);
    out
}

fn tokens_impl(
    src: &[u8],
    resolved: &[(u32, u32)],
    schema: Option<&Schema>,
    gui: bool,
) -> Vec<SemanticToken> {
    // Comments are real tokens, so one pass yields both the script tokens and
    // the comment runs. The classification loop below and the gui override
    // pass both want the comment-free view.
    let all = pdxl_lexer::tokenize_all(src);
    let lexed: Vec<pdxl_lexer::Token> = all
        .iter()
        .copied()
        .filter(|t| !t.kind.is_comment())
        .collect();

    // (start, end, type, modifiers)
    let mut emit: Vec<(u32, u32, u32, u32)> = Vec::new();
    for (i, tok) in lexed.iter().enumerate() {
        let next = lexed[i + 1..].iter().find(|t| t.kind != T::Comment);
        let is_key = tok.kind == T::Identifier && next.is_some_and(|t| is_key_operator(t.kind));
        let is_scope_prefix = tok.kind == T::Identifier && next.is_some_and(|t| t.kind == T::Colon);

        let Some(base) = base_type(tok.kind, is_key) else {
            continue;
        };

        let (ty, mods) = if in_resolved(resolved, tok.range.start) {
            // A resolved reference value (whole dotted id colors uniformly).
            (TYPE, 0)
        } else if is_scope_prefix {
            (NAMESPACE, 0)
        } else if is_key {
            let text = std::str::from_utf8(&src[tok.range.start as usize..tok.range.end as usize])
                .unwrap_or_default();
            if builtins().contains(text) {
                (FUNCTION, MOD_DEFAULT_LIBRARY)
            } else {
                (PROPERTY, 0)
            }
        } else {
            (base, 0)
        };
        emit.push((tok.range.start, tok.range.end, ty, mods));
    }

    // Comment runs come straight from the token stream. A `#!` doc comment
    // colors each `![Name]`'s name like a reference; the rest stays comment.
    for tok in all.iter().filter(|t| t.kind.is_comment()) {
        let (i, j) = (tok.range.start as usize, tok.range.end as usize);
        if tok.kind == T::DocComment {
            emit_doc_comment(src, i, j, resolved, schema, &mut emit);
        } else {
            emit.push((tok.range.start, tok.range.end, COMMENT, 0));
        }
    }

    // The gui layer overrides base classifications where it knows better.
    if gui {
        emit.sort_by_key(|&(start, ..)| start);
        emit = apply_overrides(emit, gui_overrides(src, &lexed));
    }

    // Document order; the wire format is strictly increasing by position.
    emit.sort_by_key(|&(start, ..)| start);

    // Batch offset→position (UTF-16), then delta-encode.
    let offsets: Vec<u32> = emit.iter().flat_map(|&(s, e, ..)| [s, e]).collect();
    let positions = offsets_to_positions(src, &offsets);

    let mut data = Vec::with_capacity(emit.len());
    let (mut prev_line, mut prev_start) = (0u32, 0u32);
    for (i, &(_, _, ty, mods)) in emit.iter().enumerate() {
        let start = positions[2 * i];
        let end = positions[2 * i + 1];
        if end.line != start.line || end.character < start.character {
            continue; // PDXScript tokens are single-line; skip the pathological case
        }
        let length = end.character - start.character;
        let delta_line = start.line - prev_line;
        let delta_start = if delta_line == 0 {
            start.character - prev_start
        } else {
            start.character
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type: ty,
            token_modifiers_bitset: mods,
        });
        prev_line = start.line;
        prev_start = start.character;
    }
    data
}

fn encode(src: &[u8], emit: &[(u32, u32, u32, u32)]) -> Vec<SemanticToken> {
    let offsets: Vec<u32> = emit.iter().flat_map(|&(s, e, ..)| [s, e]).collect();
    let positions = offsets_to_positions(src, &offsets);
    let mut data = Vec::with_capacity(emit.len());
    let (mut prev_line, mut prev_start) = (0, 0);
    for (i, &(_, _, token_type, token_modifiers_bitset)) in emit.iter().enumerate() {
        let start = positions[2 * i];
        let end = positions[2 * i + 1];
        if start.line != end.line || start == end {
            continue;
        }
        let delta_line = start.line - prev_line;
        let delta_start = if delta_line == 0 {
            start.character - prev_start
        } else {
            start.character
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: end.character - start.character,
            token_type,
            token_modifiers_bitset,
        });
        prev_line = start.line;
        prev_start = start.character;
    }
    data
}
