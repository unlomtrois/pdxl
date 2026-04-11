.PHONY: test lint build clean

test:
	go test ./...

lint:
	golangci-lint run ./...

build:
	go build -o bin/paradox_studio ./cmd/paradox_studio

clean:
	rm -rf bin/