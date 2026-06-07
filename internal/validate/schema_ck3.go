package validate

import "strings"

// defRule maps a RelPath prefix to the kind of symbol defined by files there.
// Phase 0 is CK3-only and hand-written; it can later be generated or made
// game-aware via config.
type defRule struct {
	prefix string
	kind   SymbolKind
}

// ck3DefRules lists the directories whose top-level NAME = { ... } fields are
// definitions. Longest prefixes are not required to be ordered; ruleFor scans
// linearly and the prefixes are mutually exclusive.
var ck3DefRules = []defRule{
	{"common/scripted_triggers/", KindScriptedTrigger},
	{"common/scripted_effects/", KindScriptedEffect},
	{"common/traits/", KindTrait},
	{"common/decisions/", KindDecision},
	{"common/on_action/", KindOnAction},
	{"events/", KindEvent},
}

// ruleFor returns the def rule whose prefix matches relPath, if any.
// relPath is the normalised (lowercase, forward-slash) FileSet key.
func ruleFor(relPath string) (defRule, bool) {
	for _, r := range ck3DefRules {
		if strings.HasPrefix(relPath, r.prefix) {
			return r, true
		}
	}
	return defRule{}, false
}
