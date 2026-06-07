// Package config loads per-project pdxl configuration from pdxl.toml.
package config

import (
	"bytes"
	"os"

	"github.com/BurntSushi/toml"
)

const DefaultPath = "pdxl.toml"

// Config is the top-level configuration structure.
type Config struct {
	Game     string      `toml:"game"`
	GamePath string      `toml:"game_path"`
	ModPath  string      `toml:"mod_path"`
	Cache    CacheConfig `toml:"cache"`
	Lint     LintConfig  `toml:"lint"`
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

// Write encodes cfg as TOML and writes it to path.
func Write(path string, cfg Config) error {
	var buf bytes.Buffer
	if err := toml.NewEncoder(&buf).Encode(cfg); err != nil {
		return err
	}
	return os.WriteFile(path, buf.Bytes(), 0o644)
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
