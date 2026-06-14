# pdxl Architecture

This document is the in-depth companion to the architecture summary in
[`CLAUDE.md`](../CLAUDE.md). Read CLAUDE.md for a terse map; read this to rebuild a
mental model of **how a request flows through the system**, **the key data
structures**, and **the non-obvious invariants** that aren't visible from any single
file.

It is written from the current code (verified against `internal/lexer`,
`internal/parser/v3`, `internal/cache`, `internal/files`, `internal/validate`,
`internal/lsp`, and `cmd/pdxl`). If you change a layer, update the relevant section.

---

## 1. Overview

**pdxl** is a toolkit for parsing Paradox Interactive scripting files (PDXScript —
used by CK3, EU5, Victoria 3, etc.). The grammar is identical across games; only the
*semantics* (what names mean, which keys reference what) differ. pdxl currently
targets CK3.

It ships as a single binary with two faces:

- a **CLI** (`pdxl lex|parse|lint|index|check|cache|watch|lsp …`), and
- a **language server** (`pdxl lsp`) that the VS Code extension drives over stdio.

**Strategic scope — editor DX, not validation depth.** Deep CK3 validation is
already well covered by [ck3-tiger](https://github.com/amtep/ck3-tiger). pdxl's
differentiation is the *editor integration* (go-to-definition, live diagnostics, and
— in future — hover/completion/references), which Tiger does not provide. Validation
here stays deliberately lightweight: just enough cross-file reference resolution to
power editor features, not a goal in itself. Be cautious about expanding the CK3
schema (`internal/validate/schema_ck3.go`) purely to catch more errors — that's
Tiger's territory.

The core idea is a **layered pipeline** from raw bytes up to editor features, with a
**cache** and a **file-overlay resolver** sitting beside it.

---

## 2. The pipeline

```
            ┌─────────────────────────── internal/files (FileSet) ───────────────────────────┐
            │  scan vanilla + mod roots in load order → winning .txt entries (mod shadows      │
            │  vanilla); ParseMod/.mod + Proton path resolution                                │
            └───────────────────────────────────────────────────────────────────────────────┘
                                              │  (per winning file)
                                              ▼
   bytes ──▶ lexer.Tokenize ──▶ []Token ──▶ v3.Parse ──▶ *Tree (flat node pool) + []Diagnostic
                                              │                       ▲
                                              │                       │ cache.Store (L1 LRU + L2 disk gob)
                                              │                       │ Get/Put keyed by mtime → SHA-256
                                              ▼
                              validate.extractFacts (single AST walk)
                                              │
                                              ▼
                                    FileFacts{Defs, Aliases, Refs}      ◀── FactStore (per-file disk cache)
                                              │
                                              ▼
                          validate.mergeAndResolve (all files' facts)
                                              │
                                ┌─────────────┴─────────────┐
                                ▼                           ▼
                         SymbolTable                    []RefDiag
                    (byKind[Kind][Name]Symbol)      (unresolved refs)
                                │                           │
        ┌───────────────────────┴───────────┐               │
        ▼                                   ▼               ▼
  go-to-definition                    CLI: check / watch    live diagnostics
  (LSP, F12)                          (one-shot / HTTP)      (LSP, mod-scoped)
```

The `validate.Project` type wraps everything below `extractFacts` and makes it
**incremental**: editing one file re-parses only that file, then rebuilds the table
from in-memory facts. The LSP server owns exactly one `Project`.

---

## 3. Layers

### 3.1 Lexer (`internal/lexer`)

A hand-written byte-offset tokenizer. No string copies — tokens are spans into the
source.

```go
type Lexer struct { source []byte; pos int }   // pos is a byte offset, not a rune index

type Token struct {
    Start int   // byte offset, inclusive
    End   int   // byte offset, exclusive  →  source[Start:End] is the literal
    Tag   Tag
}
```

- **Offsets are 0-indexed and half-open** (`[Start, End)`), used directly for
  slicing: `Token.GetValue(src) == src[Start:End]`. Line/column numbers are produced
  only for *display* (`FormatPosition` → `path:line:col`) and are **1-indexed**.
- `advance()` decodes runes with `utf8.DecodeRune`, so multi-byte identifiers work.
  Invalid UTF-8 yields `RuneError` (> 127) and is silently treated as an identifier
  character — the lexer never rejects bytes.
- The UTF-8 BOM (`\xEF\xBB\xBF`) is skipped in `Init()`.
- `Tag` (a `uint8`) covers the usual atoms and operators plus PDXScript specials:
  - `literal_boolean` — `yes` / `no` (detected after an identifier is lexed),
  - `macro_param` — `$IDENT$` lexed as a single atom (bare `$` falls back to `dollar`),
  - `script_value` — `@name` (a named script value reference),
  - `script_math` — `@[ … ]` (an inline math expression, one atom).
- `Tokenize(src []byte) []Token` is the public helper (skips `invalid`/`eof`); it's
  what `v3.Parse` consumes.

### 3.2 Parser v3 (`internal/parser/v3`)

Recursive descent + Pratt (precedence climbing), producing a **flat node-pool AST**.
This is the preferred parser for all new tools; v1 (participle) and v2 (pointer-tree)
exist only as benchmarking baselines.

```go
func Parse(filename string, src []byte) (*Tree, []Diagnostic)   // tree is ALWAYS non-nil

type Tree struct {
    Nodes []Node     // flat pool; Nodes[0] is always the KindFile root
    Index []uint32   // child indirection (see below)
    Src   []byte
}

type Node struct {
    Kind       NodeKind   // File | Field | Block | TaggedBlock | Scalar
    SrcStart   uint32     // byte offsets into Src (Value(src) == Src[SrcStart:SrcEnd])
    SrcEnd     uint32
    Op         lexer.Tag  // operator for KindField (= , ?= , >= , …); OpString() renders it
    ChildStart uint32     // range into Tree.Index
    ChildEnd   uint32
}
```

**No pointers inside nodes.** A node's children are
`Tree.Index[node.ChildStart:node.ChildEnd]`, and each element of that slice is an
*index into `Tree.Nodes`*. Helpers: `tree.Root()`, `tree.ChildRefs(n) []uint32`
(no alloc), `tree.Children(n) []Node` (allocates). This layout gives ~2× fewer
allocations than v2's pointer tree.

Node-kind semantics:

- `KindFile` — root; children are the top-level items.
- `KindField` — a `key OP value`; `children[0]` is the key `KindScalar`,
  `children[1]` is the value; `Op` holds the operator tag.
- `KindBlock` — `{ … }`; children are the items inside.
- `KindTaggedBlock` — `tag = { … }` style; `SrcStart..SrcEnd` is the tag text.
- `KindScalar` — a leaf; `SrcStart..SrcEnd` is its literal text. Scope chains like
  `scope:foo.bar` are lexed/parsed into a single scalar via infix binding power on
  `.`, `:`, `|` (bp 80).

**Error recovery.** On an unexpected token the parser records a
`Diagnostic{Filename, Offset, Msg, Severity}` and calls `synchronize()`, which skips
tokens until it reaches a `}`, EOF, or the start of a plausible new item (an atom
followed by an operator or `{`) — without consuming that resumption token. Recovery
is **zero-cost on valid input** (no diagnostics allocated). A non-empty `[]Diagnostic`
means errors were found but parsing continued, so callers should check
`len(diags) > 0` rather than trusting the (always non-nil) tree blindly.

> `Diagnostic.Offset` is a byte offset; render it with
> `lexer.Token{Start: d.Offset}.FormatPosition(d.Filename, src)`.

### 3.3 Cache (`internal/cache`)

A two-level parse cache so re-runs and warm LSP starts are cheap.

```go
func NewStore(dir string, lruCap int) (*Store, error)
func (s *Store) Get(path string, info os.FileInfo) (*v3.Tree, []v3.Diagnostic, error) // nil tree on miss
func (s *Store) Put(path string, info os.FileInfo, src []byte, tree *v3.Tree, diags []v3.Diagnostic) error
```

- **L1** — in-memory LRU (only when `lruCap > 0`), invalidated by mtime.
- **L2** — on-disk gob entries. Filename is `sha256(filepath.Clean(path)).bin`. Each
  `diskEntry` holds `{ModTime, SHA256[32], SrcGzip, Nodes, Index, Diags}` — the source
  is stored gzip-compressed, so a hot L2 read reconstructs the `Tree` without touching
  the original file.
- **Invalidation:** L1 checks mtime; L2 always verifies SHA-256 (mtime alone is
  unreliable on coarse filesystems). Same content + different mtime → refresh the
  stored mtime and keep the entry; changed hash → full miss.
- `.pdxl/.gitignore` is written on first use.

> **Content-keyed caveat.** Entries are keyed by *source content*, not by the version
> of pdxl that produced them. After changing lexer/parser/validate logic, stale
> entries can mask your change — run with `--no-cache` or `pdxl cache clear`.

### 3.4 Files (`internal/files`)

`FileSet` resolves the Paradox mod-overlay: vanilla first, mod last, later roots
shadow earlier ones for the same relative path.

```go
type FileEntry struct {
    RelPath  string   // overlay key: lowercased, forward-slash
    FullPath string   // absolute path for reading
    Kind     FileKind // Vanilla | DLC | Dependency | Mod
}

func (s *FileSet) Add(root string, kind FileKind) error
func (s *FileSet) Walk(fn func(FileEntry) error) error   // winning entries only, in insertion order
```

- Add roots in load order; `byPath[RelPath]` records the *winning* (last-added) entry.
  `Walk` visits only winners. `Stats()` reports `{Vanilla, Mod, Total, Shadowed, Replaced}`.
- `SetIgnore(dirs, files)` skips non-script `.txt` (driven by the `[scan]` config).
- `SetReplacePaths(prefixes)` implements `replace_path`: matching vanilla/DLC files are
  dropped before mod files are added.
- `.mod` files are parsed via `ParseMod` (which itself uses `v3.Parse`); Windows paths
  inside them resolve through `ResolveWindowsPath(winPath, protonPrefix)` →
  `<protonPrefix>/drive_c/…` for Proton/Steam layouts.

### 3.5 Validate (`internal/validate`)

The cross-file semantic layer on top of v3 trees + `FileSet`. Two passes:
**extract per-file facts**, then **merge + resolve**.

```go
type FileFacts struct {
    Defs    []Symbol  // definitions this file declares (duplicate-tracked on merge)
    Aliases []Symbol  // trait group / group_equivalence names (gap-fill, no dup tracking)
    Refs    []Ref     // filtered, resolvable references
}

type Symbol struct {
    Name      string
    Kind      SymbolKind  // ScriptedTrigger | ScriptedEffect | Trait | Event | Decision | OnAction | Character
    File      string      // FileSet RelPath
    Offset    int         // byte offset of the definition field node
    EndOffset int         // byte offset just past the definition name (drives go-to-definition range)
    Params    []string    // sorted $PARAM$ macro names found in the body
}

type Ref struct {
    Kind  SymbolKind
    Name  string
    File  string   // on-disk full path
    Start int      // byte range of the referenced value
    End   int
    Loc   string   // precomputed "file:line:col" for the CLI
}
```

- **`extractFacts(tree, relPath, fullPath)`** walks the AST once. Definitions are
  harvested by directory via the CK3 registry in `schema_ck3.go` (e.g.
  `common/traits/` → `KindTrait`). References are recognized by key via rule maps:
  `ck3RefRules` (scalar, e.g. `add_trait = X`), `ck3BlockIDRefRules`
  (`trigger_event = { id = X }`), and on_action-only `ck3ListRefRules` /
  `ck3WeightedRefRules`. Values that can't be resolved without scope tracking — macro
  params (`$X$`), scope chains (`foo:bar`), scope keywords (`root`/`prev`/…), quoted
  edge cases — are skipped (`skipRefValue`).
- **`mergeAndResolve(order, facts)`** builds the `SymbolTable`
  (`byKind[Kind][Name]Symbol`, **first-writer-wins** with a `Duplicates` list; aliases
  fill gaps only) and resolves every `Ref` via `Lookup(kind, name)`, emitting a
  `RefDiag` for each miss.
- **`FactStore`** (`factcache.go`) caches `FileFacts` per file on disk, SHA-invalidated
  like the AST cache (same content-keyed caveat). Unchanged files skip parsing entirely.

**`Project` — the incremental engine.** This is what long-running consumers (the LSP
server, `watch`) hold:

```go
func NewProject(fs *files.FileSet, ast *cache.Store, fc *FactStore) (*Project, error)
func (p *Project) UpdateSource(fullPath string, src []byte) error  // re-parse ONE file from a buffer
func (p *Project) Update(fullPath string) error                    // re-parse ONE file from disk
// accessors used by the LSP: Table(), Diags(), FileDiags(path), FactsAt(path), RelToFull(rel)
```

`UpdateSource` parses just the changed file, replaces its entry in the in-memory
`facts` map, and calls `rebuild()` → `mergeAndResolve` over **all** files' facts. The
whole-table rebuild sounds expensive but is cheap: every *other* file's facts are
already in memory, so nothing else is re-parsed. This is the key to sub-second
incremental updates on the full CK3 corpus.

### 3.6 LSP (`internal/lsp`)

A long-running language server (glsp, LSP 3.16, stdio) wrapping exactly one
`validate.Project`. Started by `pdxl lsp`.

```go
type Server struct {
    opts      Options
    mu        sync.Mutex           // guards proj, docs, published, timers
    notify    glsp.NotifyFunc      // captured in `initialized` for async publishing
    proj      *validate.Project
    docs      map[string][]byte    // open document URI -> current buffer text
    timers    map[string]*time.Timer
    published map[string]struct{}  // full paths currently showing diagnostics
}
```

- **`initialize` returns fast**; the project is built **asynchronously** in
  `initialized` (a large game corpus would otherwise blow the client's ~10s init
  timeout). Open documents that arrived during the build are re-published once it
  finishes.
- **Concurrency invariant: `Server.mu` is not reentrant.** Methods that already hold
  the lock (e.g. `publishProjectDiagnostics`) must use `readFileLocked`, *not*
  `readFile` (which acquires `mu` itself). Mixing them deadlocks. This split exists
  precisely because of that.
- Byte offsets ↔ LSP positions go through `offsetToPosition` / `positionToOffset`,
  which count **UTF-16 code units** (per the LSP spec), not bytes or runes.

See §4 for the two request lifecycles this layer implements.

### 3.7 CLI (`cmd/pdxl`) & Config (`internal/config`)

Cobra-based. The root command has persistent flags `--config` and `--verbose`/`-v`
(the latter sets slog to debug via `initLogging`, run from `cobra.OnInitialize`).
`main.go` injects the build version from ldflags.

| Command | Calls into | Purpose |
|---|---|---|
| `init`  | config | Write a default `pdxl.toml` (`--force` to overwrite) |
| `lex`   | lexer | Dump the token stream |
| `parse` | parser/v3 | Print the AST (`--tree` / `--json`) |
| `lint`  | cache, files, parser/v3 | Structural diagnostics (`--context`, `--no-cache`) |
| `index` | cache, files, parser/v3 | Scan game+mod, parse all winners, report counts |
| `check` | cache, files, validate | One-shot definitions + reference resolution; non-zero exit on unresolved |
| `cache` | cache | `cache size [--detailed]` / `cache clear` |
| `watch` | cache, validate | Persistent validator: build once, watch the mod dir (fsnotify, 100ms debounce), serve diagnostics over HTTP (`GET /diagnostics[?file=…]`, `GET /health`; default `--addr 127.0.0.1:7777`) |
| `lsp`   | lsp | Run the language server over stdio (`--game`, `--log-level`; `--stdio` accepted and ignored) |

`config.Load(path)` starts from `Default()` and overlays the `pdxl.toml` file, so a
partial config inherits defaults; a missing file is not an error. `watch` and `lsp`
are the two long-running consumers of `validate.Project`.

---

## 4. Request lifecycles

These two traces are the heart of the LSP. Both run against the single in-memory
`Project`.

### 4.1 Keystroke → diagnostic

```
editor edit
  → didChange                      (store buffer in s.docs[uri])
  → 200ms debounce                 (coalesce rapid edits; one timer per URI)
  → analyzeAndPublish
       → Project.UpdateSource      (re-parse ONLY the changed file from its buffer,
                                     then rebuild the whole table from in-memory facts)
       → publishProjectDiagnostics (under s.mu):
            • group Project.Diags() by file
            • keep only files under the mod root  (underModRoot; vanilla is analyzed
              but never flagged — see §1 scope)
            • publish LSP diagnostics per mod file (open OR unopened), reading source
              via readFileLocked (buffer if open, else disk)
            • clear files that had diagnostics last cycle but no longer do
            • record the new set in s.published
```

Because diagnostics are published per *mod file* (not per *open document*), an
unresolved reference shows up in the Problems panel even for files you never opened,
and fixing a definition clears references across the project. `didClose` re-analyzes
the closed file from disk and re-publishes, so it reverts to its on-disk state rather
than being force-cleared.

### 4.2 F12 → go-to-definition

```
textDocument/definition
  → definition handler
       → positionToOffset(buffer, params.Position)     (UTF-16 → byte offset)
       → FactsAt(path) → find the Ref whose [Start,End) spans the cursor offset
       → Project.Table().Lookup(ref.Kind, ref.Name)     (the defining Symbol)
       → RelToFull(sym.File)                             (RelPath → on-disk path)
       → read the definition file, convert sym.Offset..sym.EndOffset to an LSP Range
       → return protocol.Location{ URI, Range }
```

Every "no result" branch (cursor not on a ref, ref unresolved, file untracked)
returns `nil, nil`, which the editor treats as "no definition."

---

## 5. Cross-cutting invariants & gotchas

- **Byte offsets everywhere.** Lexer tokens, parser nodes, symbols, refs, and
  diagnostics all use 0-indexed, half-open byte offsets. Line/column is a display-only
  derivation; the LSP boundary additionally converts to UTF-16 code units.
- **Content-keyed caches.** Both the AST cache and the `FactStore` key on source
  content, not on pdxl's version. After changing lexer/parser/validate logic, use
  `--no-cache` or `pdxl cache clear`, or you'll debug against stale results.
- **Whole-table rebuild per edit is intentional and cheap.** `UpdateSource` re-parses
  one file and rebuilds the table from in-memory facts; unchanged files are never
  re-read. Don't "optimize" this into partial table mutation without measuring — the
  current model is what keeps it correct and simple.
- **The LSP mutex is not reentrant.** Respect the `readFile` (locks) vs
  `readFileLocked` (caller holds the lock) split, or you'll deadlock the server.
- **Diagnostics are mod-scoped.** Vanilla files feed the symbol table but are never
  flagged. Keep it that way unless the validation story changes.
- **Keep functions flat.** `golangci-lint` enforces revive `cognitive-complexity`
  (max 20) and `nestif`; extract helpers rather than nesting. Verbose debug-logging
  blocks are a common way to trip these.
- **Token tag constants are `snake_case`** by deliberate choice; the linter's
  `var-naming` rule is disabled for that package.
</content>
