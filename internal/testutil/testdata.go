// Package testutil provides shared test helpers for pdxl packages.
package testutil

import (
	"os"
	"path/filepath"
	"runtime"
)

// TestdataDir returns the absolute path to the project-level testdata/
// directory. Safe to call from any package under internal/.
func TestdataDir() string {
	_, file, _, _ := runtime.Caller(0)
	// file = .../internal/testutil/testdata.go — go up two levels to module root
	root := filepath.Join(filepath.Dir(file), "..", "..")
	abs, err := filepath.Abs(root)
	if err != nil {
		panic("testutil: could not resolve testdata path: " + err.Error())
	}
	td := filepath.Join(abs, "testdata")
	if _, err := os.Stat(td); err != nil {
		panic("testutil: testdata dir not found at " + td)
	}
	return td
}
