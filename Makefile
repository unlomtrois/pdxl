.PHONY: test lint build clean bench bench-lexer bench-parser

test:
	go test ./...

lint:
	golangci-lint run ./...

build:
	go build -o bin/pdxl ./cmd/pdxl

clean:
	rm -rf bin/

bench: bench-lexer bench-parser

bench-lexer:
	go test ./internal/lexer/... -bench=. -benchmem -count=3

bench-parser:
	go test ./internal/parser/... -bench=. -benchmem -count=3
