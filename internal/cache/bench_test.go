package cache

import (
	"os"
	"path/filepath"
	"testing"

	"pdxl/internal/testutil"
	v3 "pdxl/internal/parser/v3"
)

func benchmarkSrc(b *testing.B) (string, []byte, os.FileInfo, *v3.Tree, []v3.Diagnostic) {
	b.Helper()
	path := filepath.Join(testutil.TestdataDir(), "international_organization.txt")
	src, err := os.ReadFile(path)
	if err != nil {
		b.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		b.Fatal(err)
	}
	tree, diags := v3.Parse(path, src)
	return path, src, info, tree, diags
}

func BenchmarkCacheWriteDisk(b *testing.B) {
	path, src, info, tree, diags := benchmarkSrc(b)
	dir := b.TempDir()
	s, err := NewStore(dir, 0) // disk only
	if err != nil {
		b.Fatal(err)
	}
	b.SetBytes(int64(len(src)))
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		if err := s.Put(path, info, src, tree, diags); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkCacheReadDisk(b *testing.B) {
	path, src, info, tree, diags := benchmarkSrc(b)
	dir := b.TempDir()
	s, err := NewStore(dir, 0) // disk only, no L1
	if err != nil {
		b.Fatal(err)
	}
	if err := s.Put(path, info, src, tree, diags); err != nil {
		b.Fatal(err)
	}
	b.SetBytes(int64(len(src)))
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		if got, _, err := s.Get(path, info); err != nil || got == nil {
			b.Fatalf("cache miss: tree=%v err=%v", got, err)
		}
	}
}

func BenchmarkCacheReadL1(b *testing.B) {
	path, src, info, tree, diags := benchmarkSrc(b)
	dir := b.TempDir()
	s, err := NewStore(dir, 16)
	if err != nil {
		b.Fatal(err)
	}
	if err := s.Put(path, info, src, tree, diags); err != nil {
		b.Fatal(err)
	}
	b.SetBytes(int64(len(src)))
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		if got, _, err := s.Get(path, info); err != nil || got == nil {
			b.Fatalf("L1 miss: tree=%v err=%v", got, err)
		}
	}
}
