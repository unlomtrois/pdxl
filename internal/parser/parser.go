// Package parser implements a hand-written recursive descent + Pratt parser
// for PDXScript.
//
// PDXScript block contents are not uniform — a block may contain:
//   - fields:        key op value  (e.g. age > 18, name = "foo")
//   - value lists:   bare atoms    (e.g. { 255 255 255 }, { GEN GAZ })
//   - tagged blocks: tag { … }     (e.g. rgb { 218 215 56 })
//
// The grammar is not LL(1) at the block level, but 2-token lookahead resolves
// all ambiguities without backtracking:
//
//   - peek[0] is atom AND peek[1] is operator  → Field
//   - peek[0] is atom AND peek[1] is '{'        → TaggedBlock (inside Value)
//   - peek[0] is '{'                             → Block
//   - otherwise                                  → Scalar
//
// Pratt parsing handles compound RHS values (scope:path.chain, define:X|Y, -0.25).
//
// This is parser v2. The participle-based v1 lives in internal/parser/v1/ for
// benchmarking comparison.
package parser

import (
	"fmt"

	"pdxl/internal/lexer"
)

// ── AST types ─────────────────────────────────────────────────────────────────

// Value is the sealed interface for right-hand-side values.
type Value interface{ isValue() }

// TaggedBlock handles constructs like `rgb { 255 0 0 }`.
type TaggedBlock struct {
	Tag   string
	Items []*Item
}

func (*TaggedBlock) isValue() {}

// Block is a brace-delimited sequence of items (fields or bare scalars).
type Block struct {
	Items []*Item
}

func (*Block) isValue() {}

// Scalar is a single atom or compound token chain (scope:path.seg, define:X|Y, -0.25).
// The value is stored directly as a string slice to avoid re-scanning source.
type Scalar struct {
	Parts []string
}

// Value returns the scalar as a single concatenated string.
func (s *Scalar) Value() string {
	out := ""
	for _, p := range s.Parts {
		out += p
	}
	return out
}

func (*Scalar) isValue() {}

// Item is one entry inside a block or at the top level.
type Item struct {
	Field  *Field
	Scalar *Scalar // non-nil only when Field is nil
}

// Field is a key–operator–value triple.
type Field struct {
	KeyParts []string
	Operator string
	Value    Value
}

// Key returns the field key as a single concatenated string.
func (f *Field) Key() string {
	out := ""
	for _, p := range f.KeyParts {
		out += p
	}
	return out
}

// File is the root of a parsed PDXScript file.
type File struct {
	Items []*Item
}

// ── Parser ────────────────────────────────────────────────────────────────────

// ParseError records a parse failure with its byte offset.
type ParseError struct {
	Filename string
	Offset   int
	Msg      string
}

func (e *ParseError) Error() string {
	if e.Filename != "" {
		return fmt.Sprintf("%s: offset %d: %s", e.Filename, e.Offset, e.Msg)
	}
	return fmt.Sprintf("offset %d: %s", e.Offset, e.Msg)
}

// parser holds the pre-lexed token stream and parser state.
type parser struct {
	tokens   []lexer.Token
	src      []byte
	filename string
	pos      int // index into tokens
}

func newParser(filename string, src []byte) *parser {
	l := lexer.Init(src)
	var tokens []lexer.Token
	for {
		tok := l.Next()
		if tok == nil {
			break
		}
		// skip comments and invalid tokens
		if tok.Tag == lexer.TagInvalid || tok.Tag == lexer.TagEOF {
			continue
		}
		tokens = append(tokens, *tok)
	}
	return &parser{tokens: tokens, src: src, filename: filename}
}

// peek returns the tag of the token at pos+offset without consuming.
func (p *parser) peek(offset int) lexer.Tag {
	i := p.pos + offset
	if i >= len(p.tokens) {
		return lexer.TagEOF
	}
	return p.tokens[i].Tag
}

// advance consumes and returns the current token.
func (p *parser) advance() lexer.Token {
	if p.pos >= len(p.tokens) {
		return lexer.Token{Tag: lexer.TagEOF}
	}
	tok := p.tokens[p.pos]
	p.pos++
	return tok
}

// tokenStr returns the source string for a token.
func (p *parser) tokenStr(t lexer.Token) string {
	return string(t.GetValue(p.src))
}

// currentOffset returns the source byte offset for error reporting.
func (p *parser) currentOffset() int {
	if p.pos < len(p.tokens) {
		return p.tokens[p.pos].Start
	}
	if len(p.tokens) > 0 {
		return p.tokens[len(p.tokens)-1].End
	}
	return 0
}

// errorf creates a ParseError at the current position.
func (p *parser) errorf(format string, args ...any) *ParseError {
	return &ParseError{
		Filename: p.filename,
		Offset:   p.currentOffset(),
		Msg:      fmt.Sprintf(format, args...),
	}
}

// isAtom reports whether the tag is a value atom (not an operator or brace).
func isAtom(tag lexer.Tag) bool {
	switch tag {
	case lexer.TagIdentifier, lexer.TagLiteralNumber, lexer.TagLiteralString,
		lexer.TagLiteralBoolean, lexer.TagMinus:
		return true
	}
	return false
}

// peekOpAfterKey looks ahead past any leading scope-chain tokens (: .) to
// determine whether this item is a Field (has an operator) or a bare Scalar.
// Field key example: scope:actor ?= { ... }  → tokens: ident : ident ?= ...
func (p *parser) peekOpAfterKey() bool {
	i := 1
	for {
		tag := p.peek(i)
		if tag == lexer.TagColon || tag == lexer.TagDot {
			i++ // skip connector
			i++ // skip segment
			continue
		}
		return isOperator(tag)
	}
}

// isOperator reports whether the tag is a field assignment/comparison operator.
func isOperator(tag lexer.Tag) bool {
	switch tag {
	case lexer.TagEqual, lexer.TagEqualEqual, lexer.TagNotEqual,
		lexer.TagQuestionEqual, lexer.TagGreaterThan, lexer.TagGreaterEqual,
		lexer.TagLessThan, lexer.TagLessEqual:
		return true
	}
	return false
}

// ── Pratt binding powers ──────────────────────────────────────────────────────

// bindingPower returns the infix binding power for scope-chain connectors.
func bindingPower(tag lexer.Tag) int {
	switch tag {
	case lexer.TagColon, lexer.TagDot, lexer.TagPipe:
		return 80
	}
	return 0
}

// ── Recursive descent ─────────────────────────────────────────────────────────

// parseFile parses the top-level sequence of items.
func (p *parser) parseFile() (*File, error) {
	f := &File{}
	for p.peek(0) != lexer.TagEOF {
		item, err := p.parseItem()
		if err != nil {
			return nil, err
		}
		if item != nil {
			f.Items = append(f.Items, item)
		}
	}
	return f, nil
}

// parseItem parses one Field or bare Scalar.
// Returns nil, nil when nothing is available (should not happen inside parseFile).
func (p *parser) parseItem() (*Item, error) {
	t0 := p.peek(0)
	if t0 == lexer.TagEOF {
		return nil, nil
	}

	// A Field starts with an atom (possibly a scope chain) followed by an operator.
	// Minus at peek(0) is always a bare negative scalar on the LHS.
	// We scan past any leading scope-chain connectors (: .) to find the operator.
	if t0 != lexer.TagMinus && isAtom(t0) && p.peekOpAfterKey() {
		field, err := p.parseField()
		if err != nil {
			return nil, err
		}
		return &Item{Field: field}, nil
	}

	// Skip stray r_brace / r_bracket that might appear in malformed input.
	if t0 == lexer.TagRBrace || t0 == lexer.TagRBracket {
		p.advance()
		return nil, nil
	}

	scalar, err := p.parseValue(0)
	if err != nil {
		return nil, err
	}
	if s, ok := scalar.(*Scalar); ok {
		return &Item{Scalar: s}, nil
	}
	// block-valued bare item (unusual but valid in some mod files)
	return &Item{Scalar: &Scalar{Parts: []string{"{}"}}}, nil
}

// parseField parses  key op value.
func (p *parser) parseField() (*Field, error) {
	// key: one or more tokens connected by : or .
	keyTok := p.advance()
	keyParts := []string{p.tokenStr(keyTok)}
	for p.peek(0) == lexer.TagColon || p.peek(0) == lexer.TagDot {
		sep := p.advance()
		keyParts = append(keyParts, p.tokenStr(sep))
		seg := p.advance()
		keyParts = append(keyParts, p.tokenStr(seg))
	}

	if !isOperator(p.peek(0)) {
		return nil, p.errorf("expected operator, got %s", p.peek(0))
	}
	opTok := p.advance()
	op := p.tokenStr(opTok)

	val, err := p.parseValue(0)
	if err != nil {
		return nil, err
	}

	return &Field{KeyParts: keyParts, Operator: op, Value: val}, nil
}

// parseBlock parses `{ item* }`.
func (p *parser) parseBlock() (*Block, error) {
	p.advance() // consume '{'
	var items []*Item
	for p.peek(0) != lexer.TagRBrace && p.peek(0) != lexer.TagEOF {
		item, err := p.parseItem()
		if err != nil {
			return nil, err
		}
		if item != nil {
			items = append(items, item)
		}
	}
	if p.peek(0) == lexer.TagRBrace {
		p.advance() // consume '}'
	}
	return &Block{Items: items}, nil
}

// parseValue is the Pratt value parser.
// minBP is the minimum binding power for infix continuation (pass 0 at top level).
func (p *parser) parseValue(minBP int) (Value, error) {
	// prefix: unary minus → negative number/identifier
	if p.peek(0) == lexer.TagMinus {
		minus := p.advance()
		if p.peek(0) == lexer.TagEOF {
			return nil, p.errorf("unexpected EOF after '-'")
		}
		num := p.advance()
		parts := []string{p.tokenStr(minus), p.tokenStr(num)}
		// consume trailing scope-chain fragments after -0.25e3 etc. (rare but possible)
		for bindingPower(p.peek(0)) > minBP {
			sep := p.advance()
			parts = append(parts, p.tokenStr(sep))
			seg := p.advance()
			parts = append(parts, p.tokenStr(seg))
		}
		return &Scalar{Parts: parts}, nil
	}

	// prefix: tagged block — identifier followed directly by '{'
	if p.peek(0) == lexer.TagIdentifier && p.peek(1) == lexer.TagLBrace {
		tagTok := p.advance()
		tag := p.tokenStr(tagTok)
		block, err := p.parseBlock()
		if err != nil {
			return nil, err
		}
		return &TaggedBlock{Tag: tag, Items: block.Items}, nil
	}

	// prefix: plain block
	if p.peek(0) == lexer.TagLBrace {
		return p.parseBlock()
	}

	// atom
	if !isAtom(p.peek(0)) {
		return nil, p.errorf("expected value, got %s", p.peek(0))
	}
	tok := p.advance()
	parts := []string{p.tokenStr(tok)}

	// infix loop: extend scope chains (: . |)
	for bindingPower(p.peek(0)) > minBP {
		sep := p.advance()
		parts = append(parts, p.tokenStr(sep))
		if p.peek(0) == lexer.TagEOF {
			break
		}
		seg := p.advance()
		parts = append(parts, p.tokenStr(seg))
	}

	return &Scalar{Parts: parts}, nil
}

// ── Public API ────────────────────────────────────────────────────────────────

// ParseBytes parses a PDXScript source buffer.
// filename is used only for error messages; it is not stored in the AST.
func ParseBytes(filename string, src []byte) (*File, error) {
	p := newParser(filename, src)
	return p.parseFile()
}
