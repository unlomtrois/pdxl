// Command projectdump is a deterministic whole-project analysis dump for
// differential parity testing. It builds a FileSet from ordered roots (same
// flag style as filesetdump scan), runs the reference analysis
// (validate.Analyze with no caches), and writes a canonical JSON dump: symbol
// counts by kind, duplicates in merge order, and unresolved-reference
// diagnostics in walk order. Byte layout matches pdxl_parity::dump_project.
//
// Usage:
//
//	projectdump --root <path>:<kind> [--root ...] \
//	  [--ignore-dir <name>]... [--ignore-file <name>]... [--replace <prefix>]...
//
// This tool is additive and does not change production behavior.
package main

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"

	"pdxl/internal/files"
	"pdxl/internal/validate"
)

const dumpVersion = 1

func parseKind(s string) (files.FileKind, bool) {
	switch s {
	case "vanilla":
		return files.FileKindVanilla, true
	case "dlc":
		return files.FileKindDLC, true
	case "dependency":
		return files.FileKindDependency, true
	case "mod":
		return files.FileKindMod, true
	}
	return 0, false
}

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

type rootArg struct {
	path string
	kind files.FileKind
}

func main() {
	args := os.Args[1:]
	var roots []rootArg
	var ignoreDirs, ignoreFiles, replace []string

	for i := 0; i < len(args); i += 2 {
		flag := args[i]
		if i+1 >= len(args) {
			fmt.Fprintf(os.Stderr, "missing value for %s\n", flag)
			os.Exit(2)
		}
		value := args[i+1]
		switch flag {
		case "--root":
			idx := strings.LastIndex(value, ":")
			if idx < 0 {
				fmt.Fprintln(os.Stderr, "--root expects <path>:<kind>")
				os.Exit(2)
			}
			kind, ok := parseKind(value[idx+1:])
			if !ok {
				fmt.Fprintf(os.Stderr, "unknown kind: %s\n", value[idx+1:])
				os.Exit(2)
			}
			roots = append(roots, rootArg{path: value[:idx], kind: kind})
		case "--ignore-dir":
			ignoreDirs = append(ignoreDirs, value)
		case "--ignore-file":
			ignoreFiles = append(ignoreFiles, value)
		case "--replace":
			replace = append(replace, value)
		default:
			fmt.Fprintf(os.Stderr, "unknown flag: %s\n", flag)
			os.Exit(2)
		}
	}
	if len(roots) == 0 {
		fmt.Fprintln(os.Stderr, "usage: projectdump --root <path>:<kind> ...")
		os.Exit(2)
	}

	var fs files.FileSet
	fs.SetIgnore(ignoreDirs, ignoreFiles)
	fs.SetReplacePaths(replace)
	for _, r := range roots {
		if err := fs.Add(r.path, r.kind); err != nil {
			fmt.Fprintf(os.Stderr, "scanning %s: %v\n", r.path, err)
			os.Exit(1)
		}
	}

	tbl, diags, err := validate.Analyze(&fs, nil, nil)
	if err != nil {
		fmt.Fprintf(os.Stderr, "analyze: %v\n", err)
		os.Exit(1)
	}

	w := bufio.NewWriter(os.Stdout)
	defer w.Flush()

	w.WriteString("{\n\"version\":")
	w.WriteString(strconv.Itoa(dumpVersion))
	w.WriteString(",\n\"counts\":{")
	for i, k := range validate.Kinds {
		if i > 0 {
			w.WriteString(",")
		}
		w.WriteString("\"")
		w.WriteString(k.String())
		w.WriteString("\":")
		w.WriteString(strconv.Itoa(tbl.Count(k)))
	}
	w.WriteString(",\"total\":")
	w.WriteString(strconv.Itoa(tbl.Total()))
	w.WriteString("},\n\"duplicates\":[")
	if len(tbl.Duplicates) > 0 {
		w.WriteString("\n")
		for i, d := range tbl.Duplicates {
			w.WriteString("{\"kind\":\"")
			w.WriteString(d.Kind.String())
			w.WriteString("\",\"name\":\"")
			w.WriteString(esc(d.Name))
			w.WriteString("\",\"first_file\":\"")
			w.WriteString(esc(d.First.File))
			w.WriteString("\",\"file\":\"")
			w.WriteString(esc(d.File))
			w.WriteString("\"}")
			if i+1 < len(tbl.Duplicates) {
				w.WriteString(",")
			}
			w.WriteString("\n")
		}
	}
	w.WriteString("],\n\"unresolved\":[")
	if len(diags) > 0 {
		w.WriteString("\n")
		for i, d := range diags {
			w.WriteString("{\"file\":\"")
			w.WriteString(esc(d.File))
			w.WriteString("\",\"start\":")
			w.WriteString(strconv.Itoa(d.Start))
			w.WriteString(",\"end\":")
			w.WriteString(strconv.Itoa(d.End))
			w.WriteString(",\"loc\":\"")
			w.WriteString(esc(d.Loc))
			w.WriteString("\",\"msg\":\"")
			w.WriteString(esc(d.Msg))
			w.WriteString("\"}")
			if i+1 < len(diags) {
				w.WriteString(",")
			}
			w.WriteString("\n")
		}
	}
	w.WriteString("]\n}\n")
}
