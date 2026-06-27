#!/usr/bin/env bash
#
# parity.sh — run the Go oracle tests, the Rust tests, and the lexer differential
# comparison in one shot. The Go implementation is the oracle; the Rust port is
# validated against it.
#
# Usage:
#   scripts/parity.sh            # full run: go test + cargo test + differentials
#   scripts/parity.sh --lex      # only the lexer differential dump comparison
#   scripts/parity.sh --parse    # only the parser differential dump comparison
#
# Exit status is non-zero if any stage fails.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# All shared fixtures plus the Rust-side stress fixtures (malformed UTF-8 etc.).
collect_fixtures() {
    find testdata rust/crates/pdxl-lexer/testdata -name '*.txt' -print0 2>/dev/null | sort -z
}

# run_diff <label> <go-tool> <rust-bin>: compare a Go and Rust dump tool over all
# fixtures, asserting byte-identical output.
run_diff() {
    local label="$1" go_tool="$2" rust_bin="$3"
    echo "== ${label} differential: Go oracle vs Rust =="
    local fail=0 count=0
    local fixtures=()
    while IFS= read -r -d '' f; do fixtures+=("$f"); done < <(collect_fixtures)
    for f in "${fixtures[@]}"; do
        local go_out rust_out
        go_out="$(go run "$go_tool" "$f")"
        rust_out="$(cargo run --quiet --manifest-path rust/Cargo.toml --bin "$rust_bin" -- "$f")"
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
        echo "FAILED: ${label} differential mismatch"
        return 1
    fi
}

run_lex_diff() { run_diff "lexer" "./tools/lexdump" "lexdump"; }
run_parse_diff() { run_diff "parser" "./tools/parsedump" "parsedump"; }

case "${1:-}" in
--lex)
    run_lex_diff
    exit 0
    ;;
--parse)
    run_parse_diff
    exit 0
    ;;
esac

echo "== Go oracle tests =="
go test ./...

echo "== Rust tests =="
cargo test --manifest-path rust/Cargo.toml --workspace

# The cargo test run above already executes the differential parity integration
# tests; these standalone passes are redundant, human-readable confirmations.
run_lex_diff
run_parse_diff

echo "== parity OK =="
