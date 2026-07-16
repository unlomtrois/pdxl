//! The line builder: renders the item stream in expand-every-block style.
//!
//! Every block containing a field, nested block, or comment expands — one
//! entry per line, tab-indented, `}` on its own line. Two deliberate
//! exceptions keep the output close to vanilla conventions: empty blocks
//! render as `{ }`, and scalar-only lists (`color = { 255 0 0 }`) stay
//! inline while they fit [`MAX_INLINE_WIDTH`].
//!
//! Token bytes are never rewritten; layout decisions ride on two facts the
//! trivia scan preserved: `glued` (zero source gap ⇒ no space, which keeps
//! `scope:root.var` and dotted event ids intact without knowing grammar) and
//! `nl_before` (≥2 newlines ⇒ one blank line kept).

use pdxl_lexer::TokenKind;

use crate::trivia::Item;

/// A rendered scalar-only list longer than this expands one item per line
/// (long vanilla name lists would otherwise join into enormous lines).
const MAX_INLINE_WIDTH: usize = 100;

pub(crate) struct Options {
    /// Keep field-free, comment-free blocks (`{ 255 0 0 }`) on one line.
    pub(crate) inline_scalar_lists: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            inline_scalar_lists: true,
        }
    }
}

fn is_operator(kind: TokenKind) -> bool {
    use TokenKind::*;
    matches!(
        kind,
        Equal
            | QuestionEqual
            | EqualEqual
            | NotEqual
            | GreaterThan
            | GreaterEqual
            | LessThan
            | LessEqual
    )
}

pub(crate) fn emit(items: &[Item<'_>], opts: &Options) -> String {
    let mut e = Emitter {
        out: String::new(),
        depth: 0,
        line_open: false,
        after_operator: false,
    };
    let mut i = 0;
    while i < items.len() {
        i = e.step(items, i, opts);
    }
    e.close_line();
    e.out
}

struct Emitter {
    out: String,
    depth: usize,
    line_open: bool,
    /// The last emitted token was an operator — the next token is its value
    /// and continues the line with the space the operator already provided.
    after_operator: bool,
}

impl Emitter {
    /// Processes `items[i]`, returns the index of the next unprocessed item.
    fn step(&mut self, items: &[Item<'_>], i: usize, opts: &Options) -> usize {
        let it = &items[i];
        let Some(kind) = it.token() else {
            self.comment(it);
            return i + 1;
        };
        match kind {
            TokenKind::LBrace => self.open_block(items, i, opts),
            TokenKind::RBrace => {
                self.close_line();
                self.depth = self.depth.saturating_sub(1);
                self.start_line();
                self.out.push('}');
                // The line stays open so a trailing `# comment` can attach.
                i + 1
            }
            k if is_operator(k) => {
                // A trailing comment between key and operator closes the
                // line (comments run to EOL); reopen rather than emit a
                // leading space on a bare line.
                if self.line_open {
                    self.push_str(" ");
                } else {
                    self.start_line();
                }
                self.push_text(it.text);
                self.push_str(" ");
                self.after_operator = true;
                i + 1
            }
            _ => {
                self.scalar(it);
                i + 1
            }
        }
    }

    /// A `{`: inline `{ }` when empty, inline `{ a b c }` for scalar-only
    /// lists that fit, otherwise expand (newline + indent level).
    fn open_block(&mut self, items: &[Item<'_>], i: usize, opts: &Options) -> usize {
        // Empty block (comments inside force expansion).
        if let Some(next) = items.get(i + 1)
            && next.token() == Some(TokenKind::RBrace)
        {
            self.brace_onto_line();
            self.out.push_str("{ }");
            return i + 2;
        }

        if opts.inline_scalar_lists
            && let Some(close) = scalar_only_end(items, i)
        {
            let rendered = render_inline_list(&items[i + 1..close]);
            let prefix = if self.line_open {
                line_len(&self.out, true)
            } else {
                self.depth // tabs the fresh line would start with
            };
            let width = prefix + rendered.len() + 4; // " { " + list + " }"
            if width <= MAX_INLINE_WIDTH {
                self.brace_onto_line();
                self.out.push_str("{ ");
                self.out.push_str(&rendered);
                self.out.push_str(" }");
                return close + 1;
            }
        }

        self.brace_onto_line();
        self.out.push('{');
        // A same-line comment belongs to the opener: `a = { # why`.
        let mut next = i + 1;
        if let Some(c) = items.get(next)
            && c.is_comment()
            && c.nl_before == 0
        {
            self.out.push(' ');
            self.push_text(c.text);
            next += 1;
        }
        self.close_line();
        self.depth += 1;
        next
    }

    /// Puts `{` on the current line (value/tag position) or on a fresh line
    /// (bare block used as a list item).
    fn brace_onto_line(&mut self) {
        if self.line_open {
            if !self.after_operator {
                self.out.push(' ');
            }
            self.after_operator = false;
        } else {
            self.start_line();
        }
    }

    fn scalar(&mut self, it: &Item<'_>) {
        if self.after_operator {
            self.push_text(it.text);
            self.after_operator = false;
            return;
        }
        if self.line_open {
            if it.glued {
                self.push_text(it.text);
                return;
            }
            // A non-glued scalar on an open line starts a new entry.
            self.close_line();
        }
        self.blank_line_if(it.nl_before);
        self.start_line();
        self.push_text(it.text);
    }

    fn comment(&mut self, it: &Item<'_>) {
        if self.line_open && it.nl_before == 0 {
            self.push_str(" ");
            self.push_text(it.text);
            self.close_line(); // nothing may follow a comment on its line
            return;
        }
        self.close_line();
        self.blank_line_if(it.nl_before);
        self.start_line();
        self.push_text(it.text);
        self.close_line();
    }

    fn blank_line_if(&mut self, nl_before: u32) {
        if nl_before >= 2 && !self.out.is_empty() {
            self.out.push('\n');
        }
    }

    fn start_line(&mut self) {
        debug_assert!(!self.line_open);
        for _ in 0..self.depth {
            self.out.push('\t');
        }
        self.line_open = true;
        self.after_operator = false;
    }

    fn close_line(&mut self) {
        if self.line_open {
            self.out.push('\n');
            self.line_open = false;
        }
        self.after_operator = false;
    }

    fn push_text(&mut self, text: &[u8]) {
        self.out.push_str(&String::from_utf8_lossy(text));
    }

    fn push_str(&mut self, s: &str) {
        self.out.push_str(s);
    }
}

/// If the block opened at `items[open]` contains only scalar tokens (no
/// operators, braces, or comments), returns the index of its closing brace.
fn scalar_only_end(items: &[Item<'_>], open: usize) -> Option<usize> {
    for (j, it) in items.iter().enumerate().skip(open + 1) {
        match it.token() {
            Some(TokenKind::RBrace) => return Some(j),
            Some(TokenKind::LBrace) | None => return None,
            Some(k) if is_operator(k) => return None,
            Some(_) => {}
        }
    }
    None
}

/// Renders list items space-separated, honoring glue (`scope:x` stays one
/// word).
fn render_inline_list(items: &[Item<'_>]) -> String {
    let mut s = String::new();
    for it in items {
        if !s.is_empty() && !it.glued {
            s.push(' ');
        }
        s.push_str(&String::from_utf8_lossy(it.text));
    }
    s
}

/// Length of the currently open line (0 when no line is open).
fn line_len(out: &str, line_open: bool) -> usize {
    if !line_open {
        return 0;
    }
    out.rfind('\n').map_or(out.len(), |p| out.len() - p - 1)
}
