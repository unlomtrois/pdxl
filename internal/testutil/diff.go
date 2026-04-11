package testutil

import (
	"bytes"
	"fmt"
	"strings"
)

// DiffLines returns a human-readable diff of up to 20 differing lines between
// got and want. Used by fixture golden tests across parser packages.
func DiffLines(got, want string) string {
	gotLines := strings.Split(got, "\n")
	wantLines := strings.Split(want, "\n")
	var b bytes.Buffer
	max := len(gotLines)
	if len(wantLines) > max {
		max = len(wantLines)
	}
	shown := 0
	for i := 0; i < max && shown < 20; i++ {
		g, w := "", ""
		if i < len(gotLines) {
			g = gotLines[i]
		}
		if i < len(wantLines) {
			w = wantLines[i]
		}
		if g != w {
			fmt.Fprintf(&b, "line %d:\n  got:  %q\n  want: %q\n", i+1, g, w)
			shown++
		}
	}
	if shown == 0 {
		b.WriteString("(no line differences found — possibly trailing newline)")
	}
	return b.String()
}
