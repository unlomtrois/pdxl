# pdxl Design Guide (Primary Source Edition)

This guide is derived from direct analysis of two prior implementations:
- **GOCK3** (`unlomtrois/gock3`) — Go, CK3-only; has a parser and symbol table skeleton but never completed validation.
- **Tiger** (`amtep/tiger`) — Rust, multi-game; comprehensive validation but reloads 50 GB+ from disk on every run.

---

## 1. Purpose

pdxl is a Go toolkit for parsing and validating Paradox Interactive scripting files (CK3, Victoria 3, EU5, Imperator, HOI4). The intended end state:

- Structural parser covering the full PDXScript grammar
- Cross-file validation with scope checking, matching Tiger's coverage
- Persistent caching so incremental runs are fast (not Tiger's flaw)
- Clean public API suitable for LSP and MCP servers (future)

---

## 2. Lessons from Prior Art

### GOCK3 — what worked

- Two-token lookahead parser with explicit recovery sync points
- AST design: `FileBlock → []Field → (Key, Operator, BV)` — correct shape
- Symbol table keyed by `(Kind, Name)` — right abstraction
- Two CLI commands (`parse`, `project`) — clear separation of file-level vs project-level work

### GOCK3 — what failed

| Flaw | Root cause | pdxl fix |
|------|-----------|----------|
| `?=` vs `=` token collision | Regex-based lexer with priority order; `^==?` matched both | Hand-rolled switch lexer (already done in pdxl) |
| Date parsing ambiguous | Regex `^-?\d+\.\d{1,2}\.(\d{1,2})?` overlaps with float | Dedicated `tag_date` state in lexer |
| Singleton `PathTable` | `sync.Once` global; untestable without `resetPathTable()` hack | Pass dependencies explicitly; no package-level state |
| Debug panic in production | `panic(path)` in `expression.go:51` | Panic only for programmer errors; return `error` at boundaries |
| Debug print in ErrorManager | `runtime.Caller` printed on every `AddError()` | Diagnostics are data; printing is the caller's job |
| No validators | `BlockValidator` defined but no rules written | Build validation rules alongside the item they validate |
| No caching | Full re-parse every run | Persistent cache from Phase 1 |
| Hardcoded paths | `"game/common"` literal in loader | Config file; no hardcoded paths |

### Tiger — what worked

- **Two-phase architecture**: Phase 1 (parse → load → build databases) is parallel and produces cacheable output. Phase 2 (validate) is read-only against the frozen database.
- **Item registry**: `inventory::submit!(ItemLoader { ... })` in each module's `init`. Go equivalent: package `init()` appending to a global `[]ItemLoader` slice.
- **Scope system as bitflags**: `type Scopes uint64` with named bit constants. A trigger's valid scope is a bitmask; the validator ANDs it against the current scope stack.
- **Load order model**: `Internal < Clausewitz < Jomini < Vanilla < DLC < LoadedMods < Mod`. Later entries override earlier ones of the same path. This is the mod overlay model.
- **Confidence levels on diagnostics**: Some rules fire on undocumented Paradox syntax. A `Confidence` field (`High`/`Uncertain`) lets users filter noise.
- **Game tables as data**: Triggers, effects, and defines live in `tiger-tables/` as generated Rust, editable per game version. Go equivalent: YAML files under `data/<game>/`.

### Tiger — what failed

| Flaw | Impact |
|------|--------|
| No caching | Re-reads all vanilla files (50 GB+) every run; 60–80% of wall time is I/O |
| No incremental validation | One-line script change → full revalidation |
| String duplication | Lowercase versions re-created per run; no string interning |
| Incomplete games | HOI4 has `// TODO HOI4` stubs; different behavior across games |
| Sequential directory scan | `walkdir` walks serially before parallel item load |

---

## 3. PDXScript Language Reference

### Grammar (informal)

```
file     = field*
field    = key op value
op       = "=" | "?=" | "==" | "!=" | "<" | "<=" | ">" | ">="
value    = token | block
block    = "{" field* "}"
token    = identifier | number | boolean | date | string
```

### Types

| Type | Examples | Notes |
|------|---------|-------|
| Identifier | `brave`, `k_france`, `8_character` | May start with digit; `&` and `'` valid mid-token |
| Integer | `42`, `-7` | |
| Float | `3.14`, `0.1` | Decimal point + digit required |
| Boolean | `yes`, `no` | Keywords, not identifiers |
| Date | `1066.1.1`, `867.9.15` | `YYYY.M.D`; third component optional in some games |
| String | `"John Smith"` | No escape sequences in vanilla; avoid assuming them |
| Scripted var | `$VARIABLE$` | Dollar-delimited; used in templates |
| Scope chain | `scope:character.liege.capital` | Colon separates scope type from path |
| Local var | `@my_value` | At-sign; references scripted local variable |

### Operators

`=` assignment, `?=` assign-if-unset, `==` equality check, `!=` not-equal, `<` `<=` `>` `>=` numeric comparison. Note: `+=` and `-=` exist in some games (effects).

### File encoding

| Encoding | Marker | Games |
|---------|--------|-------|
| UTF-8 with BOM | `\xEF\xBB\xBF` | CK3, Vic3, EU5 — pdxl already handles this |
| UTF-16LE with BOM | `\xFF\xFE` | Some older DLC files |
| Windows-1252 | (none) | EU4, older Imperator files |

pdxl currently handles UTF-8 BOM. UTF-16LE and cp1252 support is needed for full game coverage.

### Comments

`#` to end of line. No block comments.

---

## 4. Architecture

```
pdxl/
├── cmd/pdxl/           — CLI (cobra; one file per subcommand)
├── internal/
│   ├── lexer/          — tokenization (done; needs date token, line/col)
│   ├── parser/         — AST construction (planned)
│   ├── files/          — file discovery, overlay model, caching
│   └── config/         — pdxl.yaml loading
├── pkg/
│   ├── mod/            — .mod / .metadata parsing
│   ├── game/           — game detection
│   ├── db/             — generic item database
│   ├── symbol/         — symbol table
│   ├── validate/       — validation rules and scope engine
│   ├── lsp/            — LSP server (Phase 4 only)
│   └── mcp/            — MCP server (Phase 4 only)
└── data/
    ├── ck3/            — triggers.yaml, effects.yaml, scopes.yaml
    ├── vic3/
    └── ...
```

### Two-pass design

```
Pass 1 — PARSING (output is cacheable)
  scan files → apply overlay → lex → parse → AST → store in cache

Pass 2 — VALIDATION (reads frozen database; not cacheable)
  load ASTs from cache → build symbol table → validate fields
  → check cross-references → check scope transitions → emit diagnostics
```

Pass 1 output (serialized ASTs) can be stored on disk and reused across runs. Pass 2 always re-runs, but against cached data it is fast (no I/O).

---

## 5. Lexer

**Current state** (done):
- Byte-offset tokens (`Start`/`End`); `GetValue(source)` slices correctly for UTF-8
- All operators, delimiters, and scope tokens
- `yes`/`no` keyword detection
- UTF-8 BOM skip in `Init()`
- Digit-leading identifier (`8_char`) and decimal number (`0.1`)

**Needed** (Phase 1 extension):
- `tag_date` — `YYYY.M.D` format; distinguish from float during `lexNumber` by checking for a second `.`
- Line/column tracking — add `line int` and `lineStart int` fields to `Lexer`; increment on `\n` in `skipWhitespace`; compute column as `pos - lineStart` at token start
- `tag_scripted_var` — `$...$` delimited token
- UTF-16LE / cp1252 decoding — normalize to `[]byte` (UTF-8) in `Init()` before lexing

---

## 6. Parser

Design based on Tiger's `BV`/`Block`/`Field` model, adapted to Go.

### AST types

```go
// BV is a block value: either a single token or a nested block.
// Use a struct with a kind discriminator, not interface{}.
type BVKind uint8

const (
    BVToken BVKind = iota
    BVBlock
)

type BV struct {
    Kind  BVKind
    Token Token  // valid when Kind == BVToken
    Block *Block // valid when Kind == BVBlock
}

type Block struct {
    Fields []Field
    Start  int // byte offset of opening brace
    End    int // byte offset after closing brace
}

type Field struct {
    Key      Token
    Operator Token
    Value    BV
}
```

**Why not `interface{}`**: Pattern-matching on `interface{}` requires type assertions with no compiler help. A `BVKind` discriminator is checked at compile time and allocates no heap for the `Token` variant.

### Parser struct

```go
type Parser struct {
    lex      *lexer.Lexer
    source   []byte
    current  lexer.Token
    next     lexer.Token // one-token lookahead
    errors   []Diagnostic
}
```

### Error recovery

Paradox files are block-structured. Recovery sync points:
- `r_brace` — end of current block; skip to here on field-level error
- `identifier` at column 0 — top-level key; skip to here on block-level error

This matches GOCK3's `RecoveryPoint` design. Do not panic; always produce a partial AST.

---

## 7. File Management and Mod Overlay

### Load order

Derived from Tiger's `Fileset`. Later entries override earlier ones of the same relative path:

```
1. Internal (pdxl built-ins, if any)
2. Vanilla game files
3. DLC files
4. Dependency mods (in declared order)
5. Target mod
```

```go
type FileKind uint8

const (
    FileKindVanilla FileKind = iota
    FileKindDLC
    FileKindDependency
    FileKindMod
)

type FileEntry struct {
    RelPath  string   // path relative to game/mod root; used as overlay key
    FullPath string
    Kind     FileKind
    MTime    time.Time
    SHA256   [32]byte // filled lazily on cache miss
}

type FileSet struct {
    // ordered: later entries shadow earlier ones with same RelPath
    entries  []FileEntry
    byPath   map[string]int // RelPath → index of winning entry
}
```

`byPath` is built after all sources are added, by iterating `entries` in order and overwriting on collision. This matches Tiger's "later overrides earlier" semantics.

---

## 8. Caching

### Strategy

- **Invalidation**: mtime first (fast); if mtime matches, SHA-256 confirms (avoids clock skew false positives). No TTL — TTL introduces stale reads after patch installs.
- **Serialization**: `encoding/gob` for AST. Fast, zero dependencies, handles Go structs natively. Use `vmihailenco/msgpack` if cross-language interop is ever needed.
- **Location**: `.pdxl/cache/` in the project root (gitignored).
- **Dependency graph**: When file A defines a symbol referenced by file B, changing A invalidates B's validation result (but not B's AST). Store `deps map[string][]string` alongside the cache index.

```go
type CacheEntry struct {
    RelPath  string
    MTime    time.Time
    SHA256   [32]byte
    ASTBytes []byte // gob-encoded Block
}

type CacheIndex struct {
    Entries map[string]CacheEntry // RelPath → entry
    Deps    map[string][]string   // RelPath → files that depend on it
}
```

### Cache miss path

1. Read file
2. Lex → Parse → `Block`
3. `gob.Encode(block)` → write to `.pdxl/cache/<hash>.bin`
4. Update index entry with new mtime and SHA-256

---

## 9. Item Registry

Tiger uses Rust's `inventory` crate for compile-time loader registration. In Go, use `init()` functions:

```go
// pkg/db/registry.go
var loaders []ItemLoader

func Register(l ItemLoader) {
    loaders = append(loaders, l)
}

type ItemLoader struct {
    Path      string   // relative path under game root, e.g. "common/traits"
    Extension string   // e.g. ".txt"
    Recursive bool
    ForGame   func(GameType) bool
    Add       func(db *DB, key Token, block *Block)
}
```

Each item package registers itself:

```go
// pkg/validate/traits/loader.go
func init() {
    db.Register(db.ItemLoader{
        Path:      "common/traits",
        Extension: ".txt",
        Recursive: true,
        ForGame:   func(g db.GameType) bool { return g == db.GameCK3 },
        Add:       addTrait,
    })
}
```

This avoids a monolithic switch in the loader and mirrors Tiger's extensibility model.

---

## 10. Symbol Table and Validation

### Symbol table

```go
type Kind uint16 // trait, character, culture, religion, event, …

type Symbol struct {
    Kind     Kind
    Name     string
    File     string
    Location Location
    Block    *Block
}

type SymbolTable struct {
    mu     sync.RWMutex
    byKind map[Kind]map[string]*Symbol
}
```

### Scope system

Tiger uses Rust `bitflags!`. Go equivalent:

```go
type Scopes uint64

const (
    ScopeCharacter Scopes = 1 << iota
    ScopeProvince
    ScopeTitle
    ScopeCulture
    ScopeReligion
    // …
    ScopeNone  Scopes = 0
    ScopeAny   Scopes = ^Scopes(0)
)

// A trigger is valid if (currentScope & triggerScopes) != 0
func (sc Scopes) Allows(required Scopes) bool {
    return sc&required != 0
}
```

### Diagnostics

```go
type Confidence uint8

const (
    ConfidenceHigh      Confidence = iota // rule is well-established
    ConfidenceUncertain                   // Paradox syntax is undocumented here
)

type Diagnostic struct {
    Severity   Severity
    Confidence Confidence
    Code       string     // "UNKNOWN_KEY", "SCOPE_MISMATCH", …
    Message    string
    Location   Location
    Token      *Token
    Suggestion string
}
```

`Confidence` lets users suppress uncertain warnings without losing high-confidence errors.

### Validation passes

1. **Load-time** (during Pass 1): syntax errors, unterminated strings, unknown tokens — emit immediately.
2. **Per-item** (Pass 2, per database entry): required fields, valid field values, type mismatches.
3. **Cross-reference** (Pass 2, after all items loaded): trait X referenced but not defined, event ID Y not found.
4. **Scope** (Pass 2, per trigger/effect block): scope transitions, mismatched scope types.

---

## 11. Configuration

```yaml
# pdxl.yaml
version: 1

game:
  type: ck3          # ck3 | vic3 | eu5 | imperator | hoi4
  path: /path/to/game

mod:
  path: /path/to/mod

cache:
  dir: .pdxl/cache   # default; relative to project root

validate:
  show_vanilla: false
  show_mods: false
  unused: false
  min_confidence: high  # high | uncertain

output:
  format: text    # text | json
  color: true
```

---

## 12. CLI

```bash
pdxl lex <file>              # print tokens (dev tool)
pdxl parse <file>            # parse file, print AST or errors
pdxl validate <mod>          # validate mod against game files
pdxl cache stats             # show cache hit/miss counts and size
pdxl cache clean             # delete .pdxl/cache/

# Flags for validate
--game /path     # game directory
--mod /path      # mod to validate (overrides config)
--show-vanilla   # include vanilla file errors
--json           # JSON output (for editor integration)
--no-cache       # skip cache read/write
--rebuild        # force cache rebuild
```

---

## 13. Testing

### Structure

```
internal/lexer/lexer_test.go      — existing; keep testTokenize() helper
internal/parser/parser_test.go    — use fixture files
tests/fixtures/
  lexer/valid.txt, invalid.txt
  parser/good_blocks.txt, bad_blocks.txt, recovery_cases.txt
  validate/ck3/valid_events/, invalid_events/
```

### Fixture-driven parser tests

```go
// Parse a fixture file; compare AST or error list against golden file.
// Update goldens with -update flag.
func TestParserFixtures(t *testing.T) { ... }
```

### Benchmarks (add when each component exists)

```go
func BenchmarkLexer(b *testing.B)       // baseline: tokens/sec
func BenchmarkParser(b *testing.B)      // baseline: files/sec
func BenchmarkCacheRead(b *testing.B)   // gob decode time
func BenchmarkCacheWrite(b *testing.B)  // gob encode + write time
```

Run benchmarks with `go test -bench=. -benchmem ./...` — do not add them before the component exists.

---

## 14. Roadmap

### Phase 1 — Foundation
- [ ] Lexer: add `tag_date`, line/col tracking, `tag_scripted_var`
- [ ] Parser: `Block`/`BV`/`Field` AST, two-token lookahead, error recovery
- [ ] File management: `FileSet`, `FileKind`, overlay model
- [ ] Cache: `CacheIndex`, gob serialization, mtime+checksum invalidation
- [ ] Integration tests: fixture-driven parse tests

### Phase 2 — Core Validation
- [ ] `SymbolTable` with `Kind` index
- [ ] Item registry (`db.Register`, `init()`-based)
- [ ] Field validation (required/optional/type)
- [ ] Cross-reference validation
- [ ] Scope validation (bitflag engine)
- [ ] CK3 game tables (`data/ck3/*.yaml`)

### Phase 3 — Multi-Game
- [ ] Vic3 support
- [ ] EU5 support
- [ ] Game detection (multiple signature files, not one)
- [ ] UTF-16LE and cp1252 encoding support
- [ ] Dependency mod loading

### Phase 4 — Tooling
- [ ] LSP server (`pkg/lsp/`) — only after Phase 2 is solid
- [ ] MCP server (`pkg/mcp/`)
- [ ] VS Code extension

---

## 15. Anti-Patterns to Avoid

These are drawn directly from GOCK3 and Tiger failures.

| Anti-pattern | Why | What to do instead |
|-------------|-----|--------------------|
| Regex-based lexer | Token priority order causes `?=`/`=` collisions; date/float ambiguity | Hand-rolled switch (already done) |
| `interface{}` / `any` for BV | Requires runtime type assertions; no compiler help on exhaustive matching | `BV` struct with `Kind` discriminator |
| Package-level singletons | Makes testing impossible without `reset*()` hacks | Pass dependencies explicitly |
| `runtime.Caller` in error path | Leaks implementation details to users; slow | Diagnostics are data; collect and format separately |
| `panic` for expected errors | Crashes on malformed mod files (user input) | Return `error`; panic only for programmer invariant violations |
| TTL-based cache expiry | Stale reads after game patch installs | mtime + SHA-256 invalidation only |
| RON for AST serialization | RON is Rust-specific; no Go support | `encoding/gob` (or msgpack if cross-language needed) |
| Single-file game detection | `witch_events.txt` can be renamed in patches | Check multiple signature files; prefer launcher metadata |
| LSP/MCP before parser works | Tooling built on an incomplete parser is unusable | Phase 4 only; parser and validator must be solid first |
| Hardcoded game paths | Breaks on any non-default install | Config file only; no literals in source |
| Mixing print with validation | `pdxfile.finalize()` printing errors mid-pipeline | Collect diagnostics; print once at the top level |
