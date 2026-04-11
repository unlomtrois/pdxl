# Ideas

## Lexer rewrite as a finite state machine

The Go compiler's own lexer (`src/cmd/compile/internal/syntax/scanner.go`) is implemented as an FSM — each state is a function that returns the next state. This makes transitions explicit and eliminates the current pattern of `Next()` inspecting results from `lexNumber`/`lexIdentifier` to decide the final tag.

Concrete benefit: the digit-leading identifier case (`1abc`), decimal number (`0.1`), and keyword detection (`yes`/`no`) are all post-scan fixups in `Next()` right now. An FSM would encode these as state transitions instead, making the rules easier to follow and extend.

Reference: `go/src/cmd/compile/internal/syntax/scanner.go`
