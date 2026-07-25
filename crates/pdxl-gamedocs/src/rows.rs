//! Handwritten row types for the GENERATED per-game doc tables
//! (`gen-tables` output). Shared by every game crate (`pdxl-ck3`,
//! `pdxl-eu5`), so the tables of different games are the same types and the
//! consumers' facade (`pdxl-game`) presents one surface.

/// One effect or trigger: the scopes it is legal in and, for iterators,
/// the element scope it yields.
#[derive(Clone, Copy, Debug)]
pub struct DocRow {
    pub name: &'static str,
    /// Human-readable usage notes from the game's documentation dump.
    pub description: &'static str,
    /// `Supported Scopes:` verbatim (`"none"` = no scope requirement).
    pub scopes: &'static [&'static str],
    /// `Supported Targets:` — for `any_*` iterators, the element scope.
    pub targets: &'static [&'static str],
}

/// One scope-transition link (`event_targets.log`): `title:` / `.holder` /
/// `scope:` style navigation.
#[derive(Clone, Copy, Debug)]
pub struct LinkRow {
    pub name: &'static str,
    /// Takes a `:key` argument (`title:e_hre`, `character:1234`).
    pub requires_data: bool,
    /// Usable from any scope; otherwise `input_scopes` constrains it.
    pub global_link: bool,
    pub wildcard: bool,
    pub input_scopes: &'static [&'static str],
    pub output_scopes: &'static [&'static str],
}

/// One scope type from the game's scope-type registry (`event_scopes.log`).
#[derive(Clone, Copy, Debug)]
pub struct ScopeTypeRow {
    pub name: &'static str,
    pub evaluate_triggers: bool,
    pub execute_effects: bool,
    pub change_scopes: bool,
    pub save_token: Option<&'static str>,
    pub stores_variables: bool,
}

/// One permanent stat modifier (`modifiers.log`).
#[derive(Clone, Copy, Debug)]
pub struct ModifierRow {
    /// May be templated (`$TRAIT_TRACK$_xp_gain_mult`).
    pub tag: &'static str,
    pub use_areas: &'static [&'static str],
}
