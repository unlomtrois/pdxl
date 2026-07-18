//! Semantic tokens (`textDocument/semanticTokens/full`) — schema-aware syntax
//! highlighting driven by pdxl's own toolchain instead of a tree-sitter
//! grammar. This is **phase 1**: coloring from the lexer token stream alone
//! (keys vs values, numbers, strings, booleans, comments, macro params,
//! operators). A later phase will layer schema meaning (effects vs triggers,
//! scope prefixes, loc keys, resolved vs unresolved refs).
//!
//! The wire format is delta-encoded 5-tuples; [`legend`] declares the token
//! type table and [`TOKEN_TYPES`]' index order **is** the encoding, so the two
//! must stay in sync with [`type_index`].

use lsp_types::{SemanticToken, SemanticTokenType, SemanticTokensLegend};
use pdxl_lexer::TokenKind as T;

use crate::position::offsets_to_positions;

/// Legend, in wire-index order. Index `i` here is the `token_type` value the
/// client receives; [`type_index`] must map onto these same positions.
pub const TOKEN_TYPES: [SemanticTokenType; 8] = [
    SemanticTokenType::PROPERTY, // 0: keys (`add_trait =`)
    SemanticTokenType::VARIABLE, // 1: bare identifiers / values
    SemanticTokenType::NUMBER,   // 2: numbers and dates
    SemanticTokenType::STRING,   // 3
    SemanticTokenType::KEYWORD,  // 4: yes / no
    SemanticTokenType::COMMENT,  // 5
    SemanticTokenType::MACRO,    // 6: $PARAM$, @sv, @[ … ]
    SemanticTokenType::OPERATOR, // 7
];

/// The legend advertised in `ServerCapabilities`. No modifiers in phase 1.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: Vec::new(),
    }
}

/// Operators that mark the identifier before them as a *key* (`k = v`,
/// `k ?= v`, `days >= 5`).
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

/// Legend index for a token, or `None` to leave it uncovered (braces,
/// brackets, invalid, EOF — the theme's default text color). `is_key` means an
/// identifier immediately precedes a key operator.
fn type_index(kind: T, is_key: bool) -> Option<u32> {
    Some(match kind {
        T::Identifier => {
            if is_key {
                0
            } else {
                1
            }
        }
        T::LiteralNumber | T::LiteralDate => 2,
        T::LiteralString => 3,
        T::LiteralBoolean => 4,
        T::Comment => 5,
        T::MacroParam | T::ScriptValue | T::ScriptMath => 6,
        k if is_operator(k) => 7,
        _ => return None,
    })
}

/// Computes the delta-encoded semantic tokens for a buffer, from the lexer
/// token stream. Pure — needs no project, so it works before the async build
/// finishes and on files outside the mod tree.
pub fn tokens(src: &[u8]) -> Vec<SemanticToken> {
    let lexed = pdxl_lexer::tokenize(src);

    // Which emitted tokens, and their legend index.
    let mut emit: Vec<(u32, u32, u32)> = Vec::new(); // (start_off, end_off, type)
    for (i, tok) in lexed.iter().enumerate() {
        let is_key = tok.kind == T::Identifier
            && lexed[i + 1..]
                .iter()
                .find(|t| t.kind != T::Comment)
                .is_some_and(|t| is_key_operator(t.kind));
        if let Some(ty) = type_index(tok.kind, is_key) {
            emit.push((tok.range.start, tok.range.end, ty));
        }
    }

    // The lexer consumes comments as inter-token whitespace (they never appear
    // in the token stream), so recover them by scanning the gaps: a `#` inside
    // a gap always starts a comment (only whitespace and comments live there),
    // running to end of line. See the formatter's trivia scan for the same fact.
    let mut cursor = 0usize;
    let scan_gap = |from: usize, to: usize, emit: &mut Vec<(u32, u32, u32)>| {
        let mut i = from;
        while i < to {
            if src[i] == b'#' {
                let mut j = i;
                while j < to && src[j] != b'\n' {
                    j += 1;
                }
                emit.push((i as u32, j as u32, 5)); // comment
                i = j;
            } else {
                i += 1;
            }
        }
    };
    for tok in &lexed {
        scan_gap(cursor, tok.range.start as usize, &mut emit);
        cursor = cursor.max(tok.range.end as usize);
    }
    scan_gap(cursor, src.len(), &mut emit);

    // Sort into document order (comments were appended out of place); the wire
    // format is strictly increasing by position.
    emit.sort_by_key(|&(start, _, _)| start);

    // Batch offset→position (UTF-16) in one pass rather than per token.
    let offsets: Vec<u32> = emit.iter().flat_map(|(s, e, _)| [*s, *e]).collect();
    let positions = offsets_to_positions(src, &offsets);

    let mut data = Vec::with_capacity(emit.len());
    let (mut prev_line, mut prev_start) = (0u32, 0u32);
    for (i, &(_, _, ty)) in emit.iter().enumerate() {
        let start = positions[2 * i];
        let end = positions[2 * i + 1];
        // Tokens are single-line in PDXScript; skip the pathological case
        // rather than emit a corrupt length.
        if end.line != start.line || end.character < start.character {
            continue;
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
            token_modifiers_bitset: 0,
        });
        prev_line = start.line;
        prev_start = start.character;
    }
    data
}
