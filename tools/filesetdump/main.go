// Command filesetdump is a deterministic FileSet / .mod descriptor dump tool for
// differential parity testing. It drives the reference Go files package
// (internal/files) and writes a canonical JSON dump to stdout whose byte layout
// matches pdxl_files::dump_scan / dump_descriptor exactly.
//
// Driven entirely by CLI args (no JSON input) so neither side needs a JSON
// parser. This tool is additive and does not change production behavior.
//
// Scan mode:
//
//	filesetdump scan --root <path>:<kind> [--root ...] \
//	  [--ignore-dir <name>]... [--ignore-file <name>]... \
//	  [--replace <prefix>]... [--query <relpath>]...
//
// Descriptor mode:
//
//	filesetdump descriptor <modfile>
package main

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"

	"pdxl/internal/files"
)

const dumpVersion = 1

func kindString(k files.FileKind) string {
	switch k {
	case files.FileKindVanilla:
		return "vanilla"
	case files.FileKindDLC:
		return "dlc"
	case files.FileKindDependency:
		return "dependency"
	case files.FileKindMod:
		return "mod"
	}
	return "unknown"
}

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

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: filesetdump <scan|descriptor> ...")
		os.Exit(2)
	}
	switch os.Args[1] {
	case "scan":
		runScan(os.Args[2:])
	case "descriptor":
		runDescriptor(os.Args[2:])
	default:
		fmt.Fprintln(os.Stderr, "usage: filesetdump <scan|descriptor> ...")
		os.Exit(2)
	}
}

type rootArg struct {
	path string
	kind files.FileKind
}

func runScan(args []string) {
	var roots []rootArg
	var ignoreDirs, ignoreFiles, replace, queries []string

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
		case "--query":
			queries = append(queries, value)
		default:
			fmt.Fprintf(os.Stderr, "unknown flag: %s\n", flag)
			os.Exit(2)
		}
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

	w := bufio.NewWriter(os.Stdout)
	defer w.Flush()

	var entries []files.FileEntry
	_ = fs.Walk(func(e files.FileEntry) error {
		entries = append(entries, e)
		return nil
	})

	w.WriteString("{\n\"version\":")
	w.WriteString(strconv.Itoa(dumpVersion))
	w.WriteString(",\n\"entries\":[")
	if len(entries) > 0 {
		w.WriteString("\n")
		for i, e := range entries {
			w.WriteString("{\"rel_path\":\"")
			w.WriteString(esc(e.RelPath))
			w.WriteString("\",\"full_path\":\"")
			w.WriteString(esc(e.FullPath))
			w.WriteString("\",\"kind\":\"")
			w.WriteString(kindString(e.Kind))
			w.WriteString("\"}")
			if i+1 < len(entries) {
				w.WriteString(",")
			}
			w.WriteString("\n")
		}
	}
	w.WriteString("],\n")

	st := fs.Stats()
	w.WriteString("\"stats\":{\"vanilla\":")
	w.WriteString(strconv.Itoa(st.Vanilla))
	w.WriteString(",\"mod\":")
	w.WriteString(strconv.Itoa(st.Mod))
	w.WriteString(",\"total\":")
	w.WriteString(strconv.Itoa(st.Total))
	w.WriteString(",\"shadowed\":")
	w.WriteString(strconv.Itoa(st.Shadowed))
	w.WriteString(",\"replaced\":")
	w.WriteString(strconv.Itoa(st.Replaced))
	w.WriteString("},\n")

	w.WriteString("\"resolutions\":[")
	if len(queries) > 0 {
		w.WriteString("\n")
		for i, q := range queries {
			w.WriteString("{\"query\":\"")
			w.WriteString(esc(q))
			if e, ok := fs.Resolve(q); ok {
				w.WriteString("\",\"found\":true,\"rel_path\":\"")
				w.WriteString(esc(e.RelPath))
				w.WriteString("\",\"kind\":\"")
				w.WriteString(kindString(e.Kind))
				w.WriteString("\"}")
			} else {
				w.WriteString("\",\"found\":false,\"rel_path\":null,\"kind\":null}")
			}
			if i+1 < len(queries) {
				w.WriteString(",")
			}
			w.WriteString("\n")
		}
	}
	w.WriteString("]\n}\n")
}

func runDescriptor(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: filesetdump descriptor <modfile>")
		os.Exit(2)
	}
	modFile := args[0]
	m, err := files.ParseMod(modFile)
	if err != nil {
		fmt.Fprintf(os.Stderr, "parsing %s: %v\n", modFile, err)
		os.Exit(1)
	}

	w := bufio.NewWriter(os.Stdout)
	defer w.Flush()

	w.WriteString("{\n\"version\":")
	w.WriteString(strconv.Itoa(dumpVersion))
	w.WriteString(",\n\"name\":\"")
	w.WriteString(esc(m.Name))
	w.WriteString("\",\n\"path\":\"")
	w.WriteString(esc(m.Path))
	w.WriteString("\",\n\"replace_paths\":[")
	for i, rp := range m.ReplacePaths {
		if i > 0 {
			w.WriteString(",")
		}
		w.WriteString("\"")
		w.WriteString(esc(rp))
		w.WriteString("\"")
	}
	w.WriteString("],\n\"is_windows_absolute\":")
	if files.IsWindowsAbsolute(m.Path) {
		w.WriteString("true")
	} else {
		w.WriteString("false")
	}
	w.WriteString("\n}\n")
}
