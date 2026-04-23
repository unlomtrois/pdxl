// Package cache provides a two-level parse cache: an in-memory LRU backed by
// an on-disk gob store. Invalidation is mtime-first with SHA-256 fallback.
package cache

import (
	"bytes"
	"compress/gzip"
	"container/list"
	"crypto/sha256"
	"encoding/gob"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sync"

	v3 "pdxl/internal/parser/v3"
)

// ── Disk entry ────────────────────────────────────────────────────────────────

type diskEntry struct {
	ModTime int64
	SHA256  [32]byte
	SrcGzip []byte
	Nodes   []v3.Node
	Index   []uint32
	Diags   []v3.Diagnostic
}

// ── In-memory LRU ─────────────────────────────────────────────────────────────

type memEntry struct {
	modTime int64
	tree    *v3.Tree
	diags   []v3.Diagnostic
}

type lruItem struct {
	key string
	e   memEntry
}

type lruCache struct {
	cap   int
	items map[string]*list.Element
	list  *list.List
}

func newLRU(cap int) *lruCache {
	return &lruCache{
		cap:   cap,
		items: make(map[string]*list.Element),
		list:  list.New(),
	}
}

func (c *lruCache) get(key string) (memEntry, bool) {
	el, ok := c.items[key]
	if !ok {
		return memEntry{}, false
	}
	c.list.MoveToFront(el)
	return el.Value.(*lruItem).e, true
}

func (c *lruCache) put(key string, e memEntry) {
	if el, ok := c.items[key]; ok {
		el.Value.(*lruItem).e = e
		c.list.MoveToFront(el)
		return
	}
	if len(c.items) == c.cap {
		back := c.list.Back()
		if back != nil {
			c.list.Remove(back)
			delete(c.items, back.Value.(*lruItem).key)
		}
	}
	el := c.list.PushFront(&lruItem{key: key, e: e})
	c.items[key] = el
}

func (c *lruCache) delete(key string) {
	if el, ok := c.items[key]; ok {
		c.list.Remove(el)
		delete(c.items, key)
	}
}

// ── Store ─────────────────────────────────────────────────────────────────────

// Store is a two-level parse cache.
type Store struct {
	dir string
	mu  sync.RWMutex
	lru *lruCache // nil when cap == 0
}

// NewStore creates a Store backed by dir. lruCap controls the in-memory LRU
// size; pass 0 to skip L1 and use disk only.
func NewStore(dir string, lruCap int) (*Store, error) {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, err
	}
	// Keep cache files out of version control.
	pdxlDir := filepath.Dir(dir)
	gitignore := filepath.Join(pdxlDir, ".gitignore")
	if _, err := os.Stat(gitignore); os.IsNotExist(err) {
		_ = os.WriteFile(gitignore, []byte("*\n"), 0o644)
	}
	s := &Store{dir: dir}
	if lruCap > 0 {
		s.lru = newLRU(lruCap)
	}
	return s, nil
}

// Get returns the cached tree for path, or (nil, nil, nil) on miss/stale.
// info must be the result of os.Stat(path) — caller already has it.
func (s *Store) Get(path string, info os.FileInfo) (*v3.Tree, []v3.Diagnostic, error) {
	modTime := info.ModTime().UnixNano()

	s.mu.RLock()
	if s.lru != nil {
		if e, ok := s.lru.get(path); ok {
			s.mu.RUnlock()
			if e.modTime == modTime {
				return e.tree, e.diags, nil
			}
			// stale L1 entry; evict under write lock below
			s.mu.Lock()
			s.lru.delete(path)
			s.mu.Unlock()
			// fall through to L2
		} else {
			s.mu.RUnlock()
		}
	} else {
		s.mu.RUnlock()
	}

	de, err := readDiskEntry(s.dir, path)
	if err != nil {
		return nil, nil, nil // cold miss
	}

	if de.ModTime == modTime {
		tree := reconstructTree(de)
		s.mu.Lock()
		if s.lru != nil {
			s.lru.put(path, memEntry{modTime: modTime, tree: tree, diags: de.Diags})
		}
		s.mu.Unlock()
		return tree, de.Diags, nil
	}

	// mtime changed — read source and verify hash
	src, err := os.ReadFile(path)
	if err != nil {
		return nil, nil, nil
	}
	h := sha256.Sum256(src)
	if h != de.SHA256 {
		return nil, nil, nil // content changed; caller must re-parse
	}

	// same content, different mtime — refresh the stored mtime
	de.ModTime = modTime
	if werr := writeDiskEntry(s.dir, path, de); werr != nil {
		return nil, nil, werr
	}
	tree := reconstructTree(de)
	s.mu.Lock()
	if s.lru != nil {
		s.lru.put(path, memEntry{modTime: modTime, tree: tree, diags: de.Diags})
	}
	s.mu.Unlock()
	return tree, de.Diags, nil
}

// Put stores a parsed result. src must be the raw bytes of the file at path.
func (s *Store) Put(path string, info os.FileInfo, src []byte, tree *v3.Tree, diags []v3.Diagnostic) error {
	gz, err := gzipCompress(src)
	if err != nil {
		return err
	}
	de := diskEntry{
		ModTime: info.ModTime().UnixNano(),
		SHA256:  sha256.Sum256(src),
		SrcGzip: gz,
		Nodes:   tree.Nodes,
		Index:   tree.Index,
		Diags:   diags,
	}
	if err := writeDiskEntry(s.dir, path, de); err != nil {
		return err
	}
	s.mu.Lock()
	if s.lru != nil {
		s.lru.put(path, memEntry{modTime: de.ModTime, tree: tree, diags: diags})
	}
	s.mu.Unlock()
	return nil
}

// ── Helpers ───────────────────────────────────────────────────────────────────

func entryPath(dir, filePath string) string {
	h := sha256.Sum256([]byte(filepath.Clean(filePath)))
	return filepath.Join(dir, fmt.Sprintf("%x.bin", h))
}

func reconstructTree(de diskEntry) *v3.Tree {
	src := gzipDecompress(de.SrcGzip)
	return &v3.Tree{Nodes: de.Nodes, Index: de.Index, Src: src}
}

func readDiskEntry(dir, path string) (diskEntry, error) {
	f, err := os.Open(entryPath(dir, path))
	if err != nil {
		return diskEntry{}, err
	}
	defer f.Close()
	var de diskEntry
	if err := gob.NewDecoder(f).Decode(&de); err != nil {
		return diskEntry{}, err
	}
	return de, nil
}

func writeDiskEntry(dir, path string, de diskEntry) error {
	ep := entryPath(dir, path)
	f, err := os.Create(ep)
	if err != nil {
		return err
	}
	if err := gob.NewEncoder(f).Encode(de); err != nil {
		f.Close()
		return err
	}
	return f.Close()
}

func gzipCompress(src []byte) ([]byte, error) {
	var buf bytes.Buffer
	w := gzip.NewWriter(&buf)
	if _, err := w.Write(src); err != nil {
		return nil, err
	}
	if err := w.Close(); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

func gzipDecompress(data []byte) []byte {
	r, err := gzip.NewReader(bytes.NewReader(data))
	if err != nil {
		return nil
	}
	out, _ := io.ReadAll(r)
	return out
}
