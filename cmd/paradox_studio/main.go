package main

import (
	"fmt"
	"os"

	"go-pdxl/internal/lexer"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintf(os.Stderr, "Usage: %s <file>\n", os.Args[0])
		os.Exit(1)
	}

	filename := os.Args[1]
	data, err := os.ReadFile(filename)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error reading file %s: %v\n", filename, err)
		os.Exit(1)
	}

	l := lexer.Init(data)
	for {
		token := l.Next()
		if token == nil {
			break // EOF
		}
		fmt.Printf("%s: %v\n", token.FormatPosition(filename, data), token.Tag)
	}
}