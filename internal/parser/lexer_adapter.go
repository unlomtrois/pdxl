package parser

// lexer_adapter.go — bridges internal/lexer into participle's lexer interfaces.
//
// Participle requires two interfaces:
//   - lexer.Definition  — describes the token type universe (symbol table)
//   - lexer.Lexer       — produces tokens from a byte stream
//
// We map our Tag constants to participle token type IDs and wrap Lexer.Next().

import (
	"io"

	participleLexer "github.com/alecthomas/participle/v2/lexer"

	pdxlLexer "pdxl/internal/lexer"
)

// pdxlDefinition implements participle/v2/lexer.Definition.
type pdxlDefinition struct{}

// PDXLDefinition is the singleton lexer definition passed to participle.Build.
var PDXLDefinition participleLexer.Definition = pdxlDefinition{}

// Symbols returns the mapping of token name → participle token type ID.
// participle uses int token type IDs; we use our Tag values directly (+1 to
// avoid 0 which participle reserves for EOF).
func (pdxlDefinition) Symbols() map[string]participleLexer.TokenType {
	return map[string]participleLexer.TokenType{
		"Identifier":    tokenType(pdxlLexer.TagIdentifier),
		"Number":        tokenType(pdxlLexer.TagLiteralNumber),
		"String":        tokenType(pdxlLexer.TagLiteralString),
		"Boolean":       tokenType(pdxlLexer.TagLiteralBoolean),
		"LBrace":        tokenType(pdxlLexer.TagLBrace),
		"RBrace":        tokenType(pdxlLexer.TagRBrace),
		"LBracket":      tokenType(pdxlLexer.TagLBracket),
		"RBracket":      tokenType(pdxlLexer.TagRBracket),
		"Plus":          tokenType(pdxlLexer.TagPlus),
		"Minus":         tokenType(pdxlLexer.TagMinus),
		"Multiply":      tokenType(pdxlLexer.TagMultiply),
		"Divide":        tokenType(pdxlLexer.TagDivide),
		"GreaterThan":   tokenType(pdxlLexer.TagGreaterThan),
		"GreaterEqual":  tokenType(pdxlLexer.TagGreaterEqual),
		"LessThan":      tokenType(pdxlLexer.TagLessThan),
		"LessEqual":     tokenType(pdxlLexer.TagLessEqual),
		"Equal":         tokenType(pdxlLexer.TagEqual),
		"EqualEqual":    tokenType(pdxlLexer.TagEqualEqual),
		"NotEqual":      tokenType(pdxlLexer.TagNotEqual),
		"QuestionEqual": tokenType(pdxlLexer.TagQuestionEqual),
		"Dot":           tokenType(pdxlLexer.TagDot),
		"Colon":         tokenType(pdxlLexer.TagColon),
		"At":            tokenType(pdxlLexer.TagAt),
		"Pipe":          tokenType(pdxlLexer.TagPipe),
		"Dollar":        tokenType(pdxlLexer.TagDollar),
		"Percent":       tokenType(pdxlLexer.TagPercent),
		"Invalid":       tokenType(pdxlLexer.TagInvalid),
	}
}

// Lex returns a new participle Lexer for the given reader.
func (pdxlDefinition) Lex(filename string, r io.Reader) (participleLexer.Lexer, error) {
	src, err := io.ReadAll(r)
	if err != nil {
		return nil, err
	}
	return &pdxlLexerAdapter{
		l:        pdxlLexer.Init(src),
		src:      src,
		filename: filename,
	}, nil
}

// tokenType converts a pdxl Tag to a participle TokenType.
// +1 so that Tag(0) != participle's EOF (0).
func tokenType(tag pdxlLexer.Tag) participleLexer.TokenType {
	return participleLexer.TokenType(int(tag) + 1)
}

// pdxlLexerAdapter implements participle/v2/lexer.Lexer.
type pdxlLexerAdapter struct {
	l        *pdxlLexer.Lexer
	src      []byte
	filename string
	line     int
	col      int
	offset   int
}

// Next returns the next participle token, or the EOF sentinel.
func (a *pdxlLexerAdapter) Next() (participleLexer.Token, error) {
	tok := a.l.Next()
	if tok == nil {
		return participleLexer.EOFToken(participleLexer.Position{
			Filename: a.filename,
			Line:     a.line + 1,
			Column:   a.col + 1,
			Offset:   a.offset,
		}), nil
	}

	pos := a.positionOf(tok.Start)
	a.offset = tok.End
	a.line, a.col = a.lineColAt(tok.End)

	return participleLexer.Token{
		Type:  tokenType(tok.Tag),
		Value: string(tok.GetValue(a.src)),
		Pos:   pos,
	}, nil
}

// positionOf returns the participle Position for a byte offset.
func (a *pdxlLexerAdapter) positionOf(offset int) participleLexer.Position {
	line, col := a.lineColAt(offset)
	return participleLexer.Position{
		Filename: a.filename,
		Line:     line + 1,
		Column:   col + 1,
		Offset:   offset,
	}
}

// lineColAt computes 0-based line and column for a byte offset by scanning src.
func (a *pdxlLexerAdapter) lineColAt(offset int) (line, col int) {
	for i := 0; i < offset && i < len(a.src); i++ {
		if a.src[i] == '\n' {
			line++
			col = 0
		} else {
			col++
		}
	}
	return
}
