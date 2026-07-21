//! Structural contexts: what *kind of clause* is expected at a tree position.
//!
//! Layer 1 of the three-layer scope model (`rust/docs/STRUCTURAL-CONTEXTS.md`):
//! given a node in a parsed file, [`context_at`] answers whether it sits in an
//! effect clause, a trigger clause, a script value, a dynamic description, or
//! a structural block with enumerable fields. Layer 2 (which *names* are legal
//! inside an effect/trigger clause) is served by the generated doc tables;
//! layer 3 (dynamic scope types) is future work.
//!
//! The engine here is game-agnostic; the specs — which directory produces
//! which root context, what an event or option block looks like — are data
//! supplied by the rules crate (`pdxl-ck3`), same split as [`Schema`].
//!
//! [`Schema`]: crate::Schema

use pdxl_ast::{NodeId, NodeKind, SyntaxTree};

/// The kind of clause a block position expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClauseKind {
    /// Mutates game state (`immediate = { … }`, option bodies).
    Effect,
    /// Checks game state (`trigger = { … }`, `limit = { … }`).
    Trigger,
    /// Arithmetic with conditional branches (`ai_will_select`,
    /// `common/script_values/` bodies).
    ScriptValue,
    /// `base` + weighted `modifier = { <triggers> }` blocks (`ai_chance`,
    /// `common/scripted_modifiers/` bodies).
    ScriptedModifier,
    /// A static-modifier body: `<modifier tag> = <number>` lines plus `icon`
    /// (`common/modifiers/` definitions). Keys are built-in modifier tags.
    StaticModifier,
    /// The dynamic-description mini-language (`desc`, `triggered_desc`,
    /// `first_valid`, `random_valid`, `switch`).
    DynamicDesc,
    /// A structural block with enumerable fields (event root, option,
    /// portrait, …).
    Struct(&'static StructSpec),
    /// A plain setting or data value; no clause inside.
    Config,
    /// Nothing is known about this position (unknown directory, key
    /// rejected by a strict struct, malformed tree).
    Unknown,
}

/// What an unknown key means inside a [`StructSpec`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// Unknown keys are inline effects (the event-option rule).
    Effect,
    /// Unknown keys are triggers.
    Trigger,
    /// Unknown keys are data (dynamic keys: weighted lists, switch cases).
    Ignore,
    /// Unknown keys are invalid here (strict structural blocks).
    Deny,
    /// Unknown keys are static-modifier tags (trait bodies, XP-track levels:
    /// "any other unknown property is read in as a modifier").
    Modifier,
    /// Unknown block-valued keys are definitions of another struct (a law
    /// group's arbitrarily-named laws open the law spec).
    Struct(&'static StructSpec),
}

/// What a scalar value in a structural field is (documentation-grade for
/// now; completion/validation may refine per kind later).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarKind {
    /// A keyword, number, bool, path — plain configuration.
    Setting,
    /// An event target / scope chain (`scope:x`, `root.liege`).
    Target,
    /// A localization key.
    LocKey,
}

/// The accepted value forms of one structural field. Many fields fork on the
/// value's node kind (`desc = key` vs `desc = { … }`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldSpec {
    pub scalar: Option<ScalarKind>,
    pub block: Option<ClauseKind>,
    /// The CK3 scope type the block's contents run in, when a definition's
    /// shape establishes a fixed root scope its script can't otherwise infer
    /// (law `can_keep` → `character`, `can_title_have` → `title`). `None`
    /// leaves the scope to the normal inference (event type, iterators, …).
    pub scope: Option<&'static str>,
    /// Human documentation for this field, distilled from the game's
    /// `_*.info` docs; surfaced on hover. `None` when undocumented.
    pub doc: Option<&'static str>,
    /// The known scalar values of this field, when the vocabulary is a fixed
    /// (or near-fixed) enum — `category = inventory/court`, `rarity = common/
    /// masterwork/famed/illustrious`. Surfaced as value completion and listed
    /// on hover. **Suggestions, not validation**: mods can extend some of
    /// these vocabularies (e.g. new artifact slot types), so an unlisted
    /// value is never diagnosed.
    pub values: Option<&'static [&'static str]>,
}

/// A block-valued field.
pub const fn block(kind: ClauseKind) -> FieldSpec {
    FieldSpec {
        scalar: None,
        block: Some(kind),
        scope: None,
        doc: None,
        values: None,
    }
}

/// A block-valued field whose contents run in a fixed CK3 scope type.
pub const fn block_scoped(kind: ClauseKind, scope: &'static str) -> FieldSpec {
    FieldSpec {
        scalar: None,
        block: Some(kind),
        scope: Some(scope),
        doc: None,
        values: None,
    }
}

/// A scalar-valued field.
pub const fn scalar(kind: ScalarKind) -> FieldSpec {
    FieldSpec {
        scalar: Some(kind),
        block: None,
        scope: None,
        doc: None,
        values: None,
    }
}

/// A field accepting either form.
pub const fn scalar_or_block(s: ScalarKind, b: ClauseKind) -> FieldSpec {
    FieldSpec {
        scalar: Some(s),
        block: Some(b),
        scope: None,
        doc: None,
        values: None,
    }
}

impl FieldSpec {
    /// Attaches hover documentation to a field spec (chainable in `const`).
    pub const fn doc(self, doc: &'static str) -> FieldSpec {
        FieldSpec {
            doc: Some(doc),
            ..self
        }
    }

    /// Attaches the field's known scalar values (chainable in `const`).
    /// Suggestions for completion/hover, never validation — see
    /// [`FieldSpec::values`].
    pub const fn values(self, values: &'static [&'static str]) -> FieldSpec {
        FieldSpec {
            values: Some(values),
            ..self
        }
    }
}

/// An enumerable structural block: known fields plus a rule for the rest.
#[derive(Debug, PartialEq, Eq)]
pub struct StructSpec {
    pub name: &'static str,
    pub fields: &'static [(&'static str, FieldSpec)],
    pub fallback: Fallback,
}

impl StructSpec {
    /// The field spec for `key`, if this struct declares it.
    pub fn field(&self, key: &str) -> Option<&FieldSpec> {
        self.fields.iter().find(|(k, _)| *k == key).map(|(_, f)| f)
    }
}

/// Directory-to-root-context table: which clause kind the *bodies* of
/// top-level definitions have under each `RelPath` prefix.
#[derive(Debug)]
pub struct ContextSchema {
    pub roots: &'static [(&'static str, ClauseKind)],
    /// Built-in effects whose block value is a documented structure
    /// (`create_character = { … }`). Consulted when a key resolves to effect
    /// context, so the block reads as [`ClauseKind::Struct`] rather than a bare
    /// effect clause.
    pub effect_structs: &'static [(&'static str, &'static StructSpec)],
}

impl ContextSchema {
    fn root_for(&self, rel_path: &str) -> Option<ClauseKind> {
        self.roots
            .iter()
            .find(|(p, _)| rel_path.starts_with(p))
            .map(|(_, k)| *k)
    }

    fn effect_struct(&self, key: &[u8]) -> Option<&'static StructSpec> {
        self.effect_structs
            .iter()
            .find(|(k, _)| k.as_bytes() == key)
            .map(|(_, spec)| *spec)
    }
}

/// The clause context of `node` in a file at `rel_path`.
///
/// Walks root→node once, threading the context through each `key = value`
/// descent. A field's key and the field node itself report the context of
/// their *containing* block; the value reports the context the key opens.
pub fn context_at(
    tree: &SyntaxTree,
    node: NodeId,
    rel_path: &str,
    schema: &ContextSchema,
) -> ClauseKind {
    let Some(body_kind) = schema.root_for(rel_path) else {
        return ClauseKind::Unknown;
    };
    let Some(path) = path_to(tree, node) else {
        return ClauseKind::Unknown;
    };

    // Context of the container currently walked into. At file top level the
    // "container" holds definitions; their values get `body_kind`.
    let mut ctx = ClauseKind::Config;
    let mut at_top = true;
    for pair in path.windows(2) {
        let (parent, child) = (pair[0], pair[1]);
        if tree.node(parent).kind != NodeKind::Field {
            continue;
        }
        let kids = tree.child_ids(parent);
        if kids.len() != 2 || child != kids[1] {
            continue; // the key (or a malformed field) stays in the container context
        }
        let value_is_block = matches!(
            tree.node(child).kind,
            NodeKind::Block | NodeKind::TaggedBlock
        );
        if at_top {
            // Top-level `NAME = { body }` opens the directory's body kind;
            // top-level scalars (`namespace = x`) are config.
            ctx = if value_is_block {
                body_kind
            } else {
                ClauseKind::Config
            };
            at_top = false;
            continue;
        }
        let key = tree.node_text(kids[0]);
        ctx = step(ctx, key, value_is_block);
    }
    ctx
}

/// Folds the context transitions over an enclosing-block key chain
/// (outermost first) — the brace stack a completion engine derives from the
/// raw tokens around a cursor. Unlike [`context_at`] this needs no parsed
/// tree, so it stays correct inside empty blocks and half-typed input.
/// Anonymous blocks (list items) pass an empty key. An empty chain is file
/// top level ([`ClauseKind::Config`]).
pub fn context_of_chain<'a, I>(keys: I, rel_path: &str, schema: &ContextSchema) -> ClauseKind
where
    I: IntoIterator<Item = &'a [u8]>,
{
    context_of_chain_rooted(keys, None, rel_path, schema)
}

/// Like [`context_of_chain`], but `root` overrides the directory-derived body
/// clause. Used for inline typed definitions (`scripted_effect NAME = { … }` in
/// an event file), whose body is an Effect/Trigger clause regardless of the
/// file's directory.
pub fn context_of_chain_rooted<'a, I>(
    keys: I,
    root: Option<ClauseKind>,
    rel_path: &str,
    schema: &ContextSchema,
) -> ClauseKind
where
    I: IntoIterator<Item = &'a [u8]>,
{
    let Some(body_kind) = root.or_else(|| schema.root_for(rel_path)) else {
        return ClauseKind::Unknown;
    };
    let mut ctx: Option<ClauseKind> = None;
    for key in keys {
        ctx = Some(match ctx {
            None => body_kind, // the outermost block is a definition body
            // A key that resolves to effect context and names a structured
            // built-in effect (`create_character`) opens that struct instead.
            Some(c) => match step(c, key, true) {
                ClauseKind::Effect => schema
                    .effect_struct(key)
                    .map_or(ClauseKind::Effect, ClauseKind::Struct),
                other => other,
            },
        });
    }
    ctx.unwrap_or(ClauseKind::Config)
}

/// The clause a `key` opens from within `ctx` — i.e. what its value block is.
/// For a struct context this applies the struct's field rules and fallback, so
/// an unknown key in an `option` (fallback effects) resolves to
/// [`ClauseKind::Effect`]. Used to classify a key (e.g. is `start_scheme` a
/// built-in effect here?) without a parsed tree.
pub fn resolve_key(ctx: ClauseKind, key: &str, value_is_block: bool) -> ClauseKind {
    step(ctx, key.as_bytes(), value_is_block)
}

/// The context transition: entering the value of `key = value` from `ctx`.
fn step(ctx: ClauseKind, key: &[u8], value_is_block: bool) -> ClauseKind {
    match ctx {
        // Inside effects, `limit`/`filter` flip to trigger context (the
        // `if = { limit = { <triggers> } <effects> }` duality); control
        // keywords and iterators keep effect context.
        ClauseKind::Effect => match key {
            b"limit" | b"filter" => ClauseKind::Trigger,
            _ => ClauseKind::Effect,
        },
        // Trigger context is closed under descent (`AND`/`OR`/`NOT`,
        // `trigger_if`, `any_*` iterators all contain triggers).
        ClauseKind::Trigger => ClauseKind::Trigger,
        // Script values branch with `if/else_if = { limit = { <triggers> } }`.
        ClauseKind::ScriptValue => match key {
            b"limit" => ClauseKind::Trigger,
            _ => ClauseKind::ScriptValue,
        },
        // Scripted modifiers: every block (`modifier = { … }`) holds trigger
        // conditions alongside its weight keys.
        ClauseKind::ScriptedModifier => {
            if value_is_block {
                ClauseKind::Trigger
            } else {
                ClauseKind::ScriptedModifier
            }
        }
        // Static modifiers are flat `tag = number` data; nothing scoped inside.
        ClauseKind::StaticModifier => ClauseKind::Config,
        // Dynamic descriptions nest arbitrarily; only `trigger` escapes into
        // trigger context. Unknown keys stay (switch cases are dynamic).
        ClauseKind::DynamicDesc => match key {
            b"trigger" => ClauseKind::Trigger,
            _ => ClauseKind::DynamicDesc,
        },
        ClauseKind::Struct(spec) => step_struct(spec, key, value_is_block),
        ClauseKind::Config => ClauseKind::Config,
        ClauseKind::Unknown => ClauseKind::Unknown,
    }
}

fn step_struct(spec: &'static StructSpec, key: &[u8], value_is_block: bool) -> ClauseKind {
    let known = std::str::from_utf8(key).ok().and_then(|k| spec.field(k));
    if let Some(field) = known {
        return match (value_is_block, field.block, field.scalar) {
            (true, Some(kind), _) => kind,
            (false, _, Some(_)) => ClauseKind::Config,
            // The value form this field does not accept.
            _ => ClauseKind::Unknown,
        };
    }
    match spec.fallback {
        Fallback::Effect => ClauseKind::Effect,
        Fallback::Trigger => ClauseKind::Trigger,
        Fallback::Ignore => ClauseKind::Config,
        Fallback::Deny => ClauseKind::Unknown,
        Fallback::Modifier => ClauseKind::StaticModifier,
        Fallback::Struct(s) if value_is_block => ClauseKind::Struct(s),
        // A scalar unknown key in a struct-fallback block is not a definition.
        Fallback::Struct(_) => ClauseKind::Config,
    }
}

/// Ancestor chain root..=target, or `None` if `target` is not in the tree.
fn path_to(tree: &SyntaxTree, target: NodeId) -> Option<Vec<NodeId>> {
    let mut path = Vec::new();
    if dfs(tree, tree.root(), target, &mut path) {
        Some(path)
    } else {
        None
    }
}

fn dfs(tree: &SyntaxTree, current: NodeId, target: NodeId, path: &mut Vec<NodeId>) -> bool {
    path.push(current);
    if current == target {
        return true;
    }
    for child in tree.children(current) {
        if dfs(tree, child, target, path) {
            return true;
        }
    }
    path.pop();
    false
}
