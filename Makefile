.PHONY: test lint build clean

test:
	go test ./...

lint:
	golangci-lint run ./...

build:
	go build -o bin/pdxl ./cmd/pdxl

clean:
	rm -rf bin/