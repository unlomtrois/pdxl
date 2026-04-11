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
	c, _ := l.advance()

	var tag Tag
	switch {
	case c >= '0' && c <= '9':
		// If next char is non-digit identifier char (like _ or letter), treat as identifier
		// Otherwise, treat as number
		if !l.isAtEnd() && (isAlpha(byte(l.peek())) || l.peek() == '_') {
			l.pos--
			tag = l.lexIdentifier(startPos)
		} else {
			tag = l.lexNumber()
		}
	case (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_':
		tag = l.lexIdentifier(startPos)
	case c > 127:
		// non-ASCII Unicode: treat as identifier start
		tag = l.lexIdentifier(startPos)
	case c == '"':
		tag = l.lexString()

	// scope operators
	case c == '.':
		tag = dot
	case c == ':':
		tag = colon
	case c == '@':
		tag = at
	case c == '|':
		tag = pipe
	case c == '$':
		tag = dollar

	// special
	case c == '%':
		tag = percent

	// operators
	case c == '=':
		if l.match('=') {
			tag = equal_equal
		} else {
			tag = equal
		}
	case c == '>':
		if l.match('=') {
			tag = greater_equal
		} else {
			tag = greater_than
		}
	case c == '<':
		if l.match('=') {
			tag = less_equal
		} else {
			tag = less_than
		}
	case c == '!':
		if l.match('=') {
			tag = not_equal
		} else {
			tag = invalid
		}
	case c == '?':
		if l.match('=') {
			tag = question_equal
		} else {
			tag = invalid
		}

	// arithmetic operators
	case c == '+':
		tag = plus
	case c == '-':
		tag = minus
	case c == '*':
		tag = multiply
	case c == '/':
		tag = divide

	case c == '{':
		tag = l_brace
	case c == '}':
		tag = r_brace
	case c == '[':
		tag = l_bracket
	case c == ']':
		tag = r_bracket

	default:
		tag = invalid
	}

	return &Token{
		Tag:   tag,
		Start: startPos,
		End:   l.pos,
	}
}

// lexIdentifier scans an identifier or keyword
func (l *Lexer) lexIdentifier(start int) Tag {
	for !l.isAtEnd() && isIdentifierChar(l.peek()) {
		l.advance()
	}
	end := l.pos

	// Check if this might be a keyword (yes/no)
	isPotentialKeyword := false
	if start >= 0 && start < len(l.source) {
		switch l.source[start] {
		case 'y', 'n':
			if end-start <= 3 {
				isPotentialKeyword = true
			}
		}
	}

	if isPotentialKeyword {
		content := l.source[start:end]
		if bytes.Equal(content, []byte("yes")) || bytes.Equal(content, []byte("no")) {
			return literal_boolean
		}
	}

	return identifier
}

// lexNumber scans a number literal
func (l *Lexer) lexNumber() Tag {
	for isDigit(byte(l.peek())) {
		l.advance()
	}

	if isIdentifierChar(l.peek()) {
		for isAlpha(byte(l.peek())) {
			l.advance()
		}
		return identifier
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

// advance consumes and returns the current rune and its byte size
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
