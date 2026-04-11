package parser

import (
	"os"
	"path/filepath"
	"testing"

	"pdxl/internal/testutil"
)

// BenchmarkParseFixtures measures parse throughput for each fixture file.
// Run with: go test ./internal/parser/... -bench=. -benchmem
func BenchmarkParseFixtures(b *testing.B) {
	td := testutil.TestdataDir()
	fixtures, err := filepath.Glob(filepath.Join(td, "*.txt"))
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
				if _, err := ParseBytes(fixturePath, src); err != nil {
					b.Fatal(err)
				}
			}
		})
	}
}

// BenchmarkParseLarge measures parse throughput on the largest fixture
// (international_organization.txt) as a stable single-file baseline.
func BenchmarkParseLarge(b *testing.B) {
	src, err := os.ReadFile(filepath.Join(testutil.TestdataDir(), "international_organization.txt"))
	if err != nil {
		b.Fatal(err)
	}
	b.SetBytes(int64(len(src)))
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		if _, err := ParseBytes("international_organization.txt", src); err != nil {
			b.Fatal(err)
		}
	}
}
