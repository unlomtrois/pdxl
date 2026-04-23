package cache

import (
	"os"
	"path/filepath"
	"reflect"
	"sync"
	"testing"
	"time"

	v3 "pdxl/internal/parser/v3"
)

// writeTempFile creates a temp file with content and returns the path.
func writeTempFile(t *testing.T, dir string, content []byte) string {
	t.Helper()
	f, err := os.CreateTemp(dir, "pdxl-cache-test-*.txt")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := f.Write(content); err != nil {
		t.Fatal(err)
	}
	f.Close()
	return f.Name()
}

func newTestStore(t *testing.T, lruCap int) (*Store, string) {
	t.Helper()
	dir := t.TempDir()
	s, err := NewStore(filepath.Join(dir, "cache"), lruCap)
	if err != nil {
		t.Fatal(err)
	}
	return s, dir
}

func parseFile(t *testing.T, path string) ([]byte, os.FileInfo, *v3.Tree, []v3.Diagnostic) {
	t.Helper()
	src, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	tree, diags := v3.Parse(path, src)
	return src, info, tree, diags
}

func TestRoundTrip(t *testing.T) {
	s, dir := newTestStore(t, 4)
	path := writeTempFile(t, dir, []byte(`key = value`))
	src, info, tree, diags := parseFile(t, path)

	if err := s.Put(path, info, src, tree, diags); err != nil {
		t.Fatal(err)
	}

	got, gotDiags, err := s.Get(path, info)
	if err != nil {
		t.Fatal(err)
	}
	if got == nil {
		t.Fatal("expected cache hit, got nil")
	}
	if !reflect.DeepEqual(got.Nodes, tree.Nodes) {
		t.Errorf("Nodes mismatch")
	}
	if !reflect.DeepEqual(got.Index, tree.Index) {
		t.Errorf("Index mismatch")
	}
	if string(got.Src) != string(src) {
		t.Errorf("Src mismatch")
	}
	if !reflect.DeepEqual(gotDiags, diags) {
		t.Errorf("Diags mismatch")
	}
}

func TestColdMiss(t *testing.T) {
	s, dir := newTestStore(t, 4)
	path := writeTempFile(t, dir, []byte(`key = value`))
	_, info, _, _ := parseFile(t, path)

	got, diags, err := s.Get(path, info)
	if err != nil || got != nil || diags != nil {
		t.Errorf("expected cold miss, got tree=%v diags=%v err=%v", got, diags, err)
	}
}

func TestMtimeHit(t *testing.T) {
	s, dir := newTestStore(t, 4)
	path := writeTempFile(t, dir, []byte(`key = value`))
	src, info, tree, diags := parseFile(t, path)

	if err := s.Put(path, info, src, tree, diags); err != nil {
		t.Fatal(err)
	}
	// Clear L1 to force L2 path
	s.lru.delete(path)

	got, _, err := s.Get(path, info)
	if err != nil || got == nil {
		t.Fatalf("expected L2 hit, got nil (err=%v)", err)
	}
}

func TestMtimeStale(t *testing.T) {
	s, dir := newTestStore(t, 4)
	path := writeTempFile(t, dir, []byte(`key = value`))
	src, info, tree, diags := parseFile(t, path)

	if err := s.Put(path, info, src, tree, diags); err != nil {
		t.Fatal(err)
	}

	// Overwrite with different content to change hash
	if err := os.WriteFile(path, []byte(`key = changed`), 0o644); err != nil {
		t.Fatal(err)
	}
	// Touch to change mtime
	newTime := time.Now().Add(time.Second)
	if err := os.Chtimes(path, newTime, newTime); err != nil {
		t.Fatal(err)
	}

	newInfo, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}

	got, _, err := s.Get(path, newInfo)
	if err != nil || got != nil {
		t.Errorf("expected stale miss, got tree=%v err=%v", got, err)
	}
}

func TestMtimeChangedSameContent(t *testing.T) {
	s, dir := newTestStore(t, 4)
	path := writeTempFile(t, dir, []byte(`key = value`))
	src, info, tree, diags := parseFile(t, path)

	if err := s.Put(path, info, src, tree, diags); err != nil {
		t.Fatal(err)
	}
	s.lru.delete(path) // force L2

	// Change mtime but keep content identical
	newTime := info.ModTime().Add(2 * time.Second)
	if err := os.Chtimes(path, newTime, newTime); err != nil {
		t.Fatal(err)
	}
	newInfo, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}

	got, _, err := s.Get(path, newInfo)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if got == nil {
		t.Fatal("expected cache hit despite mtime change (same content)")
	}
}

func TestLRUEviction(t *testing.T) {
	s, dir := newTestStore(t, 2)

	path1 := writeTempFile(t, dir, []byte(`a = 1`))
	path2 := writeTempFile(t, dir, []byte(`b = 2`))
	path3 := writeTempFile(t, dir, []byte(`c = 3`))

	for _, path := range []string{path1, path2, path3} {
		src, info, tree, diags := parseFile(t, path)
		if err := s.Put(path, info, src, tree, diags); err != nil {
			t.Fatal(err)
		}
	}

	// path1 should have been evicted from L1 (LRU cap=2)
	s.mu.RLock()
	_, inL1 := s.lru.items[path1]
	s.mu.RUnlock()
	if inL1 {
		t.Error("expected path1 to be evicted from L1")
	}

	// but path1 must still be on disk
	info1, err := os.Stat(path1)
	if err != nil {
		t.Fatal(err)
	}
	got, _, err := s.Get(path1, info1)
	if err != nil || got == nil {
		t.Errorf("expected disk hit for evicted path1, got nil (err=%v)", err)
	}
}

func TestConcurrentReads(t *testing.T) {
	s, dir := newTestStore(t, 16)
	path := writeTempFile(t, dir, []byte(`key = value`))
	src, info, tree, diags := parseFile(t, path)
	if err := s.Put(path, info, src, tree, diags); err != nil {
		t.Fatal(err)
	}

	var wg sync.WaitGroup
	for range 10 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			got, _, err := s.Get(path, info)
			if err != nil || got == nil {
				t.Errorf("concurrent Get failed: tree=%v err=%v", got, err)
			}
		}()
	}
	wg.Wait()
}

func TestNoCache(t *testing.T) {
	s, dir := newTestStore(t, 0) // cap=0 disables L1
	if s.lru != nil {
		t.Fatal("expected nil LRU when cap=0")
	}

	path := writeTempFile(t, dir, []byte(`key = value`))
	src, info, tree, diags := parseFile(t, path)

	if err := s.Put(path, info, src, tree, diags); err != nil {
		t.Fatal(err)
	}
	got, _, err := s.Get(path, info)
	if err != nil || got == nil {
		t.Errorf("expected disk hit with no L1, got nil (err=%v)", err)
	}
}
