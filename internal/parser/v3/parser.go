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
// The parser is error-tolerant: it accumulates diagnostics and produces a
// partial tree rather than stopping at the first error. Callers should check
// len(diags) > 0 rather than treating a non-nil tree as fully valid.
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

// ChildRefs returns the child indices of n into Tree.Nodes without allocating.
// Prefer this in hot paths; use Children for convenience.
func (t *Tree) ChildRefs(n Node) []uint32 {
	return t.Index[n.ChildStart:n.ChildEnd]
}

// Children returns the direct children of n as Node values.
func (t *Tree) Children(n Node) []Node {
	refs := t.ChildRefs(n)
	out := make([]Node, len(refs))
	for i, idx := range refs {
		out[i] = t.Nodes[idx]
	}
	return out
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

// Severity classifies how serious a diagnostic is.
type Severity uint8

const (
	SeverityError   Severity = iota
	SeverityWarning
)

// Diagnostic is a parse problem with a source location.
// Offset is a byte offset into the source; use lexer.Token{Start: d.Offset}.FormatPosition
// to convert to a line:column string.
type Diagnostic struct {
	Filename string
	Offset   int
	Msg      string
	Severity Severity
}

func (d Diagnostic) String() string {
	tok := lexer.Token{Start: d.Offset, End: d.Offset}
	return fmt.Sprintf("%s: %s", tok.FormatPosition(d.Filename, nil), d.Msg)
}

// ── parser ────────────────────────────────────────────────────────────────────

// invalidIdx is returned by parse functions when no node was produced.
const invalidIdx = ^uint32(0)

type parser struct {
	tokens   []lexer.Token
	src      []byte
	filename string
	pos      int    // current index into tokens
	nodes    []Node // flat node pool
	index    []uint32
	diags    []Diagnostic
}

func newParser(filename string, src []byte) *parser {
	tokens := lexer.Tokenize(src)
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

func (p *parser) addDiag(offset int, severity Severity, msg string) {
	p.diags = append(p.diags, Diagnostic{
		Filename: p.filename,
		Offset:   offset,
		Msg:      msg,
		Severity: severity,
	})
}

// synchronize skips tokens until a safe resumption point:
// a closing '}', the start of a plausible new item, or EOF.
// It does NOT consume the token it stops at.
func (p *parser) synchronize() {
	for {
		switch p.peek(0) {
		case lexer.TagEOF, lexer.TagRBrace:
			return
		}
		t0 := p.peek(0)
		if isAtom(t0) {
			t1 := p.peek(1)
			if isOperator(t1) || t1 == lexer.TagLBrace {
				return
			}
		}
		p.advance()
	}
}

func isAtom(tag lexer.Tag) bool {
	switch tag {
	case lexer.TagIdentifier, lexer.TagLiteralNumber, lexer.TagLiteralString,
		lexer.TagLiteralBoolean, lexer.TagMinus, lexer.TagMacroParam,
		lexer.TagScriptValue, lexer.TagScriptMath:
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

// parseFile parses top-level items and returns the root node index.
func (p *parser) parseFile() uint32 {
	rootIdx := p.allocNode(Node{Kind: KindFile})
	var childIdxs []uint32

	for p.peek(0) != lexer.TagEOF {
		idx := p.parseItem()
		if idx != invalidIdx {
			childIdxs = append(childIdxs, idx)
		}
	}

	start := uint32(len(p.index))
	p.index = append(p.index, childIdxs...)
	p.nodes[rootIdx].ChildStart = start
	p.nodes[rootIdx].ChildEnd = uint32(len(p.index))
	return rootIdx
}

// parseItem returns the index of the parsed node, or invalidIdx for skipped tokens.
func (p *parser) parseItem() uint32 {
	t0 := p.peek(0)
	if t0 == lexer.TagEOF {
		return invalidIdx
	}
	if t0 == lexer.TagRBrace || t0 == lexer.TagRBracket {
		p.advance()
		return invalidIdx
	}
	if t0 != lexer.TagMinus && isAtom(t0) && p.peekOpAfterKey() {
		return p.parseField()
	}
	return p.parseValue(0)
}

// parseField returns the index of a KindField node, or invalidIdx on error.
func (p *parser) parseField() uint32 {
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
		p.addDiag(p.currentOffset(), SeverityError,
			fmt.Sprintf("expected operator, got %s", p.peek(0)))
		p.synchronize()
		return invalidIdx
	}
	opTok := p.advance()

	keyIdx := p.allocNode(Node{Kind: KindScalar, SrcStart: keyStart, SrcEnd: keyEnd})

	valIdx := p.parseValue(0)
	if valIdx == invalidIdx {
		return invalidIdx
	}

	idxStart := uint32(len(p.index))
	p.index = append(p.index, keyIdx, valIdx)

	return p.allocNode(Node{
		Kind:       KindField,
		SrcStart:   keyStart,
		SrcEnd:     keyEnd,
		Op:         opTok.Tag,
		ChildStart: idxStart,
		ChildEnd:   idxStart + 2,
	})
}

// parseBlockItems parses items until '}' or EOF.
// lbrace is the opening '{' token, used for unclosed-block diagnostics.
func (p *parser) parseBlockItems(lbrace lexer.Token) []uint32 {
	var items []uint32
	for p.peek(0) != lexer.TagRBrace && p.peek(0) != lexer.TagEOF {
		idx := p.parseItem()
		if idx != invalidIdx {
			items = append(items, idx)
		}
	}
	if p.peek(0) == lexer.TagRBrace {
		p.advance()
	} else {
		// EOF reached without closing brace. Note: an inner unclosed block may
		// have consumed the closing '}' intended for this block, so the real
		// mistake may be inside rather than here.
		p.addDiag(int(lbrace.Start), SeverityError, "unclosed block (missing '}'; an inner block may have stolen the closing brace)")
	}
	return items
}

// parseValue returns the index of a value node, or invalidIdx on error.
func (p *parser) parseValue(minBP int) uint32 {
	// Unary minus.
	if p.peek(0) == lexer.TagMinus {
		start := uint32(p.tokens[p.pos].Start)
		p.advance()
		if p.pos >= len(p.tokens) {
			p.addDiag(p.currentOffset(), SeverityError, "unexpected EOF after '-'")
			return invalidIdx
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
		return p.allocNode(Node{Kind: KindScalar, SrcStart: start, SrcEnd: end})
	}

	// Tagged block.
	if p.peek(0) == lexer.TagIdentifier && p.peek(1) == lexer.TagLBrace {
		tagTok := p.advance()
		lbrace := p.advance() // consume '{'
		items := p.parseBlockItems(lbrace)
		idxStart := uint32(len(p.index))
		p.index = append(p.index, items...)
		return p.allocNode(Node{
			Kind:       KindTaggedBlock,
			SrcStart:   uint32(tagTok.Start),
			SrcEnd:     uint32(tagTok.End),
			ChildStart: idxStart,
			ChildEnd:   uint32(len(p.index)),
		})
	}

	// Plain block.
	if p.peek(0) == lexer.TagLBrace {
		lbrace := p.advance() // consume '{'
		items := p.parseBlockItems(lbrace)
		idxStart := uint32(len(p.index))
		p.index = append(p.index, items...)
		return p.allocNode(Node{
			Kind:       KindBlock,
			ChildStart: idxStart,
			ChildEnd:   uint32(len(p.index)),
		})
	}

	// Atom + optional scope-chain infix.
	if !isAtom(p.peek(0)) {
		p.addDiag(p.currentOffset(), SeverityError,
			fmt.Sprintf("expected value, got %s", p.peek(0)))
		p.synchronize()
		return invalidIdx
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
	return p.allocNode(Node{Kind: KindScalar, SrcStart: start, SrcEnd: end})
}

// ── Public API ────────────────────────────────────────────────────────────────

// Parse parses src and returns a Tree along with any diagnostics.
// The tree is always non-nil; a non-empty diagnostics slice means errors were
// found but parsing continued as far as possible.
func Parse(filename string, src []byte) (*Tree, []Diagnostic) {
	p := newParser(filename, src)
	p.parseFile()
	return &Tree{Nodes: p.nodes, Index: p.index, Src: src}, p.diags
}
