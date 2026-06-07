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
	{"history/characters/", KindCharacter},
}

// ck3RefRules maps a value-position key to the kind of symbol its scalar value
// must resolve to. Hand-written and CK3-only for now; grows incrementally.
var ck3RefRules = map[string]SymbolKind{
	"add_trait":     KindTrait,
	"remove_trait":  KindTrait,
	"has_trait":     KindTrait,
	"trigger_event": KindEvent, // scalar form: trigger_event = ns.id
}

// ck3BlockIDRefRules maps a key whose block value carries an `id = X` reference
// to the kind X must resolve to, e.g. trigger_event = { id = ns.id days = 5 }.
var ck3BlockIDRefRules = map[string]SymbolKind{
	"trigger_event": KindEvent,
}

// ck3ListRefRules maps a key whose block holds loose value items that each
// resolve to a kind, e.g. on_action `events = { ns.id ... }`. Applied only in
// on_action files (these keys are ambiguous elsewhere).
var ck3ListRefRules = map[string]SymbolKind{
	"events":      KindEvent,
	"first_valid": KindEvent,
	"on_actions":  KindOnAction,
}

// ck3WeightedRefRules maps a key whose block holds WEIGHT = id fields (the value
// is the reference), e.g. on_action `random_events = { 50 = ns.id ... }`.
// Numeric values (weights, config like chance_to_happen) are skipped. Applied
// only in on_action files.
var ck3WeightedRefRules = map[string]SymbolKind{
	"random_events": KindEvent,
}

// OnActionDir is the file prefix under which list/weighted reference rules apply.
const OnActionDir = "common/on_action/"

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
