package main

// version is set at build time via -ldflags="-X main.version=..."
var version = "dev"

func main() {
	rootCmd.Version = version
	Execute()
}
