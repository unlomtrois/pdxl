//! Recursive-descent + Pratt parser, a direct port of `internal/parser/v3`.
//!
//! The algorithm, node-allocation order, and child-index ordering match the Go
//! parser exactly — these are differential-parity targets, not just "equivalent
//! trees". The parser is error-tolerant: it accumulates diagnostics and always
//! returns a (possibly partial) tree.

use std::sync::Arc;

use pdxl_lexer::{Token, TokenKind, tokenize};
use pdxl_src::TextRange;

use crate::diagnostic::{Diagnostic, Parse, Severity};
use pdxl_ast::{Node, NodeId, NodeKind, SyntaxTree};

/// Parses `source`, returning a tree and diagnostics.
///
/// The tree is always produced; check [`Parse::diagnostics`] rather than trusting
/// the tree blindly. `filename` is shared into each diagnostic.
pub fn parse(filename: impl Into<Arc<str>>, source: impl Into<Arc<[u8]>>) -> Parse {
    parse_inner(filename.into(), source.into(), false)
}

/// Parses `source` as an interface script (`.gui`). Identical to [`parse`]
/// except that an unquoted datafunction — `enabled = [ArmyWindow.CanMerge]` —
/// is accepted as a scalar value covering the whole `[…]` text. The script
/// grammar is untouched (it is a Go-parity target); `.gui`'s other extras
/// (`template NAME { }`, `type x = base { }`, `block "n" { }`) already parse
/// as sibling scalars + tagged blocks, mirroring the typed-definition shape.
pub fn parse_gui(filename: impl Into<Arc<str>>, source: impl Into<Arc<[u8]>>) -> Parse {
    parse_inner(filename.into(), source.into(), true)
}

fn parse_inner(filename: Arc<str>, source: Arc<[u8]>, gui: bool) -> Parse {
    let mut p = Parser::new(filename, &source);
    p.gui = gui;
    p.parse_file();
    let tree = SyntaxTree::from_parts(
        source,
        p.nodes.into_boxed_slice(),
        p.index.into_boxed_slice(),
    );
    Parse {
        tree,
        diagnostics: p.diags,
    }
}

struct Parser {
    tokens: Vec<Token>,
    filename: Arc<str>,
    pos: usize,
    nodes: Vec<Node>,
    index: Vec<NodeId>,
    diags: Vec<Diagnostic>,
    /// Interface-script dialect: accept `[Datafunction.Chain]` values.
    gui: bool,
}

/// A blank node template; fields are overwritten by each `alloc_node` caller.
const fn blank(kind: NodeKind) -> Node {
    Node {
        kind,
        range: TextRange::new(0, 0),
        operator: TokenKind::Invalid,
        child_start: 0,
        child_end: 0,
    }
}

impl Parser {
    fn new(filename: Arc<str>, src: &[u8]) -> Self {
        let tokens = tokenize(src);
        let cap = tokens.len() / 2;
        Parser {
            tokens,
            filename,
            pos: 0,
            nodes: Vec::with_capacity(cap),
            index: Vec::with_capacity(cap),
            diags: Vec::new(),
            gui: false,
        }
    }

    /// Appends a node and returns its id.
    fn alloc_node(&mut self, node: Node) -> NodeId {
        let id = NodeId::new(self.nodes.len() as u32);
        self.nodes.push(node);
        id
    }

    /// The token kind at `pos + offset`, or [`TokenKind::Eof`] past the end.
    fn peek(&self, offset: usize) -> TokenKind {
        match self.tokens.get(self.pos + offset) {
            Some(t) => t.kind,
            None => TokenKind::Eof,
        }
    }

    /// Advances one token. (Return value is unused by callers that only need the
    /// side effect; callers that need the token read it before advancing.)
    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    /// The byte offset to attribute a diagnostic to at the current position.
    fn current_offset(&self) -> u32 {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].range.start
        } else if let Some(last) = self.tokens.last() {
            last.range.end
        } else {
            0
        }
    }

    fn add_diag(&mut self, offset: u32, severity: Severity, message: String) {
        self.diags.push(Diagnostic {
            filename: Arc::clone(&self.filename),
            offset,
            message,
            severity,
        });
    }

    /// Skips tokens until a safe resumption point: a closing `}`, the start of a
    /// plausible new item, or EOF. Does NOT consume the token it stops at.
    fn synchronize(&mut self) {
        loop {
            match self.peek(0) {
                TokenKind::Eof | TokenKind::RBrace => return,
                t0 if is_atom(t0) => {
                    let t1 = self.peek(1);
                    if is_operator(t1) || t1 == TokenKind::LBrace {
                        return;
                    }
                    self.advance();
                }
                _ => self.advance(),
            }
        }
    }

    /// Looks past a key (skipping `:`/`.` chain pairs) to see if an operator
    /// follows — i.e. whether this item is a field.
    fn peek_op_after_key(&self) -> bool {
        let mut i = 1;
        loop {
            let tag = self.peek(i);
            if tag == TokenKind::Colon || tag == TokenKind::Dot {
                i += 2;
                continue;
            }
            return is_operator(tag);
        }
    }

    // ── Parsing ─────────────────────────────────────────────────────────────

    /// Parses top-level items and returns the root node id (always `0`).
    fn parse_file(&mut self) -> NodeId {
        let root = self.alloc_node(blank(NodeKind::File));
        let mut child_ids: Vec<NodeId> = Vec::new();

        while self.peek(0) != TokenKind::Eof {
            if let Some(id) = self.parse_item() {
                child_ids.push(id);
            }
        }

        let start = self.index.len() as u32;
        self.index.extend_from_slice(&child_ids);
        let end = self.index.len() as u32;
        let root_node = &mut self.nodes[root.index()];
        root_node.child_start = start;
        root_node.child_end = end;
        root
    }

    /// Parses one item, or `None` for a skipped token.
    fn parse_item(&mut self) -> Option<NodeId> {
        let t0 = self.peek(0);
        if t0 == TokenKind::Eof {
            return None;
        }
        if t0 == TokenKind::RBrace || t0 == TokenKind::RBracket {
            self.advance();
            return None;
        }
        if t0 != TokenKind::Minus && is_atom(t0) && self.peek_op_after_key() {
            return self.parse_field();
        }
        self.parse_value(0)
    }

    /// Parses a `key OP value` field, or `None` on error.
    fn parse_field(&mut self) -> Option<NodeId> {
        let key_start = self.tokens[self.pos].range.start;
        let mut key_end = self.tokens[self.pos].range.end;
        self.advance();
        while self.peek(0) == TokenKind::Colon || self.peek(0) == TokenKind::Dot {
            self.advance(); // connector
            if self.pos < self.tokens.len() {
                key_end = self.tokens[self.pos].range.end;
                self.advance();
            }
        }

        if !is_operator(self.peek(0)) {
            let off = self.current_offset();
            let got = self.peek(0);
            self.add_diag(
                off,
                Severity::Error,
                format!("expected operator, got {}", got.as_str()),
            );
            self.synchronize();
            return None;
        }
        let op_tok = self.tokens[self.pos];
        self.advance();

        let key_id = self.alloc_node(Node {
            kind: NodeKind::Scalar,
            range: TextRange::new(key_start, key_end),
            operator: TokenKind::Invalid,
            child_start: 0,
            child_end: 0,
        });

        // scripted_trigger/effect call sites pass a bare comparator as a macro
        // argument, e.g. `OPERATOR = <=`. After a single '=', accept a comparator
        // token as the value. (A non-'=' first operator stays a real error.)
        let val_id = if op_tok.kind == TokenKind::Equal && is_operator(self.peek(0)) {
            let cmp_tok = self.tokens[self.pos];
            self.advance();
            self.alloc_node(Node {
                kind: NodeKind::Scalar,
                range: cmp_tok.range,
                operator: TokenKind::Invalid,
                child_start: 0,
                child_end: 0,
            })
        } else {
            self.parse_value(0)?
        };

        let idx_start = self.index.len() as u32;
        self.index.push(key_id);
        self.index.push(val_id);

        Some(self.alloc_node(Node {
            kind: NodeKind::Field,
            range: TextRange::new(key_start, key_end),
            operator: op_tok.kind,
            child_start: idx_start,
            child_end: idx_start + 2,
        }))
    }

    /// Parses block items until `}` or EOF. `lbrace` is the opening brace token,
    /// used for the unclosed-block diagnostic offset.
    fn parse_block_items(&mut self, lbrace: Token) -> Vec<NodeId> {
        let mut items: Vec<NodeId> = Vec::new();
        while self.peek(0) != TokenKind::RBrace && self.peek(0) != TokenKind::Eof {
            if let Some(id) = self.parse_item() {
                items.push(id);
            }
        }
        if self.peek(0) == TokenKind::RBrace {
            self.advance();
        } else {
            // EOF without a closing brace. An inner unclosed block may have eaten
            // the '}' meant for this block, so the real mistake may be nested.
            self.add_diag(
                lbrace.range.start,
                Severity::Error,
                "unclosed block (missing '}'; an inner block may have stolen the closing brace)"
                    .to_string(),
            );
        }
        items
    }

    /// Parses a value, or `None` on error.
    fn parse_value(&mut self, min_bp: i32) -> Option<NodeId> {
        // Unary minus.
        if self.peek(0) == TokenKind::Minus {
            let start = self.tokens[self.pos].range.start;
            self.advance();
            if self.pos >= self.tokens.len() {
                let off = self.current_offset();
                self.add_diag(off, Severity::Error, "unexpected EOF after '-'".to_string());
                return None;
            }
            let mut end = self.tokens[self.pos].range.end;
            self.advance();
            while binding_power(self.peek(0)) > min_bp {
                self.advance(); // connector
                if self.pos < self.tokens.len() {
                    end = self.tokens[self.pos].range.end;
                    self.advance();
                }
            }
            return Some(self.alloc_node(Node {
                kind: NodeKind::Scalar,
                range: TextRange::new(start, end),
                operator: TokenKind::Invalid,
                child_start: 0,
                child_end: 0,
            }));
        }

        // Tagged block.
        if self.peek(0) == TokenKind::Identifier && self.peek(1) == TokenKind::LBrace {
            let tag_tok = self.tokens[self.pos];
            self.advance(); // tag
            let lbrace = self.tokens[self.pos];
            self.advance(); // consume '{'
            let items = self.parse_block_items(lbrace);
            let idx_start = self.index.len() as u32;
            self.index.extend_from_slice(&items);
            let idx_end = self.index.len() as u32;
            return Some(self.alloc_node(Node {
                kind: NodeKind::TaggedBlock,
                range: tag_tok.range,
                operator: TokenKind::Invalid,
                child_start: idx_start,
                child_end: idx_end,
            }));
        }

        // Plain block.
        if self.peek(0) == TokenKind::LBrace {
            let lbrace = self.tokens[self.pos];
            self.advance(); // consume '{'
            let items = self.parse_block_items(lbrace);
            let idx_start = self.index.len() as u32;
            self.index.extend_from_slice(&items);
            let idx_end = self.index.len() as u32;
            return Some(self.alloc_node(Node {
                kind: NodeKind::Block,
                range: TextRange::new(0, 0),
                operator: TokenKind::Invalid,
                child_start: idx_start,
                child_end: idx_end,
            }));
        }

        // Interface dialect: `[Datafunction.Chain( … )]` as one scalar value
        // covering the whole bracketed text. Brackets do not nest in the
        // datafunction language, so scan to the first `]`.
        if self.gui && self.peek(0) == TokenKind::LBracket {
            let start = self.tokens[self.pos].range.start;
            let mut end = self.tokens[self.pos].range.end;
            self.advance();
            while self.peek(0) != TokenKind::RBracket && self.peek(0) != TokenKind::Eof {
                end = self.tokens[self.pos].range.end;
                self.advance();
            }
            if self.peek(0) == TokenKind::RBracket {
                end = self.tokens[self.pos].range.end;
                self.advance();
            } else {
                self.add_diag(
                    start,
                    Severity::Error,
                    "unclosed datafunction (missing ']')".to_string(),
                );
            }
            return Some(self.alloc_node(Node {
                kind: NodeKind::Scalar,
                range: TextRange::new(start, end),
                operator: TokenKind::Invalid,
                child_start: 0,
                child_end: 0,
            }));
        }

        // Atom + optional scope-chain infix.
        if !is_atom(self.peek(0)) {
            let off = self.current_offset();
            let got = self.peek(0);
            self.add_diag(
                off,
                Severity::Error,
                format!("expected value, got {}", got.as_str()),
            );
            self.synchronize();
            return None;
        }
        let start = self.tokens[self.pos].range.start;
        let mut end = self.tokens[self.pos].range.end;
        self.advance();
        while binding_power(self.peek(0)) > min_bp {
            self.advance(); // connector
            if self.pos < self.tokens.len() {
                end = self.tokens[self.pos].range.end;
                self.advance();
            }
        }
        Some(self.alloc_node(Node {
            kind: NodeKind::Scalar,
            range: TextRange::new(start, end),
            operator: TokenKind::Invalid,
            child_start: 0,
            child_end: 0,
        }))
    }
}

/// Whether `tag` can begin an atom/value.
fn is_atom(tag: TokenKind) -> bool {
    matches!(
        tag,
        TokenKind::Identifier
            | TokenKind::LiteralNumber
            | TokenKind::LiteralString
            | TokenKind::LiteralBoolean
            | TokenKind::LiteralDate
            | TokenKind::Minus
            | TokenKind::MacroParam
            | TokenKind::ScriptValue
            | TokenKind::ScriptMath
    )
}

/// Whether `tag` is an assignment/comparison operator.
fn is_operator(tag: TokenKind) -> bool {
    matches!(
        tag,
        TokenKind::Equal
            | TokenKind::EqualEqual
            | TokenKind::NotEqual
            | TokenKind::QuestionEqual
            | TokenKind::GreaterThan
            | TokenKind::GreaterEqual
            | TokenKind::LessThan
            | TokenKind::LessEqual
    )
}

/// Infix binding power for scope-chain connectors (`.`, `:`, `|` → 80).
fn binding_power(tag: TokenKind) -> i32 {
    match tag {
        TokenKind::Colon | TokenKind::Dot | TokenKind::Pipe => 80,
        _ => 0,
    }
}
