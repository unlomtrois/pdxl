package v3

import (
	"os"
	"path/filepath"
	"testing"

	"pdxl/internal/testutil"
)

// BenchmarkParseFixtures measures parse throughput for each fixture file.
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
				if _, diags := Parse(fixturePath, src); len(diags) > 0 {
					b.Fatalf("unexpected diagnostics: %v", diags)
				}
			}
		})
	}
}

// BenchmarkParseLarge is a stable single-file baseline on the largest fixture.
func BenchmarkParseLarge(b *testing.B) {
	src, err := os.ReadFile(filepath.Join(testutil.TestdataDir(), "international_organization.txt"))
	if err != nil {
		b.Fatal(err)
	}
	b.SetBytes(int64(len(src)))
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		if _, diags := Parse("international_organization.txt", src); len(diags) > 0 {
			b.Fatalf("unexpected diagnostics: %v", diags)
		}
	}
}
