// Package files provides directory scanning and mod-overlay resolution for
// PDXScript project files. Callers add source roots in load order (vanilla
// first, mod last); later additions shadow earlier ones for the same relative
// path, matching the Paradox mod overlay semantics.
package files

import (
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	v3 "pdxl/internal/parser/v3"
)

// FileKind identifies where a file originates in the load order.
type FileKind uint8

const (
	FileKindVanilla    FileKind = iota
	FileKindDLC
	FileKindDependency
	FileKindMod
)

// FileEntry is a single resolved .txt file.
type FileEntry struct {
	RelPath  string   // normalised overlay key (forward slashes, lowercase)
	FullPath string   // absolute path for reading
	Kind     FileKind
}

// Stats summarises a FileSet after scanning.
type Stats struct {
	Vanilla  int // winning vanilla entries
	Mod      int // winning mod entries
	Total    int // total winning entries
	Shadowed int // vanilla files overridden by a mod file
	Replaced int // vanilla files dropped due to replace_path
}

// FileSet holds a collection of PDXScript files with overlay semantics applied.
// The zero value is ready to use.
type FileSet struct {
	entries      []FileEntry
	byPath       map[string]int      // RelPath → index of last-added (winning) entry
	replacePaths []string            // normalised prefixes that fully replace vanilla
	replaced     int                 // count of vanilla/DLC files dropped by replace_path
	ignoreDirs   map[string]struct{} // directory base names skipped during Add
	ignoreFiles  map[string]struct{} // file base names skipped during Add (lowercased)
}

// SetIgnore registers directory and file base names to skip during Add, used to
// exclude non-script .txt files (license texts, manifests, etc.). Comparison is
// case-insensitive. Call before adding any roots.
func (s *FileSet) SetIgnore(dirs, files []string) {
	s.ignoreDirs = make(map[string]struct{}, len(dirs))
	for _, d := range dirs {
		s.ignoreDirs[strings.ToLower(d)] = struct{}{}
	}
	s.ignoreFiles = make(map[string]struct{}, len(files))
	for _, f := range files {
		s.ignoreFiles[strings.ToLower(f)] = struct{}{}
	}
}

// SetReplacePaths registers directory prefixes that are fully replaced by the
// mod. Any vanilla file whose RelPath starts with one of these prefixes is
// silently dropped when Add is called with FileKindVanilla or FileKindDLC.
// Call this before adding vanilla files.
func (s *FileSet) SetReplacePaths(paths []string) {
	s.replacePaths = make([]string, len(paths))
	for i, p := range paths {
		s.replacePaths[i] = strings.ToLower(filepath.ToSlash(p))
	}
}

// Add scans root for .txt files and registers them with the given kind.
// It must be called in load order: vanilla first, mod last.
// Directories whose base name starts with '.' are skipped.
func (s *FileSet) Add(root string, kind FileKind) error {
	root = filepath.Clean(root)
	return filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			if s.skipDir(d.Name()) {
				return filepath.SkipDir
			}
			return nil
		}
		if !strings.EqualFold(filepath.Ext(d.Name()), ".txt") {
			return nil
		}
		if _, ok := s.ignoreFiles[strings.ToLower(d.Name())]; ok {
			return nil
		}
		rel, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		s.register(strings.ToLower(filepath.ToSlash(rel)), path, kind)
		return nil
	})
}

// skipDir reports whether a directory (by base name) should not be descended:
// dot-directories and configured ignore_dirs.
func (s *FileSet) skipDir(name string) bool {
	if strings.HasPrefix(name, ".") {
		return true
	}
	_, ignored := s.ignoreDirs[strings.ToLower(name)]
	return ignored
}

// register adds (or overlays) a winning entry for relKey, applying replace_path
// dropping for vanilla/DLC files.
func (s *FileSet) register(relKey, fullPath string, kind FileKind) {
	if (kind == FileKindVanilla || kind == FileKindDLC) && s.isReplaced(relKey) {
		s.replaced++
		return
	}
	entry := FileEntry{RelPath: relKey, FullPath: fullPath, Kind: kind}
	if s.byPath == nil {
		s.byPath = make(map[string]int)
	}
	if idx, ok := s.byPath[relKey]; ok {
		s.entries[idx] = entry
	} else {
		s.byPath[relKey] = len(s.entries)
		s.entries = append(s.entries, entry)
	}
}

// Resolve returns the winning FileEntry for relPath (normalised to lowercase
// forward-slash form), or (FileEntry{}, false) if absent.
func (s *FileSet) Resolve(relPath string) (FileEntry, bool) {
	relPath = strings.ToLower(filepath.ToSlash(relPath))
	if idx, ok := s.byPath[relPath]; ok {
		return s.entries[idx], true
	}
	return FileEntry{}, false
}

// Stats returns a summary of the FileSet after all Add calls have been made.
func (s *FileSet) Stats() Stats {
	var st Stats
	st.Replaced = s.replaced
	for i, e := range s.entries {
		if s.byPath[e.RelPath] != i {
			if e.Kind == FileKindVanilla || e.Kind == FileKindDLC {
				st.Shadowed++
			}
			continue
		}
		st.Total++
		switch e.Kind {
		case FileKindVanilla, FileKindDLC:
			st.Vanilla++
		case FileKindMod, FileKindDependency:
			st.Mod++
		}
	}
	return st
}

// isReplaced reports whether relPath falls under a replace_path prefix.
func (s *FileSet) isReplaced(relPath string) bool {
	for _, prefix := range s.replacePaths {
		if relPath == prefix || strings.HasPrefix(relPath, prefix+"/") {
			return true
		}
	}
	return false
}

// Mod holds metadata parsed from a .mod file.
type Mod struct {
	Name         string
	Path         string   // resolved absolute path to mod directory
	ReplacePaths []string // relative paths that fully replace vanilla
}

// ParseMod reads a CK3 .mod file and returns its metadata.
// path is the path to the .mod file itself; the mod directory in the Mod.Path
// field is resolved relative to the .mod file's directory.
// Windows-style absolute paths (C:/...) in the path field are returned as-is
// and must be resolved by the caller if needed.
func ParseMod(modFile string) (Mod, error) {
	src, err := os.ReadFile(modFile)
	if err != nil {
		return Mod{}, err
	}
	tree, _ := v3.Parse(modFile, src)
	root := tree.Root()

	var m Mod
	for _, ref := range tree.ChildRefs(root) {
		node := tree.Nodes[ref]
		if node.Kind != v3.KindField {
			continue
		}
		children := tree.ChildRefs(node)
		if len(children) < 2 {
			continue
		}
		key := string(tree.Nodes[children[0]].Value(tree.Src))
		val := string(tree.Nodes[children[1]].Value(tree.Src))
		switch key {
		case "name":
			m.Name = strings.Trim(val, `"`)
		case "path":
			raw := strings.Trim(val, `"`)
			// Absolute paths are kept verbatim: Windows-shaped (C:/...) for
			// Proton-managed descriptors, and native absolute paths — the Linux
			// launcher writes absolute Unix paths, which used to be wrongly
			// joined onto the .mod directory.
			if IsWindowsAbsolute(raw) || filepath.IsAbs(raw) {
				m.Path = raw
			} else {
				m.Path = filepath.Join(filepath.Dir(modFile), filepath.FromSlash(raw))
			}
		case "replace_path":
			m.ReplacePaths = append(m.ReplacePaths, strings.Trim(val, `"`))
		}
	}
	return m, nil
}

// IsWindowsAbsolute reports whether p looks like a Windows absolute path (C:/...).
func IsWindowsAbsolute(p string) bool {
	return len(p) >= 3 && p[1] == ':' && (p[2] == '/' || p[2] == '\\')
}

// ResolveWindowsPath translates a Windows absolute path (e.g. C:/users/...)
// to a Linux path under the given Proton/Wine prefix by mapping the drive
// letter to <prefix>/drive_c (only C: is supported).
func ResolveWindowsPath(winPath, protonPrefix string) string {
	// Normalise backslashes and strip the drive letter + separator.
	p := strings.ReplaceAll(winPath, "\\", "/")
	if len(p) >= 3 && p[1] == ':' {
		p = p[3:] // strip "C:/"
	}
	return filepath.Join(filepath.Clean(protonPrefix), "drive_c", filepath.FromSlash(p))
}

// Walk calls fn for each winning entry in stable insertion order.
// Returning an error from fn stops the walk and returns that error.
func (s *FileSet) Walk(fn func(FileEntry) error) error {
	for i, e := range s.entries {
		if s.byPath[e.RelPath] == i {
			if err := fn(e); err != nil {
				return err
			}
		}
	}
	return nil
}
