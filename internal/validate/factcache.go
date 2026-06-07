package validate

import (
	"crypto/sha256"
	"encoding/gob"
	"fmt"
	"os"
	"path/filepath"
)

// FactStore is a per-file disk cache of FileFacts, keyed by file path and
// invalidated by content hash (mtime is advisory; SHA-256 is authoritative,
// mirroring the AST cache). It lets unchanged files skip parsing entirely.
type FactStore struct {
	dir string
}

type factEntry struct {
	ModTime int64
	SHA256  [32]byte
	Facts   FileFacts
}

// NewFactStore creates a FactStore backed by dir.
func NewFactStore(dir string) (*FactStore, error) {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, err
	}
	return &FactStore{dir: dir}, nil
}

// Get returns the cached facts for path, or (zero, false) on miss/stale.
// info must be os.Stat(path) from the caller.
func (s *FactStore) Get(path string, info os.FileInfo) (FileFacts, bool) {
	f, err := os.Open(s.entryPath(path))
	if err != nil {
		return FileFacts{}, false
	}
	defer f.Close()
	var fe factEntry
	if err := gob.NewDecoder(f).Decode(&fe); err != nil {
		return FileFacts{}, false
	}
	src, err := os.ReadFile(path)
	if err != nil {
		return FileFacts{}, false
	}
	if sha256.Sum256(src) != fe.SHA256 {
		return FileFacts{}, false // content changed
	}
	return fe.Facts, true
}

// Put stores facts for path. src must be the raw bytes of the file at path.
func (s *FactStore) Put(path string, info os.FileInfo, src []byte, facts FileFacts) error {
	fe := factEntry{
		ModTime: info.ModTime().UnixNano(),
		SHA256:  sha256.Sum256(src),
		Facts:   facts,
	}
	f, err := os.Create(s.entryPath(path))
	if err != nil {
		return err
	}
	defer f.Close()
	return gob.NewEncoder(f).Encode(fe)
}

func (s *FactStore) entryPath(path string) string {
	h := sha256.Sum256([]byte(filepath.Clean(path)))
	return filepath.Join(s.dir, fmt.Sprintf("%x.bin", h))
}
