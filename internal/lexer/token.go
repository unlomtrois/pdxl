package lexer

import "fmt"

// Position represents the line and column position of a token in the source
type Position struct {
	Line   int
	Column int
}

// Token represents a lexical token
type Token struct {
	Start int // Start position in source
	End   int // End position in source
	Tag   Tag // Token type
}

// Tag represents the type of a token
type Tag uint8

const (
	// Identifier
	identifier Tag = iota

	// Literals
	literal_number  // 108
	literal_string  // "something.dds"
	literal_boolean // yes / no

	// Delimiters
	l_brace   // {
	r_brace   // }
	l_bracket // [
	r_bracket // ]

	// Arithmetic operators
	plus     // +
	minus    // -
	multiply // *
	divide   // /

	// Comparison operators
	greater_than  // >
	greater_equal // >=
	less_than     // <
	less_equal    // <=

	// Assignment operators
	equal // =

	// Equality operators
	equal_equal    // ==
	not_equal      // !=
	question_equal // ?=

	// Scope resolution operators
	dot    // .
	colon  // :
	at     // @
	pipe   // |
	dollar // $

	percent // %

	comment // # Something
	invalid
	eof
)

// GetValue returns the literal value of the token from the source
func (t Token) GetValue(source []byte) []byte {
	return source[t.Start:t.End]
}

// getPosition returns the line and column position of this token in the source
func (t Token) getPosition(source []byte) Position {
	var line = 1
	var column = 1

	// Iterate through source up to token start position
	for i := 0; i < t.Start; i++ {
		if source[i] == '\n' {
			line++
			column = 1
		} else {
			column++
		}
	}

	return Position{Line: line, Column: column}
}

// FormatPosition formats the token position as "path:line:column"
func (t Token) FormatPosition(path string, source []byte) string {
	pos := t.getPosition(source)
	return fmt.Sprintf("%s:%d:%d", path, pos.Line, pos.Column)
}
