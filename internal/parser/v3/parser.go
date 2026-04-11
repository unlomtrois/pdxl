// Package parser implements a flat-node-pool PDXScript parser (Option A).
//
// All nodes live in a single []Node slice — no heap pointers inside Node.
// Parent-child relationships are expressed via an index array: each node
// stores a range [ChildStart, ChildEnd) into Tree.Index, and Tree.Index[i]
// is the index of the i-th child in Tree.Nodes.
//
// String values are byte-offset ranges into the original source slice,
// so no string copies are made during parsing.
//
// This is the fastest variant. For the pointer-based tree see internal/parser/v2.
// For the participle-based reference see internal/parser/v1.
package v3

import (
	"fmt"

	"pdxl/internal/lexer"
)

// ── Node kinds ────────────────────────────────────────────────────────────────

type NodeKind uint8

const (
	KindFile        NodeKind = iota // root; children = top-level items
	KindField                       // children[0]=key scalar, children[1]=value; Op set
	KindBlock                       // children = items
	KindTaggedBlock                 // children = items; SrcStart..SrcEnd = tag text
	KindScalar                      // leaf; SrcStart..SrcEnd is the text
)

// ── Node ──────────────────────────────────────────────────────────────────────

// Node is a pointer-free AST node.
type Node struct {
	Kind NodeKind

	// Source byte range (for scalars, field keys, tagged-block tag names).
	SrcStart uint32
	SrcEnd   uint32

	// Op stores the operator tag for Field nodes.
	Op lexer.Tag

	// Child range into Tree.Index (not Tree.Nodes).
	ChildStart uint32
	ChildEnd   uint32
}

// Value returns the source text for this node.
func (n Node) Value(src []byte) string {
	return string(src[n.SrcStart:n.SrcEnd])
}

// OpString returns the operator as its source symbol (e.g. "=", "?=", ">=").
func (n Node) OpString() string {
	switch n.Op {
	case lexer.TagEqual:
		return "="
	case lexer.TagEqualEqual:
		return "=="
	case lexer.TagNotEqual:
		return "!="
	case lexer.TagQuestionEqual:
		return "?="
	case lexer.TagGreaterThan:
		return ">"
	case lexer.TagGreaterEqual:
		return ">="
	case lexer.TagLessThan:
		return "<"
	case lexer.TagLessEqual:
		return "<="
	}
	return n.Op.String()
}

// ── Tree ──────────────────────────────────────────────────────────────────────

// Tree is the result of parsing.
// Nodes[0] is always the KindFile root.
// Index provides child indirection: node.Children are Index[ChildStart:ChildEnd],
// each element being an index into Nodes.
type Tree struct {
	Nodes []Node
	Index []uint32 // child index array
	Src   []byte
}

// Root returns the file root node.
func (t *Tree) Root() Node { return t.Nodes[0] }

// Children returns the direct children of n.
func (t *Tree) Children(n Node) []Node {
	refs := t.Index[n.ChildStart:n.ChildEnd]
	out := make([]Node, len(refs))
	for i, idx := range refs {
		out[i] = t.Nodes[idx]
	}
	return out
}

// ── ParseError ────────────────────────────────────────────────────────────────

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

// ── parser ────────────────────────────────────────────────────────────────────

type parser struct {
	tokens   []lexer.Token
	src      []byte
	filename string
	pos      int    // current index into tokens
	nodes    []Node // flat node pool
	index    []uint32
}

func newParser(filename string, src []byte) *parser {
	l := lexer.Init(src)
	tokens := make([]lexer.Token, 0, len(src)/8)
	for {
		tok := l.Next()
		if tok == nil {
			break
		}
		if tok.Tag == lexer.TagInvalid || tok.Tag == lexer.TagEOF {
			continue
		}
		tokens = append(tokens, *tok)
	}
	cap := len(tokens) / 2
	return &parser{
		tokens:   tokens,
		src:      src,
		filename: filename,
		nodes:    make([]Node, 0, cap),
		index:    make([]uint32, 0, cap),
	}
}

// allocNode appends a node and returns its index.
func (p *parser) allocNode(n Node) uint32 {
	idx := uint32(len(p.nodes))
	p.nodes = append(p.nodes, n)
	return idx
}

// peek returns the tag at pos+offset.
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

func (p *parser) currentOffset() int {
	if p.pos < len(p.tokens) {
		return p.tokens[p.pos].Start
	}
	if len(p.tokens) > 0 {
		return p.tokens[len(p.tokens)-1].End
	}
	return 0
}

func (p *parser) errorf(format string, args ...any) *ParseError {
	return &ParseError{
		Filename: p.filename,
		Offset:   p.currentOffset(),
		Msg:      fmt.Sprintf(format, args...),
	}
}

func isAtom(tag lexer.Tag) bool {
	switch tag {
	case lexer.TagIdentifier, lexer.TagLiteralNumber, lexer.TagLiteralString,
		lexer.TagLiteralBoolean, lexer.TagMinus:
		return true
	}
	return false
}

func isOperator(tag lexer.Tag) bool {
	switch tag {
	case lexer.TagEqual, lexer.TagEqualEqual, lexer.TagNotEqual,
		lexer.TagQuestionEqual, lexer.TagGreaterThan, lexer.TagGreaterEqual,
		lexer.TagLessThan, lexer.TagLessEqual:
		return true
	}
	return false
}

func bindingPower(tag lexer.Tag) int {
	switch tag {
	case lexer.TagColon, lexer.TagDot, lexer.TagPipe:
		return 80
	}
	return 0
}

func (p *parser) peekOpAfterKey() bool {
	i := 1
	for {
		tag := p.peek(i)
		if tag == lexer.TagColon || tag == lexer.TagDot {
			i += 2
			continue
		}
		return isOperator(tag)
	}
}

// ── Parsing ───────────────────────────────────────────────────────────────────

// parseFile parses top-level items, returns root node index.
func (p *parser) parseFile() (uint32, error) {
	rootIdx := p.allocNode(Node{Kind: KindFile})
	var childIdxs []uint32

	for p.peek(0) != lexer.TagEOF {
		idx, err := p.parseItem()
		if err != nil {
			return 0, err
		}
		if idx != ^uint32(0) {
			childIdxs = append(childIdxs, idx)
		}
	}

	start := uint32(len(p.index))
	p.index = append(p.index, childIdxs...)
	p.nodes[rootIdx].ChildStart = start
	p.nodes[rootIdx].ChildEnd = uint32(len(p.index))
	return rootIdx, nil
}

// parseItem returns the index of the parsed node, or ^uint32(0) for skipped tokens.
func (p *parser) parseItem() (uint32, error) {
	t0 := p.peek(0)
	if t0 == lexer.TagEOF {
		return ^uint32(0), nil
	}
	if t0 == lexer.TagRBrace || t0 == lexer.TagRBracket {
		p.advance()
		return ^uint32(0), nil
	}
	if t0 != lexer.TagMinus && isAtom(t0) && p.peekOpAfterKey() {
		return p.parseField()
	}
	return p.parseValue(0)
}

// parseField returns the index of a KindField node.
func (p *parser) parseField() (uint32, error) {
	// Scan key span.
	keyStart := uint32(p.tokens[p.pos].Start)
	keyEnd := uint32(p.tokens[p.pos].End)
	p.advance()
	for p.peek(0) == lexer.TagColon || p.peek(0) == lexer.TagDot {
		p.advance() // connector
		if p.pos < len(p.tokens) {
			keyEnd = uint32(p.tokens[p.pos].End)
			p.advance()
		}
	}

	if !isOperator(p.peek(0)) {
		return 0, p.errorf("expected operator, got %s", p.peek(0))
	}
	opTok := p.advance()

	// Allocate key scalar.
	keyIdx := p.allocNode(Node{Kind: KindScalar, SrcStart: keyStart, SrcEnd: keyEnd})

	// Parse value.
	valIdx, err := p.parseValue(0)
	if err != nil {
		return 0, err
	}

	// Build field node with two children: key, value.
	idxStart := uint32(len(p.index))
	p.index = append(p.index, keyIdx, valIdx)

	fieldIdx := p.allocNode(Node{
		Kind:       KindField,
		SrcStart:   keyStart,
		SrcEnd:     keyEnd,
		Op:         opTok.Tag,
		ChildStart: idxStart,
		ChildEnd:   idxStart + 2,
	})
	return fieldIdx, nil
}

// parseBlockItems parses items until '}' or EOF, returns child index slice.
func (p *parser) parseBlockItems() ([]uint32, error) {
	var items []uint32
	for p.peek(0) != lexer.TagRBrace && p.peek(0) != lexer.TagEOF {
		idx, err := p.parseItem()
		if err != nil {
			return nil, err
		}
		if idx != ^uint32(0) {
			items = append(items, idx)
		}
	}
	if p.peek(0) == lexer.TagRBrace {
		p.advance()
	}
	return items, nil
}

// parseValue returns the index of a value node.
func (p *parser) parseValue(minBP int) (uint32, error) {
	// Unary minus.
	if p.peek(0) == lexer.TagMinus {
		start := uint32(p.tokens[p.pos].Start)
		p.advance()
		if p.pos >= len(p.tokens) {
			return 0, p.errorf("unexpected EOF after '-'")
		}
		end := uint32(p.tokens[p.pos].End)
		p.advance()
		for bindingPower(p.peek(0)) > minBP {
			p.advance()
			if p.pos < len(p.tokens) {
				end = uint32(p.tokens[p.pos].End)
				p.advance()
			}
		}
		idx := p.allocNode(Node{Kind: KindScalar, SrcStart: start, SrcEnd: end})
		return idx, nil
	}

	// Tagged block.
	if p.peek(0) == lexer.TagIdentifier && p.peek(1) == lexer.TagLBrace {
		tagTok := p.advance()
		p.advance() // consume '{'
		items, err := p.parseBlockItems()
		if err != nil {
			return 0, err
		}
		idxStart := uint32(len(p.index))
		p.index = append(p.index, items...)
		idx := p.allocNode(Node{
			Kind:       KindTaggedBlock,
			SrcStart:   uint32(tagTok.Start),
			SrcEnd:     uint32(tagTok.End),
			ChildStart: idxStart,
			ChildEnd:   uint32(len(p.index)),
		})
		return idx, nil
	}

	// Plain block.
	if p.peek(0) == lexer.TagLBrace {
		p.advance() // consume '{'
		items, err := p.parseBlockItems()
		if err != nil {
			return 0, err
		}
		idxStart := uint32(len(p.index))
		p.index = append(p.index, items...)
		idx := p.allocNode(Node{
			Kind:       KindBlock,
			ChildStart: idxStart,
			ChildEnd:   uint32(len(p.index)),
		})
		return idx, nil
	}

	// Atom + optional scope-chain infix.
	if !isAtom(p.peek(0)) {
		return 0, p.errorf("expected value, got %s", p.peek(0))
	}
	start := uint32(p.tokens[p.pos].Start)
	end := uint32(p.tokens[p.pos].End)
	p.advance()
	for bindingPower(p.peek(0)) > minBP {
		p.advance() // connector
		if p.pos < len(p.tokens) {
			end = uint32(p.tokens[p.pos].End)
			p.advance()
		}
	}
	idx := p.allocNode(Node{Kind: KindScalar, SrcStart: start, SrcEnd: end})
	return idx, nil
}

// ── Public API ────────────────────────────────────────────────────────────────

// Parse parses src and returns a Tree. filename is used only in error messages.
func Parse(filename string, src []byte) (*Tree, error) {
	p := newParser(filename, src)
	if _, err := p.parseFile(); err != nil {
		return nil, err
	}
	return &Tree{Nodes: p.nodes, Index: p.index, Src: src}, nil
}
