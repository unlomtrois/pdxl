// Command factsdump is a deterministic FileFacts dump tool for parity testing.
//
// Usage: factsdump <file> <relpath> [<relpath>...]
//
// It parses <file> once with parser v3, then runs the reference fact extraction
// (internal/validate.ExtractFileFacts) once per given relpath — the relative
// path drives the definition rule and the on_action gating, so one fixture can
// be exercised under several directory personas in a single invocation. One
// JSON dump is emitted per relpath, in order, with defs/aliases/refs one per
// line; the byte layout matches pdxl_parity::dump_facts exactly.
//
// The <file> argument is used verbatim as the extraction fullPath, so both
// implementations produce identical ref locations when given identical args.
//
// This tool is additive and does not change production behavior.
package main

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"

	v3 "pdxl/internal/parser/v3"
	"pdxl/internal/validate"
)

const dumpVersion = 1

func jsonEscape(b *strings.Builder, s string) {
	for _, r := range s {
		switch r {
		case '"':
			b.WriteString("\\\"")
		case '\\':
			b.WriteString("\\\\")
		case '\n':
			b.WriteString("\\n")
		case '\r':
			b.WriteString("\\r")
		case '\t':
			b.WriteString("\\t")
		default:
			if r < 0x20 {
				fmt.Fprintf(b, "\\u%04x", r)
			} else {
				b.WriteRune(r)
			}
		}
	}
}

func esc(s string) string {
	var b strings.Builder
	jsonEscape(&b, s)
	return b.String()
}

func writeSymbols(w *bufio.Writer, symbols []validate.Symbol) {
	if len(symbols) > 0 {
		w.WriteString("\n")
		for i, s := range symbols {
			w.WriteString("{\"name\":\"")
			w.WriteString(esc(s.Name))
			w.WriteString("\",\"kind\":\"")
			w.WriteString(s.Kind.String())
			w.WriteString("\",\"file\":\"")
			w.WriteString(esc(s.File))
			w.WriteString("\",\"offset\":")
			w.WriteString(strconv.Itoa(s.Offset))
			w.WriteString(",\"end_offset\":")
			w.WriteString(strconv.Itoa(s.EndOffset))
			w.WriteString(",\"params\":[")
			for j, p := range s.Params {
				if j > 0 {
					w.WriteString(",")
				}
				w.WriteString("\"")
				w.WriteString(esc(p))
				w.WriteString("\"")
			}
			w.WriteString("]}")
			if i+1 < len(symbols) {
				w.WriteString(",")
			}
			w.WriteString("\n")
		}
	}
}

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "usage: factsdump <file> <relpath> [<relpath>...]")
		os.Exit(2)
	}
	file := os.Args[1]
	src, err := os.ReadFile(file)
	if err != nil {
		fmt.Fprintf(os.Stderr, "reading %s: %v\n", file, err)
		os.Exit(1)
	}
	tree, _ := v3.Parse(file, src)

	w := bufio.NewWriter(os.Stdout)
	defer w.Flush()

	for _, relPath := range os.Args[2:] {
		f := validate.ExtractFileFacts(tree, relPath, file)

		w.WriteString("{\n\"version\":")
		w.WriteString(strconv.Itoa(dumpVersion))
		w.WriteString(",\n\"rel_path\":\"")
		w.WriteString(esc(relPath))
		w.WriteString("\",\n\"defs\":[")
		writeSymbols(w, f.Defs)
		w.WriteString("],\n\"aliases\":[")
		writeSymbols(w, f.Aliases)
		w.WriteString("],\n\"refs\":[")
		if len(f.Refs) > 0 {
			w.WriteString("\n")
			for i, r := range f.Refs {
				w.WriteString("{\"kind\":\"")
				w.WriteString(r.Kind.String())
				w.WriteString("\",\"name\":\"")
				w.WriteString(esc(r.Name))
				w.WriteString("\",\"file\":\"")
				w.WriteString(esc(r.File))
				w.WriteString("\",\"start\":")
				w.WriteString(strconv.Itoa(r.Start))
				w.WriteString(",\"end\":")
				w.WriteString(strconv.Itoa(r.End))
				w.WriteString(",\"loc\":\"")
				w.WriteString(esc(r.Loc))
				w.WriteString("\"}")
				if i+1 < len(f.Refs) {
					w.WriteString(",")
				}
				w.WriteString("\n")
			}
		}
		w.WriteString("]\n}\n")
	}
}
