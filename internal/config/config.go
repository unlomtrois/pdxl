// Package config loads per-project pdxl configuration from .pdxl/config.toml.
package config

import (
	"os"

	"github.com/BurntSushi/toml"
)

const DefaultPath = ".pdxl/config.toml"

// Config is the top-level configuration structure.
type Config struct {
	Game  string      `toml:"game"`
	Cache CacheConfig `toml:"cache"`
	Lint  LintConfig  `toml:"lint"`
}

// CacheConfig controls the two-level parse cache.
type CacheConfig struct {
	Enabled bool   `toml:"enabled"`
	Dir     string `toml:"dir"`
	LRUCap  int    `toml:"lru_cap"`
}

// LintConfig holds lint-command defaults.
type LintConfig struct {
	Context int `toml:"context"`
}

// Default returns a Config with sensible out-of-the-box values.
func Default() Config {
	return Config{
		Cache: CacheConfig{
			Enabled: true,
			Dir:     ".pdxl/cache",
			LRUCap:  256,
		},
	}
}

// Load reads the TOML file at path, starting from Default().
// A missing file is not an error — defaults are returned unchanged.
func Load(path string) (Config, error) {
	cfg := Default()
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return cfg, nil
	}
	_, err := toml.DecodeFile(path, &cfg)
	return cfg, err
}
