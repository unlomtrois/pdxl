package lexer

import (
	"bytes"
	"unicode/utf8"
)

// UTF8_BOM_SEQUENCE represents the UTF-8 BOM bytes
const UTF8_BOM_SEQUENCE = "\xEF\xBB\xBF"

// Lexer represents a lexical analyzer
type Lexer struct {
	source []byte
	pos    int
}

// Init initializes a new Lexer with the given source
func Init(source []byte) *Lexer {
	// Handle UTF-8 BOM
	bomPresent := bytes.HasPrefix(source, []byte(UTF8_BOM_SEQUENCE))
	offset := 0
	if bomPresent {
		offset = len(UTF8_BOM_SEQUENCE)
	}
	return &Lexer{
		source: source,
		pos:    offset,
	}
}

// Next returns the next token from the source
func (l *Lexer) Next() *Token {
	l.skipWhitespace()
	if l.isAtEnd() {
		return nil
	}

	startPos := l.pos
	c, size := l.advance()

	var tag Tag
	switch {
	case c >= '0' && c <= '9':
		if !l.isAtEnd() && (isAlpha(byte(l.peek())) || l.peek() == '_') {
			// digit followed immediately by letter/underscore — treat whole thing as identifier
			l.pos -= size
			tag = l.lexIdentifier()
		} else {
			tag = l.lexNumber()
			// trailing identifier chars after number (e.g. 1abc) — treat as identifier
			if isIdentifierChar(l.peek()) {
				for isIdentifierChar(l.peek()) {
					l.advance()
				}
				tag = identifier
			}
		}
	case isIdentifierStart(c):
		tag = l.lexIdentifier()
		switch string(l.source[startPos:l.pos]) {
		case "yes", "no":
			tag = literal_boolean
		}
	default:
		switch c {
		case '"':
			tag = l.lexString()

		// scope operators
		case '.':
			tag = dot
		case ':':
			tag = colon
		case '@':
			// @name is a read-once script value reference/definition;
			// @[ expr ] is inline math. Both are single value atoms.
			// A bare '@' with neither form falls back to at.
			switch {
			case isIdentifierChar(l.peek()):
				for isIdentifierChar(l.peek()) {
					l.advance()
				}
				tag = script_value
			case l.peek() == '[':
				l.advance() // consume '['
				for !l.isAtEnd() && l.peek() != ']' {
					l.advance()
				}
				if !l.isAtEnd() {
					l.advance() // consume ']'
				}
				tag = script_math
			default:
				tag = at
			}
		case '|':
			tag = pipe
		case '$':
			nameStart := l.pos
			for !l.isAtEnd() && isIdentifierChar(rune(l.peek())) {
				l.advance()
			}
			if l.pos > nameStart && !l.isAtEnd() && l.peek() == '$' {
				l.advance() // consume closing $
				tag = macro_param
			} else {
				l.pos = nameStart // backtrack — bare dollar
				tag = dollar
			}

		// special
		case '%':
			tag = percent

		// operators
		case '=':
			if l.match('=') {
				tag = equal_equal
			} else {
				tag = equal
			}
		case '>':
			if l.match('=') {
				tag = greater_equal
			} else {
				tag = greater_than
			}
		case '<':
			if l.match('=') {
				tag = less_equal
			} else {
				tag = less_than
			}
		case '!':
			if l.match('=') {
				tag = not_equal
			} else {
				tag = invalid
			}
		case '?':
			if l.match('=') {
				tag = question_equal
			} else {
				tag = invalid
			}

		// arithmetic operators
		case '+':
			tag = plus
		case '-':
			tag = minus
		case '*':
			tag = multiply
		case '/':
			tag = divide

		case '{':
			tag = l_brace
		case '}':
			tag = r_brace
		case '[':
			tag = l_bracket
		case ']':
			tag = r_bracket

		default:
			tag = invalid
		}
	}

	return &Token{
		Tag:   tag,
		Start: startPos,
		End:   l.pos,
	}
}

// lexIdentifier scans an identifier
func (l *Lexer) lexIdentifier() Tag {
	for !l.isAtEnd() && isIdentifierChar(l.peek()) {
		l.advance()
	}
	return identifier
}

// lexNumber scans a number literal, including an optional decimal part (e.g. 0.1)
func (l *Lexer) lexNumber() Tag {
	for isDigit(byte(l.peek())) {
		l.advance()
	}

	if l.peek() == '.' && isDigit(byte(l.peekNext())) {
		l.advance() // consume '.'
		for isDigit(byte(l.peek())) {
			l.advance()
		}
	}

	return literal_number
}

// lexString scans a string literal
func (l *Lexer) lexString() Tag {
	for !l.isAtEnd() && l.peek() != '"' {
		l.advance()
	}
	if l.isAtEnd() {
		return invalid // unterminated string
	}
	l.advance() // consume closing quote

	return literal_string
}

// skipWhitespace skips whitespace and comments
func (l *Lexer) skipWhitespace() {
	for !l.isAtEnd() {
		c := l.peek()
		switch c {
		case ' ', '\t', '\r':
			l.advance()
		case '\n':
			l.advance()
		case '#':
			// Skip until end of line
			for !l.isAtEnd() && l.peek() != '\n' {
				l.advance()
			}
		default:
			return
		}
	}
}

// isIdentifierStart reports whether a rune can begin an identifier
func isIdentifierStart(r rune) bool {
	return r > 127 || (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || r == '_'
}

// isIdentifierChar reports whether a rune can be part of an identifier
func isIdentifierChar(r rune) bool {
	if r > 127 {
		return true // any non-ASCII Unicode codepoint is valid in an identifier
	}
	if isAlphaNumeric(byte(r)) {
		return true
	}
	switch r {
	case '_', '&', '\'':
		return true
	default:
		return false
	}
}

// isAtEnd reports whether we've reached the end of the source
func (l *Lexer) isAtEnd() bool {
	return l.pos >= len(l.source)
}

// advance consumes and returns the current rune and its byte size.
// Note: invalid UTF-8 bytes produce utf8.RuneError (U+FFFD) with size 1.
// Since RuneError > 127, it passes isIdentifierChar — no validation is performed.
func (l *Lexer) advance() (rune, int) {
	if l.isAtEnd() {
		return 0, 0
	}
	r, size := utf8.DecodeRune(l.source[l.pos:])
	l.pos += size
	return r, size
}

// peek returns the current rune without consuming it
func (l *Lexer) peek() rune {
	if l.isAtEnd() {
		return 0
	}
	r, _ := utf8.DecodeRune(l.source[l.pos:])
	return r
}

// peekNext returns the rune after the current one without consuming either
func (l *Lexer) peekNext() rune {
	if l.isAtEnd() {
		return 0
	}
	_, size := utf8.DecodeRune(l.source[l.pos:])
	next := l.pos + size
	if next >= len(l.source) {
		return 0
	}
	r, _ := utf8.DecodeRune(l.source[next:])
	return r
}

// match consumes the current byte if it matches expected ASCII character
func (l *Lexer) match(expected byte) bool {
	if l.isAtEnd() {
		return false
	}
	if l.source[l.pos] != expected {
		return false
	}
	l.pos++
	return true
}

// Helper functions
func isAlpha(c byte) bool {
	return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
}

func isNumeric(c byte) bool {
	return c >= '0' && c <= '9'
}

func isAlphaNumeric(c byte) bool {
	return isAlpha(c) || isNumeric(c)
}

func isDigit(c byte) bool {
	return c >= '0' && c <= '9'
}

// Tokenize returns all valid tokens from src, skipping TagInvalid and TagEOF.
func Tokenize(src []byte) []Token {
	l := Init(src)
	out := make([]Token, 0, len(src)/8)
	for {
		tok := l.Next()
		if tok == nil {
			break
		}
		if tok.Tag == TagInvalid || tok.Tag == TagEOF {
			continue
		}
		out = append(out, *tok)
	}
	return out
}
