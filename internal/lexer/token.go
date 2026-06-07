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
	literal_date    // 1099.1.1

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
	dollar       // $
	macro_param  // $IDENT$
	script_value // @name (read-once script value reference/definition)
	script_math  // @[ expr ] (inline math)

	percent // %

	comment // # Something
	invalid
	eof
)

// String returns the name of the tag for display purposes.
func (t Tag) String() string {
	switch t {
	case identifier:
		return "identifier"
	case literal_number:
		return "literal_number"
	case literal_string:
		return "literal_string"
	case literal_boolean:
		return "literal_boolean"
	case literal_date:
		return "literal_date"
	case l_brace:
		return "l_brace"
	case r_brace:
		return "r_brace"
	case l_bracket:
		return "l_bracket"
	case r_bracket:
		return "r_bracket"
	case plus:
		return "plus"
	case minus:
		return "minus"
	case multiply:
		return "multiply"
	case divide:
		return "divide"
	case greater_than:
		return "greater_than"
	case greater_equal:
		return "greater_equal"
	case less_than:
		return "less_than"
	case less_equal:
		return "less_equal"
	case equal:
		return "equal"
	case equal_equal:
		return "equal_equal"
	case not_equal:
		return "not_equal"
	case question_equal:
		return "question_equal"
	case dot:
		return "dot"
	case colon:
		return "colon"
	case at:
		return "at"
	case pipe:
		return "pipe"
	case dollar:
		return "dollar"
	case macro_param:
		return "macro_param"
	case script_value:
		return "script_value"
	case script_math:
		return "script_math"
	case percent:
		return "percent"
	case comment:
		return "comment"
	case invalid:
		return "invalid"
	case eof:
		return "eof"
	default:
		return fmt.Sprintf("Tag(%d)", uint8(t))
	}
}

// Exported Tag aliases for use outside the lexer package.
const (
	TagIdentifier    = identifier
	TagLiteralNumber = literal_number
	TagLiteralString = literal_string
	TagLiteralBoolean = literal_boolean
	TagLiteralDate   = literal_date
	TagLBrace        = l_brace
	TagRBrace        = r_brace
	TagLBracket      = l_bracket
	TagRBracket      = r_bracket
	TagPlus          = plus
	TagMinus         = minus
	TagMultiply      = multiply
	TagDivide        = divide
	TagGreaterThan   = greater_than
	TagGreaterEqual  = greater_equal
	TagLessThan      = less_than
	TagLessEqual     = less_equal
	TagEqual         = equal
	TagEqualEqual    = equal_equal
	TagNotEqual      = not_equal
	TagQuestionEqual = question_equal
	TagDot           = dot
	TagColon         = colon
	TagAt            = at
	TagPipe          = pipe
	TagDollar        = dollar
	TagMacroParam    = macro_param
	TagScriptValue   = script_value
	TagScriptMath    = script_math
	TagPercent       = percent
	TagInvalid       = invalid
	TagEOF           = eof
)

// IsInvalid reports whether the token is an invalid token.
func (t Token) IsInvalid() bool {
	return t.Tag == invalid
}

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
