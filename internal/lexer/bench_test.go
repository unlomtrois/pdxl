package lexer

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func testdataDir() string {
	_, file, _, _ := runtime.Caller(0)
	root := filepath.Join(filepath.Dir(file), "..", "..")
	abs, _ := filepath.Abs(root)
	return filepath.Join(abs, "testdata")
}

// BenchmarkLexFixtures measures lex throughput for each fixture file.
// Run with: go test ./internal/lexer/... -bench=. -benchmem
func BenchmarkLexFixtures(b *testing.B) {
	fixtures, err := filepath.Glob(filepath.Join(testdataDir(), "*.txt"))
	if err != nil {
		b.Fatal(err)
	}
	for _, fixturePath := range fixtures {
		src, err := os.ReadFile(fixturePath)
		if err != nil {
			b.Fatal(err)
		}
		name := filepath.Base(fixturePath)
		b.Run(name, func(b *testing.B) {
			b.SetBytes(int64(len(src)))
			b.ReportAllocs()
			b.ResetTimer()
			for b.Loop() {
				l := Init(src)
				for l.Next() != nil {
				}
			}
		})
	}
}

// BenchmarkLexLarge is a stable single-file baseline on the largest fixture.
func BenchmarkLexLarge(b *testing.B) {
	src, err := os.ReadFile(filepath.Join(testdataDir(), "international_organization.txt"))
	if err != nil {
		b.Fatal(err)
	}
	b.SetBytes(int64(len(src)))
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		l := Init(src)
		for l.Next() != nil {
		}
	}
}
