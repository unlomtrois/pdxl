# PDXL Design Guide

## Purpose

This document guides the development of **pdxl** — a Go-based toolkit for parsing and validating Paradox Interactive game scripting files. The goal is to learn from both **Tiger** (Rust, comprehensive but single-pass with no caching) and **GOCK3** (Go, incomplete but cleaner implementation), building a production-quality tool.

## Project Goals

| Goal | Rationale |
|------|----------|
| Comprehensive validation | Match Tiger's coverage |
| Incremental/cached | Fix Tiger's main flaw |
| Multi-game support | CK3, Vic3, EU5, Imperator, HOI4 |
| Extensible architecture | Easy to add new games/features |
| Editor tooling foundation | LSP, AI-assisted workflows |
| Performance at scale | Handle 50GB+ game directories |

## Architecture Summary

```
pdxl/
├── cmd/pdxl/           # CLI entry point
├── internal/
│   ├── lexer/        # Tokenization (done ✓, needs extension)
│   ├──parser/        # AST construction (planned)
│   ├──pdxfile/       # File orchestration
│   ├──files/         # File discovery & caching
│   └──config/        # Configuration
├── pkg/
│   ├── mod/          # Mod loading (.mod, .metadata)
│   ├── game/         # Game detection
│   ├── symbol/       # Symbol table (cached)
│   ├──validate/      # Validation rules
│   ├──lsp/          # LSP server (planned)
│   └──mcp/           # MCP server (planned)
└── data/             # Game tables (triggers, effects, etc.)
```

---

## Core Principles (Avoid Past Flaws)

### 1. Caching First — NEVER Reload From Disk Every Run

**Tiger's flaw**: Every run reloads 50GB+ of game files.

**Solution**: Implement persistent caching from day one.

```go
// Cache structure
type Cache struct {
    root     string                    // Cache directory
    files    map[string]FileCacheEntry // path → entry with mtime, checksum
    symbols  map[string]SymbolEntry   // cached symbols
}

type FileCacheEntry struct {
    Path      string
    MTime     time.Time
    Checksum [sha256.Size]byte
    AST      []byte  // serialized AST (msgpack/ron)
}

// On startup:
// 1. Load cache index
// 2. Check file mtimes/checksums
// 3. Invalidate changed files
// 4. Rebuild only invalidated entries

// Cache invalidation:
// - File modified → invalidate AST, rebuild
// - Dependency invalidates → invalidate dependent entries
// - Game version changes → full rebuild
```

### 2. Two-Pass Architecture — Separate Parsing From Validation

**GOCK3's flaw**: Mixed parsing with validation, incomplete validators.

**Solution**: Strict two-pass design.

```
Pass 1: PARSING (can be cached)
├── Scan files
├── Lex → Tokens
├── Tokens → AST (Block structure)
├── Store in cache (or load from cache)
└── NO cross-reference validation

Pass 2: VALIDATION (depends on Pass 1)
├── Load AST (from cache or rebuild)
├── Type-specific validation
├── Cross-reference checking
├── Scope verification
└── Report errors
```

### 3. Symbol Table — Built-in, Not Afterthought

**GOCK3's flaw**: Symbol table created but barely used.

**Solution**: First-class symbol infrastructure.

```go
type SymbolTable struct {
    mu      sync.RWMutex
    byKind  map[Kind]map[string]*Symbol  // Kind → Symbol
    byName  map[string]*Symbol         // name → Symbol (for fast lookup)
    deps    map[string][]string        // symbol → dependencies (for invalidation)
}

type Symbol struct {
    Kind     Kind
    Name     string
    File     string
    Location Location
    Block   *Block  // AST reference
    Defs    []string  // sub-definitions
    Refs    []string  // references to other symbols
}
```

### 4. Incremental Validation — Never Re-Validate Everything

**GOCK3's flaw**: Parses entire project on each run.

**Solution**: Track changes and validate only affected items.

```go
type Project struct {
    cache     *Cache
    symbols   *SymbolTable
    
    // Track dirty items between runs
    dirtySet  map[string]bool  // files that changed
    depGraph  DependencyGraph // file → files that depend on it
    
    // Methods
    func (p *Project) Load() error
    func (p *Project) MarkDirty(path string)
    func (p *Project) InvalidateDependents(path string)
    func (p *Project) ValidateModified() []Diagnostic
}
```

---

## Module Design

### Lexer (internal/lexer)

**Current state**: Working basic lexer.

**Improvements needed**:
- More token types (dates, etc.)
- Error recovery
- Position tracking (Line/Column)

```go
// Extended token tags needed
type Tag uint8

const (
    tag_invalid Tag = iota
    tag_identifier
    tag_literal_number
    tag_literal_string
    tag_literal_boolean  // done ✓
    tag_date          // YYYY.MM.DD format
    
    // Operators
    tag_equal
    tag_equal_equal
    tag_question_equal
    // ...
)
```

### Parser (internal/parser)

Design from scratch — needs to be robust.

```go
type Parser struct {
    lex         *lexer.Lexer
    current     *Token  // current token
    lookahead   *Token  // next token (lookahead)
    source     []byte // for value extraction
    errors     []Error
    recovery   RecoveryConfig
}

// AST structures
type Block struct {
    Tag    Token  // key that preceded this block
    Fields []*Field
    LOC    Location
}

type Field struct {
    Key      Token
    Operator Token  // =, +=, ?=, etc.
    Value    BV
    LOC      Location
}

type BV interface{}  // Token | *Block | []BV (for lists)

// Error recovery
type RecoveryConfig struct {
    SyncPoints []TokenType  // tokens to sync to
    MaxDepth   int
}
```

### File Management (internal/files)

Implement proper caching from day one.

```go
type FileSet struct {
    root       string
    files      []FileEntry
    fileIndex  map[string]int  // path → index
    
    // Caching support
    cacheDir   string
}

type FileEntry struct {
    Path     string  // relative path
    FullPath string  // absolute
    Kind    FileKind  // Vanilla, Mod, DLC, etc.
    Size    int64
    MTime   time.Time
    SHA256  [sha256.Size]byte
}

func (fs *FileSet) Scan(root string) error
func (fs *FileSet) GetCachedAST(path string) ([]byte, bool)  // returns cached
func (fs *FileSet) Invalidate(path string)
```

### Validation Framework (pkg/validate)

Build comprehensive rules from Tiger's implementation.

```go
// Validator interface
type Validator interface {
    Validate(block *Block, ctx *Context) []Diagnostic
}

// Game-specific validators
type CK3Validator struct{}
type Vic3Validator struct{}

// Rule types
type FieldRule struct {
    Name     string
    Required bool
    Type     ValueType
    Validator func(*Context) error
}

type ReferenceRule struct {
    From Kind
    To   Kind
    Validate func(ref string, ctx *Context) bool
}

// Scope validation (from Tiger)
type ScopeValidator struct {
    current Scope
    stack  []Scope  // scope chain
}

func (sv *ScopeValidator) push(scope Scope)
func (sv *ScopeValidator) pop() Scope
func (sv *ScopeValidator) validate(block *Block, ctx *Context) []Diagnostic
```

---

## Configuration System

**GOCK3's flaw**: Hardcoded paths.

**Solution**: Full configuration support.

```go
// pdxl.yaml
type Config struct {
    Version string `yaml:"version"`
    
    Game struct {
        Type    string `yaml:"type"`  // ck3, vic3, eu5, imperator, hoi4
        Path   string `yaml:"path"`
    } `yaml:"game"`
    
    Mod struct {
        Path string `yaml:"path"`
    } `yaml:"mod"`
    
    Cache struct {
        Dir    string `yaml:"dir"`     // default: .pdxl/cache
        TTL   time.Duration `yaml:"ttl"` // cache TTL
    } `yaml:"cache"`
    
    Validate struct {
        ShowVanilla bool `yaml:"show_vanilla"`
        ShowMods  bool `yaml:"show_mods"`
        Unused   bool `yaml:"unused"`
    } `yaml:"validate"`
    
    Output struct {
        Format string `yaml:"format"` // text, json
        Color  bool   `yaml:"color"`
    } `yaml:"output"`
}
```

---

## Errorreporting

Build proper reporting upfront.

```go
type Diagnostic struct {
    Severity Severity
    Code     string     // "UNKNOWN_KEY", "SCOPE_MISMATCH", etc.
    Message  string
    Location Location
    Source  *Token
    
    // Fix suggestions
    Suggestions []Suggestion
    Docs      string  // link to documentation
}

type Severity uint8

const (
    SeverityError   Severity = iota
    SeverityWarning
    SeverityInfo
    SeverityHint
)

// Formatters
func (d Diagnostic) FormatText() string
func (d Diagnostic) FormatJSON() string
func (d Diagnostic) FormatVSCode() LSPDiagnostic  // for LSP
```

---

## Testing Strategy

### Test Structure

```go
// tests/
// ├── fixtures/
// │   ├── lexer/
// │   │   ├── valid_tokens.txt
// │   │   └── invalid_tokens.txt
// │   ├── parser/
// │   │   ├── good_blocks.txt
// │   │   └── bad_blocks.txt
// │   ├── validate/
// │   │   ├── valid_events/
// │   │   └── invalid_events/
// │   └── mod/
// │       └── basic_mod/
// └── integration/
//     ├── test_parse.go
//     └── test_validate.go
```

### Benchmarking

```go
func BenchmarkLexer(b *testing.B)
func BenchmarkParser(b *testing.B)
func BenchmarkCache(b *testing.B)
func BenchmarkIncremental(b *testing.B)
```

---

## Performance Considerations

### 1. Parallel File Processing

```go
func (fs *FileSet) ScanParallel() {
    workers := runtime.GOMAXPROCS(0)
    tasks := make(chan FileEntry, workers)
    
    var wg sync.WaitGroup
    for i := 0; i < workers; i++ {
        wg.Add(1)
        go func() {
            defer wg.Done()
            for entry := range tasks {
                parseFile(entry)
            }
        }()
    }
    // ... send tasks
}
```

### 2. Memory Efficiency

- Use `sync.Pool` for parsing buffers
- Pre-allocate slices with capacity hints
- Reuse token/symbol objects via pool

### 3. Caching Strategy

- **Level 1**: In-memory LRU cache (recently parsed)
- **Level 2**: Disk cache (msgpack/ron serialization)
- **TTL**: Configurable, default 24h
- **Invalidation**: File mtime + content hash

---

## Multi-Game Support

### Game Detection

```go
type GameType uint8

const (
    GameUnknown GameType = iota
    GameCK3
    GameVic3
    GameEU5
    GameImperator
    GameHOI4
)

func DetectGame(path string) (GameType, error) {
    // Check signature files
    signatures := map[GameType]string{
        GameCK3:      "game/events/witch_events.txt",
        GameVic3:     "game/events/titanic_events.txt",
        // ...
    }
    // ...
}
```

### Game Tables

Structure similar to Tiger's `tiger-tables`:

```
data/
├── ck3/
│   ├── triggers.yaml
│   ├── effects.yaml
│   ├── defines.yaml
│   └── scopes.yaml
├── vic3/
│   └── ...
```

---

## Roadmap

### Phase 1: Foundation
- [x] Lexer with positions ✓
- [ ] Parser with error recovery
- [ ] Basic caching layer
- [ ] Integration tests

### Phase 2: Core Validation
- [ ] Symbol table with caching
- [ ] Field validation
- [ ] Reference validation
- [ ] Scope validation

### Phase 3: Multi-Game
- [ ] CK3 support
- [ ] Vic3 support
- [ ] Game tables loader
- [ ] Cross-file validation

### Phase 4: Tooling
- [ ] LSP server
- [ ] VS Code extension
- [ ] MCP server
- [ ] AI-assisted workflows

---

## Command-Line Interface

```bash
# Basic usage
pdxl parse <file>           # Parse a single file → tokens
pdxl validate <mod>         # Validate a mod
pdxl cache clean            # Clear cache
pdxl cache stats            # Show cache stats

# Options
pdxl validate --game /path   # Game directory
pdxl validate --mod /path    # Mod to validate
pdxl validate --vanilla     # Show vanilla errors
pdxl validate --json       # JSON output

# Cache
pdxl validate --no-cache    # Skip cache
pdxl validate --rebuild    # Force rebuild
```

---

## Key Takeaways

| Principle | Implementation |
|-----------|---------------|
| **Cache first** | Persistent cache with mtime/checksum invalidation |
| **Two-pass** | Parse → Validate, strictly separated |
| **Incremental** | Track dirty files, only validate changed + dependents |
| **Extensible** | Plugin-based validators per game |
| **Tested** | Benchmarks from day one |
| **Documented** | Tables as YAML/JSON (editable) |

## Avoided Flaws

| From Tiger | Avoided By |
|-----------|-----------|
| No caching | Persistent disk cache |
| Single-pass | Incremental validation |
| Monolithic | Modular packages |
| Rust-specific | Go (cross-platform, familiar) |

| From GOCK3 | Avoided By |
|-----------|-----------|
| Incomplete validators | Tiger-accurate rules |
| No caching | From-scratch caching |
| Hardcoded paths | Config system |
| Poor error handling | Structured diagnostics |
| No benchmarks | Benchmarking first |