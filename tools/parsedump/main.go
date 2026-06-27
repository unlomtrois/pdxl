// Command parsedump is a deterministic structured dump tool for parser parity
// testing. It parses the file given as its single argument with the reference Go
// parser v3 (internal/parser/v3) and writes a canonical, normalized JSON dump to
// stdout, one node and one diagnostic per line.
//
// The schema and byte layout match pdxl_syntax::dump_json exactly, so the Rust
// port can be compared against this oracle byte-for-byte. Filenames are omitted
// from the dump so different checkout paths cannot cause false mismatches.
//
// Normalization: a node's "operator" is the operator tag name only for Field
// nodes; every other node reports "invalid".
//
// This tool is additive and does not change production parser behavior.
package main

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"

	v3 "pdxl/internal/parser/v3"
)

const dumpVersion = 1

func kindString(k v3.NodeKind) string {
	switch k {
	case v3.KindFile:
		return "file"
	case v3.KindField:
		return "field"
	case v3.KindBlock:
		return "block"
	case v3.KindTaggedBlock:
		return "tagged_block"
	case v3.KindScalar:
		return "scalar"
	}
	return "unknown"
}

func severityString(s v3.Severity) string {
	if s == v3.SeverityWarning {
		return "warning"
	}
	return "error"
}

// jsonEscape applies minimal JSON string escaping matching the Rust dumper.
func jsonEscape(s string) string {
	var b strings.Builder
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
				fmt.Fprintf(&b, "\\u%04x", r)
			} else {
				b.WriteRune(r)
			}
		}
	}
	return b.String()
}

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: parsedump <file>")
		os.Exit(2)
	}
	data, err := os.ReadFile(os.Args[1])
	if err != nil {
		fmt.Fprintf(os.Stderr, "reading %s: %v\n", os.Args[1], err)
		os.Exit(1)
	}

	// The dump omits the filename; pass a fixed placeholder.
	tree, diags := v3.Parse("input", data)

	w := bufio.NewWriter(os.Stdout)
	defer w.Flush()

	w.WriteString("{")
	w.WriteString("\"version\":")
	w.WriteString(strconv.Itoa(dumpVersion))
	w.WriteString(",\"source_length\":")
	w.WriteString(strconv.Itoa(len(data)))
	w.WriteString(",\"nodes\":[")
	if len(tree.Nodes) > 0 {
		w.WriteString("\n")
		for i, n := range tree.Nodes {
			operator := "invalid"
			if n.Kind == v3.KindField {
				operator = n.Op.String()
			}
			w.WriteString("{\"id\":")
			w.WriteString(strconv.Itoa(i))
			w.WriteString(",\"kind\":\"")
			w.WriteString(kindString(n.Kind))
			w.WriteString("\",\"start\":")
			w.WriteString(strconv.FormatUint(uint64(n.SrcStart), 10))
			w.WriteString(",\"end\":")
			w.WriteString(strconv.FormatUint(uint64(n.SrcEnd), 10))
			w.WriteString(",\"operator\":\"")
			w.WriteString(operator)
			w.WriteString("\",\"child_start\":")
			w.WriteString(strconv.FormatUint(uint64(n.ChildStart), 10))
			w.WriteString(",\"child_end\":")
			w.WriteString(strconv.FormatUint(uint64(n.ChildEnd), 10))
			w.WriteString("}")
			if i+1 < len(tree.Nodes) {
				w.WriteString(",")
			}
			w.WriteString("\n")
		}
	}
	w.WriteString("],\"child_ids\":[")
	for i, idx := range tree.Index {
		if i > 0 {
			w.WriteString(",")
		}
		w.WriteString(strconv.FormatUint(uint64(idx), 10))
	}
	w.WriteString("],\"diagnostics\":[")
	if len(diags) > 0 {
		w.WriteString("\n")
		for i, d := range diags {
			w.WriteString("{\"offset\":")
			w.WriteString(strconv.Itoa(d.Offset))
			w.WriteString(",\"severity\":\"")
			w.WriteString(severityString(d.Severity))
			w.WriteString("\",\"message\":\"")
			w.WriteString(jsonEscape(d.Msg))
			w.WriteString("\"}")
			if i+1 < len(diags) {
				w.WriteString(",")
			}
			w.WriteString("\n")
		}
	}
	w.WriteString("]}\n")
}
