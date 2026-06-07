package main

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"

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

func init() {
	cacheCmd.AddCommand(cacheSizeCmd)
	rootCmd.AddCommand(cacheCmd)
}

func runCacheSize(_ *cobra.Command, _ []string) error {
	dir := cfg.Cache.Dir

	var entries int
	var total int64
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
	fmt.Printf("entries  %d\n", entries)
	fmt.Printf("size     %s\n", humanBytes(total))
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
