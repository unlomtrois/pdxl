package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"
	"pdxl/internal/lexer"
)

var showTags bool
var showPos bool

var lexCmd = &cobra.Command{
	Use:   "lex <file>",
	Short: "Tokenize a Paradox scripting file",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filename := args[0]
		data, err := os.ReadFile(filename)
		if err != nil {
			return fmt.Errorf("reading %s: %w", filename, err)
		}
		basename := filepath.Base(filename)
		l := lexer.Init(data)
		var sb strings.Builder
		for {
			token := l.Next()
			if token == nil {
				break
			}
			if token.IsInvalid() {
				fmt.Printf("%s: invalid %q\n", token.FormatPosition(basename, data), token.GetValue(data))
				continue
			}
			sb.Reset()
			if showPos {
				sb.WriteByte('[')
				sb.WriteString(token.FormatPosition(basename, data))
				sb.WriteString("]\t")
			}
			if showTags {
				fmt.Fprintf(&sb, "%-17s", token.Tag.String())
			}
			sb.Write(token.GetValue(data))
			fmt.Println(sb.String())
		}
		return nil
	},
}

func init() {
	lexCmd.Flags().BoolVar(&showTags, "tags", false, "show token tag alongside each value")
	lexCmd.Flags().BoolVar(&showPos, "show-pos", false, "show filename and position alongside each value")
	rootCmd.AddCommand(lexCmd)
}
