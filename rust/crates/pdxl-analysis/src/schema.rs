//! The schema: game-specific knowledge as *data*, not code.
//!
//! The Go implementation hardcodes CK3 specifics inside its extraction
//! functions (`rule.kind == KindTrait` triggers alias harvesting;
//! `common/on_action/` gates list rules). This crate's engine is generic; every
//! game-specific decision lives in a [`Schema`] value supplied by a rules crate
//! (`pdxl-ck3`). Same behavior, different ownership.

use std::collections::{HashMap, HashSet};

use crate::model::SymbolKind;

/// Maps a `RelPath` prefix to the kind of symbol defined by files there.
#[derive(Clone, Debug)]
pub struct DefRule {
    pub prefix: &'static str,
    pub kind: SymbolKind,
    /// `None`: definitions are the top-level `NAME = { … }` fields (the
    /// original model). `Some(prefixes)`: definitions form a **tree** — a key
    /// is a definition (and is recursed into) iff it starts with one of these
    /// prefixes AND its value is a block. CK3 landed titles:
    /// `e_x = { k_y = { d_z = { … } } }` with attribute keys (`color`,
    /// `cultural_names`, …) interleaved at every level.
    pub nested_key_prefixes: Option<&'static [&'static str]>,
}

/// Everything the extraction engine needs to know about a game's conventions.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    /// Directories whose top-level `NAME = { … }` fields are definitions.
    /// Scanned linearly; prefixes must be mutually exclusive (Go parity).
    pub def_rules: Vec<DefRule>,
    /// `key = value` — the scalar value resolves to a kind.
    pub ref_rules: HashMap<&'static str, SymbolKind>,
    /// `key = { id = value … }` — the `id` scalar resolves to a kind.
    pub block_id_ref_rules: HashMap<&'static str, SymbolKind>,
    /// `key = { item item … }` — loose scalar items, gated by `list_gate_prefix`.
    pub list_ref_rules: HashMap<&'static str, SymbolKind>,
    /// `key = { WEIGHT = id … }` — numeric-keyed values, gated like lists.
    pub weighted_ref_rules: HashMap<&'static str, SymbolKind>,
    /// RelPath prefix under which list/weighted rules apply (they are ambiguous
    /// elsewhere). Go: `OnActionDir`.
    pub list_gate_prefix: &'static str,
    /// Definition kinds that expose extra resolvable names via direct-child
    /// fields, e.g. CK3 traits' `group` / `group_equivalence`.
    pub alias_keys: HashMap<SymbolKind, &'static [&'static str]>,
    /// Relative-scope keywords a reference value may be at runtime
    /// (`has_trait = prev`); unresolvable without scope tracking, so skipped.
    pub scope_keywords: HashSet<&'static str>,
    /// Self-identifying scope-literal prefixes: any scalar (key, value, or
    /// list item, at any depth) of the form `<prefix>:<name>[.chain…]`
    /// references `<name>` as the mapped kind. CK3: `title:` → Title
    /// (`has_title = title:e_hre`, `title:k_england = { … }`,
    /// `title:e_byzantium.holder`). The extracted range covers exactly the
    /// name, so editor features land on it precisely.
    pub scope_ref_prefixes: Vec<(&'static str, SymbolKind)>,
}

impl Schema {
    /// The def rule whose prefix matches `rel_path`, if any (Go's `ruleFor`).
    pub fn rule_for(&self, rel_path: &str) -> Option<&DefRule> {
        self.def_rules
            .iter()
            .find(|r| rel_path.starts_with(r.prefix))
    }

    /// Whether a reference value should not be resolved: macro parameters
    /// (`$X$`), scope/data-function chains (`foo:bar`), relative-scope
    /// keywords, and empties (Go's `skipRefValue`).
    pub fn skip_ref_value(&self, val: &str) -> bool {
        if val.is_empty() || val.contains(['$', ':']) {
            return true;
        }
        self.scope_keywords.contains(val)
    }
}
