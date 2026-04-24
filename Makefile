VERSION ?= $(shell git describe --tags --always || echo "dev")
LDFLAGS  = -ldflags="-X main.version=$(VERSION)"

.PHONY: test lint build install clean bench bench-lexer bench-parser bench-cache

test:
	go test ./...

lint:
	golangci-lint run ./...

build:
	go build $(LDFLAGS) -o bin/pdxl ./cmd/pdxl

install:
	go install $(LDFLAGS) ./cmd/pdxl

clean:
	rm -rf bin/

bench: bench-lexer bench-parser bench-cache

bench-lexer:
	go test ./internal/lexer/... -bench=. -benchmem -count=3

bench-parser:
	go test ./internal/parser/... -bench=. -benchmem -count=3

bench-cache:
	go test ./internal/cache/... -bench=. -benchmem -count=3
