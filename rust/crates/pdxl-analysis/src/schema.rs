//! The schema: game-specific knowledge as *data*, not code.
//!
//! Organizing principle (see `rust/docs/SCHEMA-SCALING.md`): **co-locate by
//! growth axis**. Everything about one game concept lives in a single
//! [`KindSpec`] row supplied by a rules crate (`pdxl-ck3`); the engine grows a
//! new [`RefPattern`] variant only when PDXScript itself has a new syntactic
//! reference shape (rare). [`Schema::new`] compiles the rows into lookup
//! indices once, so the extraction hot path stays hash-lookup cheap.
//!
//! The Go implementation hardcodes CK3 specifics inside its extraction
//! functions (`rule.kind == KindTrait` triggers alias harvesting;
//! `common/on_action/` gates list rules). Here the engine is generic; per-rule
//! `gate` prefixes generalize the on_action gating.

use std::collections::{HashMap, HashSet};

use crate::model::SymbolKind;

/// A presentation hint for a symbol kind, neutral to any editor protocol.
/// The LSP layer maps these onto `lsp_types::SymbolKind`; other frontends can
/// map them however they like. Small and closed on purpose: it describes what
/// a kind *is like*, not what it *is*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconHint {
    /// Callable/composable logic (scripted triggers, effects).
    Function,
    /// Something that fires (events, on_actions).
    Event,
    /// A label attached to objects (traits).
    Tag,
    /// A player-facing action (decisions).
    Action,
    /// A concrete game object (characters).
    Object,
    /// A node in a containment hierarchy (landed titles).
    Hierarchy,
}

/// How definitions are shaped inside a directory's files.
///
/// An enum rather than optional fields on [`DefSource`], following ck3-tiger's
/// precedent (`ItemLoader::Normal` vs `::Full`): the common case pays no
/// ceremony, each variant names a real shape, and invalid flag combinations
/// are unrepresentable. If a future shape ever needs *logic* beyond data
/// (tiger's landed-titles handler also validates tier nesting order), the
/// escape hatch is tiger's: a bespoke extractor — not more fields here.
#[derive(Clone, Debug)]
pub enum DefShape {
    /// Definitions are the top-level `NAME = { … }` fields (the original
    /// model; every kind except landed titles).
    TopLevel,
    /// Definitions form a **tree**: a key is a definition (and is recursed
    /// into) iff it starts with one of these prefixes AND its value is a
    /// block. CK3 landed titles: `e_x = { k_y = { d_z = { … } } }` with
    /// attribute keys (`color`, `cultural_names`, …) interleaved at every
    /// level.
    Tree {
        key_prefixes: &'static [&'static str],
    },
}

/// Where a kind's definitions come from: files under `dir_prefix` (a
/// `RelPath` prefix), read according to `shape`.
#[derive(Clone, Debug)]
pub struct DefSource {
    pub dir_prefix: &'static str,
    pub shape: DefShape,
}

/// One syntactic shape a reference to a kind can take. Grows only when
/// PDXScript itself has a new reference syntax — game knowledge picks
/// variants, it never adds them.
#[derive(Clone, Debug)]
pub enum RefPattern {
    /// `key = X` — the scalar value resolves to the kind.
    KeyValue(&'static str),
    /// `key = { id = X … }` — the block's `id` scalar resolves to the kind.
    KeyBlockId(&'static str),
    /// `key = { X Y … }` — loose scalar items each resolve to the kind.
    KeyList(&'static str),
    /// `key = { WEIGHT = X … }` — numeric-keyed values resolve to the kind.
    KeyWeighted(&'static str),
    /// `prefix:X[.chain…]` — a self-identifying scope literal in ANY scalar
    /// position (key, value, or list item, at any depth). The extracted range
    /// covers exactly `X`, so editor features land on it precisely.
    ScopePrefix(&'static str),
}

/// A reference rule: a pattern, optionally gated to files under a `RelPath`
/// directory prefix (patterns like [`RefPattern::KeyList`] are ambiguous
/// outside their home directory — CK3's `events = { … }` means event refs
/// only under `common/on_action/`).
#[derive(Clone, Debug)]
pub struct RefRule {
    pub pattern: RefPattern,
    pub gate: Option<&'static str>,
}

/// ALL knowledge about one game concept, co-located: adding a kind to a game
/// is one row here (plus its `SymbolKind` variant while the enum remains the
/// ID — Phase 2 of the scaling plan replaces that with an open registry).
#[derive(Clone, Debug)]
pub struct KindSpec {
    pub kind: SymbolKind,
    pub icon: IconHint,
    /// Where definitions live, if this kind is definable from script files.
    pub defs: Option<DefSource>,
    /// Every way script text can reference this kind.
    pub refs: &'static [RefRule],
    /// Direct-child field keys whose values are extra resolvable names for a
    /// definition, e.g. CK3 traits' `group` / `group_equivalence`.
    pub aliases: &'static [&'static str],
}

/// A compiled definition-directory row (kept for [`Schema::rule_for`]'s
/// consumers; derived from [`KindSpec::defs`]).
#[derive(Clone, Debug)]
pub struct DefRule {
    pub prefix: &'static str,
    pub kind: SymbolKind,
    pub shape: DefShape,
}

/// A compiled key-triggered reference rule (the per-key form of
/// [`RefPattern`], with the key hoisted into the index).
#[derive(Clone, Debug)]
pub(crate) struct KeyRule {
    pub(crate) kind: SymbolKind,
    pub(crate) form: KeyForm,
    pub(crate) gate: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyForm {
    Value,
    BlockId,
    List,
    Weighted,
}

/// A compiled scope-literal rule (`prefix:name`).
#[derive(Clone, Debug)]
pub(crate) struct ScopeRule {
    pub(crate) prefix: &'static str,
    pub(crate) kind: SymbolKind,
    pub(crate) gate: Option<&'static str>,
}

/// Everything the extraction engine needs to know about a game's conventions,
/// compiled from [`KindSpec`] rows into lookup indices.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    /// Directories whose files define symbols. Scanned linearly; prefixes
    /// must be mutually exclusive (Go parity).
    def_rules: Vec<DefRule>,
    /// key → the reference rules that key can trigger (all key-based
    /// [`RefPattern`]s share this one index).
    key_rules: HashMap<&'static str, Vec<KeyRule>>,
    /// Scope-literal prefixes, checked against every scalar.
    scope_rules: Vec<ScopeRule>,
    /// kind → its alias field keys.
    alias_keys: HashMap<SymbolKind, &'static [&'static str]>,
    /// kind → its presentation hint.
    icons: HashMap<SymbolKind, IconHint>,
    /// Relative-scope keywords a reference value may be at runtime
    /// (`has_trait = prev`); unresolvable without scope tracking, so skipped.
    scope_keywords: HashSet<&'static str>,
}

impl Schema {
    /// Compiles kind rows (plus game-wide scope keywords, which belong to no
    /// single kind) into the lookup indices the extraction engine uses.
    pub fn new(specs: &[KindSpec], scope_keywords: &[&'static str]) -> Schema {
        let mut schema = Schema {
            scope_keywords: scope_keywords.iter().copied().collect(),
            ..Schema::default()
        };
        for spec in specs {
            schema.icons.insert(spec.kind, spec.icon);
            if let Some(defs) = &spec.defs {
                schema.def_rules.push(DefRule {
                    prefix: defs.dir_prefix,
                    kind: spec.kind,
                    shape: defs.shape.clone(),
                });
            }
            if !spec.aliases.is_empty() {
                schema.alias_keys.insert(spec.kind, spec.aliases);
            }
            for rule in spec.refs {
                let (key, form) = match rule.pattern {
                    RefPattern::KeyValue(k) => (k, KeyForm::Value),
                    RefPattern::KeyBlockId(k) => (k, KeyForm::BlockId),
                    RefPattern::KeyList(k) => (k, KeyForm::List),
                    RefPattern::KeyWeighted(k) => (k, KeyForm::Weighted),
                    RefPattern::ScopePrefix(prefix) => {
                        schema.scope_rules.push(ScopeRule {
                            prefix,
                            kind: spec.kind,
                            gate: rule.gate,
                        });
                        continue;
                    }
                };
                schema.key_rules.entry(key).or_default().push(KeyRule {
                    kind: spec.kind,
                    form,
                    gate: rule.gate,
                });
            }
        }
        schema
    }

    /// The def rule whose prefix matches `rel_path`, if any (Go's `ruleFor`).
    pub fn rule_for(&self, rel_path: &str) -> Option<&DefRule> {
        self.def_rules
            .iter()
            .find(|r| rel_path.starts_with(r.prefix))
    }

    /// The presentation hint for a kind ([`IconHint::Object`] when the kind
    /// has no spec — a neutral "some game object" fallback).
    pub fn icon(&self, kind: SymbolKind) -> IconHint {
        self.icons.get(&kind).copied().unwrap_or(IconHint::Object)
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

    pub(crate) fn key_rules(&self, key: &str) -> Option<&[KeyRule]> {
        self.key_rules.get(key).map(Vec::as_slice)
    }

    pub(crate) fn scope_rules(&self) -> &[ScopeRule] {
        &self.scope_rules
    }

    pub(crate) fn alias_keys(&self, kind: SymbolKind) -> Option<&'static [&'static str]> {
        self.alias_keys.get(&kind).copied()
    }
}

impl KeyRule {
    /// Whether this rule applies to the file at `rel_path`.
    pub(crate) fn applies(&self, rel_path: &str) -> bool {
        self.gate.is_none_or(|g| rel_path.starts_with(g))
    }
}

impl ScopeRule {
    pub(crate) fn applies(&self, rel_path: &str) -> bool {
        self.gate.is_none_or(|g| rel_path.starts_with(g))
    }
}
