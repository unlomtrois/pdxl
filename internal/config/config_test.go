package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestDefault(t *testing.T) {
	cfg := Default()
	if !cfg.Cache.Enabled {
		t.Error("expected cache enabled by default")
	}
	if cfg.Cache.Dir != ".pdxl/cache" {
		t.Errorf("unexpected default cache dir: %q", cfg.Cache.Dir)
	}
	if cfg.Cache.LRUCap != 256 {
		t.Errorf("unexpected default lru_cap: %d", cfg.Cache.LRUCap)
	}
	if cfg.Game != "" {
		t.Errorf("expected empty game by default, got %q", cfg.Game)
	}
}

func TestLoadMissing(t *testing.T) {
	cfg, err := Load(filepath.Join(t.TempDir(), "nonexistent.toml"))
	if err != nil {
		t.Fatalf("unexpected error for missing file: %v", err)
	}
	if cfg.Cache.LRUCap != 256 {
		t.Errorf("expected defaults, got lru_cap=%d", cfg.Cache.LRUCap)
	}
}

func TestLoadValid(t *testing.T) {
	f := filepath.Join(t.TempDir(), "config.toml")
	err := os.WriteFile(f, []byte(`
game = "ck3"

[cache]
enabled = false
dir     = "/tmp/mycache"
lru_cap = 64

[lint]
context = 5
`), 0o644)
	if err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(f)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if cfg.Game != "ck3" {
		t.Errorf("game: got %q, want ck3", cfg.Game)
	}
	if cfg.Cache.Enabled {
		t.Error("expected cache disabled")
	}
	if cfg.Cache.Dir != "/tmp/mycache" {
		t.Errorf("cache dir: got %q", cfg.Cache.Dir)
	}
	if cfg.Cache.LRUCap != 64 {
		t.Errorf("lru_cap: got %d, want 64", cfg.Cache.LRUCap)
	}
	if cfg.Lint.Context != 5 {
		t.Errorf("context: got %d, want 5", cfg.Lint.Context)
	}
}

func TestLoadPartial(t *testing.T) {
	f := filepath.Join(t.TempDir(), "config.toml")
	err := os.WriteFile(f, []byte(`game = "ck3"`), 0o644)
	if err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(f)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if cfg.Game != "ck3" {
		t.Errorf("game: got %q", cfg.Game)
	}
	// other fields should remain at defaults
	if !cfg.Cache.Enabled {
		t.Error("expected cache still enabled")
	}
	if cfg.Cache.LRUCap != 256 {
		t.Errorf("lru_cap should be default 256, got %d", cfg.Cache.LRUCap)
	}
}
