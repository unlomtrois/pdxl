package lsp

import (
	"net/url"
	"path/filepath"
	"strings"
	"unicode/utf8"

	protocol "github.com/tliron/glsp/protocol_3_16"
)

// offsetToPosition converts a byte offset in text to an LSP Position (0-based
// line, character in UTF-16 code units, as the protocol requires).
func offsetToPosition(text []byte, off int) protocol.Position {
	if off > len(text) {
		off = len(text)
	}
	line, col := 0, 0
	for i := 0; i < off; {
		r, size := utf8.DecodeRune(text[i:])
		if r == '\n' {
			line++
			col = 0
		} else {
			col += utf16Len(r)
		}
		i += size
	}
	return protocol.Position{Line: uint32(line), Character: uint32(col)}
}

// positionToOffset converts an LSP Position (0-based line, character in UTF-16
// code units) to a byte offset in text. Returns len(text) if the position is
// past the end.
func positionToOffset(text []byte, pos protocol.Position) int {
	line, col := uint32(0), uint32(0)
	for i := 0; i < len(text); {
		if line == pos.Line && col == pos.Character {
			return i
		}
		if line > pos.Line {
			return i
		}
		r, size := utf8.DecodeRune(text[i:])
		if r == '\n' {
			if line == pos.Line {
				// We didn't reach the target column on this line.
				return i
			}
			line++
			col = 0
		} else {
			col += uint32(utf16Len(r))
		}
		i += size
	}
	return len(text)
}

// utf16Len returns the number of UTF-16 code units a rune occupies.
func utf16Len(r rune) int {
	if r >= 0x10000 {
		return 2 // surrogate pair
	}
	return 1
}

// uriToPath converts a file:// document URI to a cleaned local path.
func uriToPath(uri string) string {
	s := strings.TrimPrefix(uri, "file://")
	if p, err := url.PathUnescape(s); err == nil {
		s = p
	}
	return filepath.Clean(s)
}

// pathToURI converts a local path to a file:// document URI.
func pathToURI(path string) string {
	abs, err := filepath.Abs(path)
	if err != nil {
		abs = path
	}
	u := url.URL{Scheme: "file", Path: abs}
	return u.String()
}
