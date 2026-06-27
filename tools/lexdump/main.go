// Command lexdump is a deterministic token-dump tool for lexer parity testing.
//
// It tokenizes the file given as its single argument using the reference Go
// lexer (internal/lexer) and writes one token per line to stdout in a stable,
// structured format:
//
//	<tag>\t<start>\t<end>
//
// where <tag> is lexer.Tag.String(), and <start>/<end> are zero-based,
// half-open byte offsets into the source. Every token returned by Lexer.Next
// is emitted, including invalid tokens, so invalid/partial input behavior is
// covered. The source slice for a token is source[start:end] by definition, so
// comparing (tag, start, end) against another implementation reading the same
// bytes is a complete token-stream comparison.
//
// This tool is additive: it does not modify the behavior of the existing Go
// implementation. It exists only to drive differential tests for the Rust port.
package main

import (
	"bufio"
	"fmt"
	"os"

	"pdxl/internal/lexer"
)

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: lexdump <file>")
		os.Exit(2)
	}
	data, err := os.ReadFile(os.Args[1])
	if err != nil {
		fmt.Fprintf(os.Stderr, "reading %s: %v\n", os.Args[1], err)
		os.Exit(1)
	}

	w := bufio.NewWriter(os.Stdout)
	defer w.Flush()

	l := lexer.Init(data)
	for {
		tok := l.Next()
		if tok == nil {
			break
		}
		fmt.Fprintf(w, "%s\t%d\t%d\n", tok.Tag.String(), tok.Start, tok.End)
	}
}
