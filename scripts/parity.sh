#!/usr/bin/env bash
#
# parity.sh — run the Go oracle tests, the Rust tests, and the lexer differential
# comparison in one shot. The Go implementation is the oracle; the Rust port is
# validated against it.
#
# Usage:
#   scripts/parity.sh            # full run: go test + cargo test + differential
#   scripts/parity.sh --lex      # only the lexer differential dump comparison
#
# Exit status is non-zero if any stage fails.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

run_lex_diff() {
    echo "== lexer differential: Go oracle vs Rust =="
    local fail=0 count=0
    # Shared fixtures plus the Rust-side stress fixtures (malformed UTF-8 etc.).
    local fixtures=()
    while IFS= read -r -d '' f; do fixtures+=("$f"); done < <(
        find testdata rust/crates/pdxl-lexer/testdata -name '*.txt' -print0 2>/dev/null | sort -z
    )
    for f in "${fixtures[@]}"; do
        local go_out rust_out
        go_out="$(go run ./tools/lexdump "$f")"
        rust_out="$(cargo run --quiet --manifest-path rust/Cargo.toml --bin lexdump -- "$f")"
        if [[ "$go_out" == "$rust_out" ]]; then
            count=$((count + 1))
        else
            echo "MISMATCH: $f"
            diff <(printf '%s' "$go_out") <(printf '%s' "$rust_out") | head -20 || true
            fail=1
        fi
    done
    if [[ $fail -eq 0 ]]; then
        echo "OK: $count fixtures byte-identical"
    else
        echo "FAILED: lexer differential mismatch"
        return 1
    fi
}

if [[ "${1:-}" == "--lex" ]]; then
    run_lex_diff
    exit 0
fi

echo "== Go oracle tests =="
go test ./...

echo "== Rust tests =="
cargo test --manifest-path rust/Cargo.toml --workspace

# The cargo test run above already executes the differential parity integration
# test (crates/pdxl-lexer/tests/parity.rs). This standalone pass is a redundant,
# human-readable confirmation.
run_lex_diff

echo "== parity OK =="
