package lexer

import "bytes"

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
	c := l.advance()

	var tag Tag
	switch c {
	case '0', '1', '2', '3', '4', '5', '6', '7', '8', '9':
		// If next char is non-digit identifier char (like _ or letter), treat as identifier
		// Otherwise, treat as number
		if !l.isAtEnd() && (isAlpha(l.peek()) || l.peek() == '_') {
			l.pos--
			tag = l.lexIdentifier()
		} else {
			tag = l.lexNumber()
		}
	case 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
		'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
		'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
		'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
		'_':
		tag = l.lexIdentifier()
	case '"':
		tag = l.lexString()

	// scope operators
	case '.':
		tag = dot
	case ':':
		tag = colon
	case '@':
		tag = at
	case '|':
		tag = pipe
	case '$':
		tag = dollar

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

	//
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

	return &Token{
		Tag: tag,
		Start: startPos,
		End:   l.pos,
	}
}

// lexIdentifier scans an identifier or keyword
func (l *Lexer) lexIdentifier() Tag {
	start := l.pos - 1

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
	for isDigit(l.peek()) {
		l.advance()
	}

	if isIdentifierChar(l.peek()) {
		for isAlpha(l.peek()) {
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

// isIdentifierChar reports whether a character can be part of an identifier
func isIdentifierChar(c byte) bool {
	if isAlphaNumeric(c) {
		return true
	}
	switch c {
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

// advance consumes and returns the current character
func (l *Lexer) advance() byte {
	if l.isAtEnd() {
		return 0
	}
	c := l.source[l.pos]
	l.pos++
	return c
}

// peek returns the current character without consuming it
func (l *Lexer) peek() byte {
	if l.isAtEnd() {
		return 0
	}
	return l.source[l.pos]
}

// match consumes the current character if it matches expected
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