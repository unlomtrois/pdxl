// Package parser builds an AST from a PDXScript source using participle.
//
// PDXScript block contents are not uniform — a block may contain:
//   - fields:       key op value  (key = value, age > 18, …)
//   - a value list: bare tokens   ({ 255 255 255 }, { GEN GAZ })
//   - a tagged block: tag { … }   (rgb { 218 215 56 })
//
// We model RHS values as a sealed Value interface with three implementations.
// Participle's Union tries them left to right: TaggedBlock → Block → Scalar.
// TaggedBlock must precede Block so "rgb" isn't consumed as a Scalar before
// the parser sees the following "{".
package parser

import (
	"bytes"

	"github.com/alecthomas/participle/v2"
	participleLexer "github.com/alecthomas/participle/v2/lexer"
)

// ── Value union (interface + three implementations) ───────────────────────────

// Value is the sealed interface for right-hand-side values.
type Value interface{ value() }

// TaggedBlock handles constructs like `rgb { 255 0 0 }` and `hsv { 0.5 1.0 0.8 }`.
type TaggedBlock struct {
	Pos   participleLexer.Position
	Tag   string  `parser:"@Identifier"`
	Items []*Item `parser:"'{' @@* '}'"`
}

func (*TaggedBlock) value() {}

// Block is a brace-delimited sequence of items (fields or bare scalars).
type Block struct {
	Pos   participleLexer.Position
	Items []*Item `parser:"'{' @@* '}'"`
}

func (*Block) value() {}

// Scalar is a single token value or a scope/path chain.
//
// Paradox scripting uses several compound token forms that are built from
// individual lexer tokens:
//
//	c:GEN                   — scope type + colon + identifier
//	scope:title.k_france    — scope + colon + dotted path
//	title:k_france.capital  — same
//	great_power_score       — plain identifier
//	255                     — number
//	yes / no                — boolean
//	"text"                  — quoted string
//
// We capture the constituent tokens into Parts and join them for display.
// Using []string with repeated @-captures lets participle accumulate each piece.
type Scalar struct {
	Pos   participleLexer.Position
	Parts []string `parser:"@(Identifier | Number | Boolean | String) ( @(Colon | Dot) @(Identifier | Number) )*"`
}

// Value returns the scalar as a single concatenated string.
func (s *Scalar) Value() string {
	result := ""
	for _, p := range s.Parts {
		result += p
	}
	return result
}

func (*Scalar) value() {}

// ── Item and Field ────────────────────────────────────────────────────────────

// Item is one entry inside a block or at the top level.
// Participle tries Field first; Scalar is the fallback for bare values.
type Item struct {
	Pos    participleLexer.Position
	Field  *Field  `parser:"  @@"`
	Scalar *Scalar `parser:"| @@"`
}

// Field is a key–operator–value triple.
//
//	key = value
//	key = { … }
//	key = rgb { … }
//	age > 18
//	scope:actor ?= { … }
type Field struct {
	Pos      participleLexer.Position
	KeyParts []string `parser:"@(Identifier | Number | Boolean) ( @(Colon | Dot) @(Identifier | Number) )*"`
	Operator string   `parser:"@(Equal | EqualEqual | NotEqual | QuestionEqual | GreaterThan | GreaterEqual | LessThan | LessEqual)"`
	Value    Value    `parser:"@@"`
}

// Key returns the field key as a single concatenated string.
func (f *Field) Key() string {
	result := ""
	for _, p := range f.KeyParts {
		result += p
	}
	return result
}

// ── File root ─────────────────────────────────────────────────────────────────

// File is the root of every parsed PDXScript file.
type File struct {
	Pos   participleLexer.Position
	Items []*Item `parser:"@@*"`
}

// ── Parser construction ───────────────────────────────────────────────────────

// PDXLParser is the ready-to-use PDXScript parser. Build once, reuse across files.
var PDXLParser = participle.MustBuild[File](
	participle.Lexer(PDXLDefinition),
	participle.Union[Value](&TaggedBlock{}, &Block{}, &Scalar{}),
	participle.Elide("Invalid"),
)

// ── Convenience helpers ───────────────────────────────────────────────────────

// ParseBytes parses a PDXScript source buffer, using filename for error messages.
func ParseBytes(filename string, src []byte) (*File, error) {
	return PDXLParser.Parse(filename, bytes.NewReader(src))
}
