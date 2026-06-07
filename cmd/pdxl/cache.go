package main

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/spf13/cobra"
)

var cacheCmd = &cobra.Command{
	Use:   "cache",
	Short: "Inspect the parse cache",
}

var cacheSizeCmd = &cobra.Command{
	Use:   "size",
	Short: "Report the number of cached entries and total on-disk size",
	Args:  cobra.NoArgs,
	RunE:  runCacheSize,
}

var cacheClearCmd = &cobra.Command{
	Use:   "clear",
	Short: "Delete all cached parse entries",
	Args:  cobra.NoArgs,
	RunE:  runCacheClear,
}

var cacheSizeDetailed bool

func init() {
	cacheSizeCmd.Flags().BoolVar(&cacheSizeDetailed, "detailed", false, "break down by sub-cache (ast vs symbols)")
	cacheCmd.AddCommand(cacheSizeCmd)
	cacheCmd.AddCommand(cacheClearCmd)
	rootCmd.AddCommand(cacheCmd)
}

type cacheGroup struct {
	entries int
	size    int64
}

func runCacheSize(_ *cobra.Command, _ []string) error {
	dir := cfg.Cache.Dir

	var entries int
	var total int64
	groups := map[string]*cacheGroup{}
	err := filepath.WalkDir(dir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() || filepath.Ext(path) != ".bin" {
			return nil
		}
		info, err := d.Info()
		if err != nil {
			return err
		}
		entries++
		total += info.Size()
		if cacheSizeDetailed {
			name := groupOf(dir, path)
			g := groups[name]
			if g == nil {
				g = &cacheGroup{}
				groups[name] = g
			}
			g.entries++
			g.size += info.Size()
		}
		return nil
	})
	if os.IsNotExist(err) {
		fmt.Printf("%s: cache is empty (0 entries, 0 B)\n", dir)
		return nil
	}
	if err != nil {
		return err
	}

	fmt.Printf("%s\n", dir)
	if cacheSizeDetailed {
		names := make([]string, 0, len(groups))
		for n := range groups {
			names = append(names, n)
		}
		sort.Strings(names)
		for _, n := range names {
			g := groups[n]
			fmt.Printf("  %-10s %6d entries  %s\n", n, g.entries, humanBytes(g.size))
		}
	}
	fmt.Printf("entries  %d\n", entries)
	fmt.Printf("size     %s\n", humanBytes(total))
	return nil
}

// groupOf labels a cache entry by its first path segment under dir: entries
// directly in dir are the AST cache ("ast"); a subdirectory (e.g. "symbols")
// names its own sub-cache.
func groupOf(dir, path string) string {
	rel, err := filepath.Rel(dir, path)
	if err != nil {
		return "ast"
	}
	if i := strings.IndexRune(rel, filepath.Separator); i >= 0 {
		return rel[:i]
	}
	return "ast"
}

func runCacheClear(_ *cobra.Command, _ []string) error {
	dir := cfg.Cache.Dir

	var removed int
	var freed int64
	err := filepath.WalkDir(dir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() || filepath.Ext(path) != ".bin" {
			return nil
		}
		if info, err := d.Info(); err == nil {
			freed += info.Size()
		}
		if err := os.Remove(path); err != nil {
			return err
		}
		removed++
		return nil
	})
	if os.IsNotExist(err) {
		fmt.Printf("%s: cache is already empty\n", dir)
		return nil
	}
	if err != nil {
		return err
	}

	fmt.Printf("removed %d entries (%s freed)\n", removed, humanBytes(freed))
	return nil
}

// humanBytes formats a byte count in IEC units (KiB, MiB, ...).
func humanBytes(n int64) string {
	const unit = 1024
	if n < unit {
		return fmt.Sprintf("%d B", n)
	}
	div, exp := int64(unit), 0
	for m := n / unit; m >= unit; m /= unit {
		div *= unit
		exp++
	}
	return fmt.Sprintf("%.1f %ciB", float64(n)/float64(div), "KMGTPE"[exp])
}
