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

use crate::kind::{CallKinds, GuiKinds, KindId};

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
    /// A piece of text (localization keys).
    Text,
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
    /// Definitions are the top-level `NAME = <value>` fields, where the value
    /// may be a scalar *or* a block. CK3 script values come in both forms
    /// (`minor_stress_gain = 10` and `formula = { … }`). Unlike [`Self::TopLevel`],
    /// scalar-valued fields count — so this shape only fits directories where
    /// every top-level field is a definition (no `namespace`-style metadata).
    TopLevelValued,
    /// Definitions form a **tree**: a key is a definition (and is recursed
    /// into) iff it starts with one of these prefixes AND its value is a
    /// block. CK3 landed titles: `e_x = { k_y = { d_z = { … } } }` with
    /// attribute keys (`color`, `cultural_names`, …) interleaved at every
    /// level.
    Tree {
        key_prefixes: &'static [&'static str],
    },
    /// Definitions are the direct block-valued children of named container
    /// blocks found anywhere in the file (CK3 faiths inside `faiths = {}`).
    ChildrenOf { containers: &'static [&'static str] },
    /// Definitions are the block-valued children of every **top-level**
    /// block, minus the named block-valued attributes of that outer block.
    /// CK3 laws: top-level law groups whose block children are laws, except
    /// group attributes like `can_change_law_group`. (Scalar attributes —
    /// `default`, `flag`, `@vars` — are excluded for free by the block
    /// check.)
    GroupedBlocks { exclude: &'static [&'static str] },
    /// Definitions come from a semicolon-separated CSV file whose **first
    /// column is a numeric id** (CK3 `map_data/definition.csv`). Not a script
    /// shape: the project layer routes matching files to a CSV reader instead
    /// of the parser, so the extraction engine never sees this variant.
    IdCsv,
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
    /// `key = X` directly inside a **top-level definition body** (depth 1).
    /// For attribute fields whose key is reused deeper in script with another
    /// meaning (CB `group` vs `static_group_filter = { group = … }`).
    KeyValueTop(&'static str),
    /// `parent = { key = X … }` — like [`RefPattern::KeyValue`], but only
    /// when the field sits directly inside a block opened by `parent`
    /// (CK3: `name` is a loc key inside `option`, a variable-list name in
    /// list effects).
    KeyValueUnder(&'static str, &'static str),
    /// `key = { field = X … }` — the block's named-field scalar resolves to
    /// the kind (`trigger_event = { id = X }`, `trigger_event = { on_action = X }`).
    KeyBlockField(&'static str, &'static str),
    /// `key = { X Y … }` — loose scalar items each resolve to the kind.
    KeyList(&'static str),
    /// `key = { X = v Y = v … }` — the block's field *keys* each resolve to
    /// the kind (CK3 trait `compatibility` maps trait names to values).
    KeyBlockKeys(&'static str),
    /// `key = { WEIGHT = X … }` — numeric-keyed values resolve to the kind.
    KeyWeighted(&'static str),
    /// `prefix:X[.chain…]` — a self-identifying scope literal in ANY scalar
    /// position (key, value, or list item, at any depth). The extracted range
    /// covers exactly `X`, so editor features land on it precisely.
    ScopePrefix(&'static str),
    /// `X = { … }` at file top level — every top-level block's **key** is a
    /// reference to the kind (CK3 `history/provinces/`: the keys are province
    /// ids defined elsewhere, in `map_data/definition.csv`). `@var` script
    /// constants are skipped. Only meaningful gated to a directory.
    TopLevelBlockKeys,
}

/// A reference rule: a pattern, optionally gated to files under a `RelPath`
/// directory prefix (patterns like [`RefPattern::KeyList`] are ambiguous
/// outside their home directory — CK3's `events = { … }` means event refs
/// only under `common/on_action/`).
#[derive(Clone, Debug)]
pub struct RefRule {
    pub pattern: RefPattern,
    pub gate: Option<&'static str>,
    /// Alternate kinds the reference may resolve to when the owning kind
    /// does not define the name (CK3: `custom_description`'s `text` is a
    /// trigger-localization key, an effect-localization key, or a plain loc
    /// key). Diagnosed as unresolved only when no kind in the chain defines
    /// it; navigation follows whichever kind resolves.
    pub alt: &'static [KindId],
}

/// ALL knowledge about one game concept, co-located: adding a kind to a game
/// is one row here (one KindId const in the game crate, referenced
/// here).
#[derive(Clone, Debug)]
pub struct KindSpec {
    pub kind: KindId,
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
    pub kind: KindId,
    pub shape: DefShape,
}

/// A compiled key-triggered reference rule (the per-key form of
/// [`RefPattern`], with the key hoisted into the index).
#[derive(Clone, Debug)]
pub(crate) struct KeyRule {
    pub(crate) kind: KindId,
    pub(crate) form: KeyForm,
    pub(crate) gate: Option<&'static str>,
    pub(crate) alt: &'static [KindId],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyForm {
    Value,
    /// Scalar value, only as a direct child of a top-level definition body.
    ValueTop,
    /// Scalar value, only when the enclosing block's field key matches.
    ValueUnder(&'static str),
    /// The named direct-child field of the value block carries the reference.
    BlockField(&'static str),
    List,
    /// The block's field keys carry the references.
    BlockKeys,
    Weighted,
}

/// A compiled scope-literal rule (`prefix:name`).
#[derive(Clone, Debug)]
pub(crate) struct ScopeRule {
    pub(crate) prefix: &'static str,
    pub(crate) kind: KindId,
    pub(crate) gate: Option<&'static str>,
    pub(crate) alt: &'static [KindId],
}

/// A compiled top-level-block-keys rule ([`RefPattern::TopLevelBlockKeys`]).
#[derive(Clone, Debug)]
pub(crate) struct TopKeyRule {
    pub(crate) kind: KindId,
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
    /// Top-level-block-keys rules, checked against every top-level field.
    top_key_rules: Vec<TopKeyRule>,
    /// kind → its alias field keys.
    alias_keys: HashMap<KindId, &'static [&'static str]>,
    /// kind → its presentation hint.
    icons: HashMap<KindId, IconHint>,
    /// Relative-scope keywords a reference value may be at runtime
    /// (`has_trait = prev`); unresolvable without scope tracking, so skipped.
    scope_keywords: HashSet<&'static str>,
    /// Typed-definition keywords: a top-level `KEYWORD NAME = { … }` defines
    /// `NAME` of the mapped kind regardless of directory (CK3:
    /// `scripted_effect` / `scripted_trigger`, used inline in event files).
    typed_defs: HashMap<&'static str, KindId>,
    /// Keyed-value definitions: a top-level `KEY = value` where `KEY` maps here
    /// defines `value` of that kind (CK3: `namespace = X`). The definition is
    /// the declaration's value, so nothing else in the file is affected.
    keyed_value_defs: HashMap<&'static str, KindId>,
    /// Every kind in registration order — the stable order for reports and the
    /// doc-ref default lookup (replaces the old `SymbolKind::ALL`).
    kinds: Vec<KindId>,
    /// Doc-comment reference aliases (`![scheme:X]` → the scheme kind).
    by_alias: HashMap<&'static str, KindId>,
    /// Which kinds call-by-name references resolve to. `None` for schemas with
    /// no call-by-name convention.
    call_kinds: Option<CallKinds>,
    /// Which kinds interface-script (`.gui`) symbols get. `None` disables
    /// `.gui` analysis.
    gui_kinds: Option<GuiKinds>,
    /// The kind that bare `[concept|E]` encyclopedia links in localization text
    /// resolve to (CK3: game concepts). `None` disables loc-layer concept refs.
    loc_concept_kind: Option<KindId>,
}

impl Schema {
    /// Compiles kind rows (plus game-wide scope keywords, which belong to no
    /// single kind) into the lookup indices the extraction engine uses.
    // The trailing params are all optional game-binding hooks (call/gui/loc
    // conventions); if a fourth arrives, bundle them into a `GameHooks` struct.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        specs: &[KindSpec],
        scope_keywords: &[&'static str],
        typed_defs: &[(&'static str, KindId)],
        keyed_value_defs: &[(&'static str, KindId)],
        doc_ref_aliases: &[(&'static str, KindId)],
        call_kinds: Option<CallKinds>,
        gui_kinds: Option<GuiKinds>,
        loc_concept_kind: Option<KindId>,
    ) -> Schema {
        let mut schema = Schema {
            scope_keywords: scope_keywords.iter().copied().collect(),
            typed_defs: typed_defs.iter().copied().collect(),
            keyed_value_defs: keyed_value_defs.iter().copied().collect(),
            by_alias: doc_ref_aliases.iter().copied().collect(),
            call_kinds,
            gui_kinds,
            loc_concept_kind,
            ..Schema::default()
        };
        for spec in specs {
            // A kind may appear in several rows (e.g. one per def directory);
            // register it once, in first-seen order.
            if !schema.kinds.contains(&spec.kind) {
                schema.kinds.push(spec.kind);
            }
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
                    RefPattern::KeyValueTop(k) => (k, KeyForm::ValueTop),
                    RefPattern::KeyValueUnder(parent, k) => (k, KeyForm::ValueUnder(parent)),
                    RefPattern::KeyBlockField(k, field) => (k, KeyForm::BlockField(field)),
                    RefPattern::KeyList(k) => (k, KeyForm::List),
                    RefPattern::KeyBlockKeys(k) => (k, KeyForm::BlockKeys),
                    RefPattern::KeyWeighted(k) => (k, KeyForm::Weighted),
                    RefPattern::ScopePrefix(prefix) => {
                        schema.scope_rules.push(ScopeRule {
                            prefix,
                            kind: spec.kind,
                            gate: rule.gate,
                            alt: rule.alt,
                        });
                        continue;
                    }
                    RefPattern::TopLevelBlockKeys => {
                        schema.top_key_rules.push(TopKeyRule {
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
                    alt: rule.alt,
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
    pub fn icon(&self, kind: KindId) -> IconHint {
        self.icons.get(&kind).copied().unwrap_or(IconHint::Object)
    }

    /// Symbol kinds referenced by a scalar value for `key` in `rel_path`.
    /// This is a read-only query for editor features; extraction keeps using
    /// the compiled rules directly.
    pub fn value_kinds<'a>(
        &'a self,
        key: &'a str,
        rel_path: &'a str,
    ) -> impl Iterator<Item = KindId> + 'a {
        self.key_rules(key).into_iter().flat_map(move |rules| {
            rules.iter().filter_map(move |rule| {
                (matches!(rule.form, KeyForm::Value | KeyForm::ValueTop) && rule.applies(rel_path))
                    .then_some(rule.kind)
            })
        })
    }

    /// Symbol kinds referenced by loose or weighted list values for `key`.
    pub fn list_value_kinds<'a>(
        &'a self,
        key: &'a str,
        rel_path: &'a str,
    ) -> impl Iterator<Item = KindId> + 'a {
        self.key_rules(key).into_iter().flat_map(move |rules| {
            rules.iter().filter_map(move |rule| {
                (matches!(rule.form, KeyForm::List | KeyForm::Weighted) && rule.applies(rel_path))
                    .then_some(rule.kind)
            })
        })
    }

    /// Symbol kinds addressed by a configured `prefix:` scope literal.
    pub fn scope_prefix_kinds<'a>(
        &'a self,
        prefix: &'a str,
        rel_path: &'a str,
    ) -> impl Iterator<Item = KindId> + 'a {
        self.scope_rules.iter().filter_map(move |rule| {
            (rule.prefix == prefix && rule.applies(rel_path)).then_some(rule.kind)
        })
    }

    /// Whether a reference value should not be resolved: macro parameters
    /// (`$X$`), scope/data-function chains (`foo:bar`), file paths
    /// (`gfx/…dds`), relative-scope keywords, and empties (Go's `skipRefValue`).
    pub fn skip_ref_value(&self, val: &str) -> bool {
        if val.is_empty() || val.contains(['$', ':', '/']) || val.starts_with(['[', '@']) {
            // `[...]` values are inline datafunction text, not names; a `/`
            // means a texture/sound path, never a symbol key; `@…` is a script
            // constant (extracted separately, resolved file-locally).
            return true;
        }
        // A scope keyword — bare (`has_trait = prev`) or heading a chain
        // (`title = root.primary_title`) — is runtime navigation, not a name.
        let first_segment = val.split('.').next().unwrap_or(val);
        self.scope_keywords.contains(first_segment)
    }

    /// The kind a typed-definition keyword introduces (`scripted_effect` →
    /// [`KindId::ScriptedEffect`]), or `None` for a non-keyword scalar.
    pub fn typed_def_kind(&self, keyword: &str) -> Option<KindId> {
        self.typed_defs.get(keyword).copied()
    }

    /// The kind a top-level `KEY = value` defines through `KEY` (`namespace` →
    /// the namespace kind), the definition being `value`; `None` otherwise.
    pub fn keyed_value_def_kind(&self, key: &str) -> Option<KindId> {
        self.keyed_value_defs.get(key).copied()
    }

    /// Every kind in registration order (stable report / doc-ref ordering).
    pub fn kinds(&self) -> &[KindId] {
        &self.kinds
    }

    /// The kind a doc-comment reference alias names (`scheme` → the scheme kind).
    pub fn kind_by_alias(&self, alias: &str) -> Option<KindId> {
        self.by_alias.get(alias).copied()
    }

    /// Which kinds call-by-name references resolve to, if this game has any.
    pub fn call_kinds(&self) -> Option<CallKinds> {
        self.call_kinds
    }

    /// The interface-script symbol kinds, when the game declares them.
    pub fn gui_kinds(&self) -> Option<GuiKinds> {
        self.gui_kinds
    }

    /// The kind that `[concept|E]` localization links resolve to, if any.
    pub fn loc_concept_kind(&self) -> Option<KindId> {
        self.loc_concept_kind
    }

    pub(crate) fn key_rules(&self, key: &str) -> Option<&[KeyRule]> {
        self.key_rules.get(key).map(Vec::as_slice)
    }

    pub(crate) fn scope_rules(&self) -> &[ScopeRule] {
        &self.scope_rules
    }

    pub(crate) fn top_key_rules(&self) -> &[TopKeyRule] {
        &self.top_key_rules
    }

    pub(crate) fn alias_keys(&self, kind: KindId) -> Option<&'static [&'static str]> {
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

impl TopKeyRule {
    pub(crate) fn applies(&self, rel_path: &str) -> bool {
        self.gate.is_none_or(|g| rel_path.starts_with(g))
    }
}
