//! The EU5 rules crate: game knowledge as data over the `pdxl-analysis`
//! engine. Starter schema — EU5 ships the same Jomini script layer as CK3
//! (scripted effects/triggers, script values, namespaced events), but under
//! module roots (`in_game/`, `main_menu/`) instead of a flat game dir, so
//! every directory prefix here carries the `in_game/` module.
//!
//! Public surface mirrors `pdxl-ck3` — consumers reach whichever game was
//! compiled in through the `pdxl-game` facade, so both crates must expose
//! the same modules and functions.
//!
//! Known noise (starter): EU5's `gfx/map/city_data/` files use an asset
//! convention where objects declare `name = "X"` and are referenced as
//! `@X` — the engine reads those as (unresolved) script constants, ~242
//! diagnostics in vanilla. Modeling that name/`@` relationship as its own
//! kind is a planned refinement, not a rule bug.

pub mod contexts;
pub mod coverage;
pub mod derived;
pub(crate) mod entities;
pub mod kinds;
pub mod tables;

use pdxl_analysis::{CallKinds, KindId, Schema};

/// Relative-scope keywords (`prev`, `this`, …) skipped during resolution.
// Uppercase forms are equally legal in Jomini script (`has_or_had_tag =
// ROOT` occurs in vanilla). `yes`/`no` are toggle values, never symbol names
// (`unlock_unit = yes`); `list` is the flag-definition selector keyword
// (`coa = list X`); `REB` is the engine's hardcoded rebel tag — real at
// runtime, defined in no file.
const SCOPE_KEYWORDS: &[&str] = &[
    "root", "this", "prev", "from", "fromfrom", "ROOT", "THIS", "PREV", "FROM", "yes", "no",
    "list", "REB",
];

/// `namespace = X` declares an event namespace (same convention as CK3).
const KEYED_VALUE_DEFS: &[(&str, KindId)] = &[("namespace", kinds::NAMESPACE)];

/// Any-depth keyed-value definitions: `define_unique_country_tag = SAGEO`
/// inside an event effect *creates* that tag — the definition site for the
/// ~31 dynamic countries (San Giorgio, the Sikh Empire, …) that exist in no
/// setup file.
const NESTED_VALUE_DEFS: &[(&str, KindId)] =
    &[("define_unique_country_tag", kinds::DYNAMIC_COUNTRY)];

const CALL_KINDS: CallKinds = CallKinds {
    effect: kinds::SCRIPTED_EFFECT,
    trigger: kinds::SCRIPTED_TRIGGER,
    value: kinds::SCRIPT_VALUE,
};

/// The compiled datafunction registry — empty until EU5's `DumpDataTypes`
/// output is generated into `tables`.
pub fn datafn_registry() -> &'static pdxl_gui::datafn::DataFnRegistry {
    static REG: std::sync::OnceLock<pdxl_gui::datafn::DataFnRegistry> = std::sync::OnceLock::new();
    REG.get_or_init(|| pdxl_gui::datafn::DataFnRegistry::from_rows(tables::DATA_FNS))
}

/// Builds the EU5 schema: the hand-written entity rows plus the
/// table-derived scope-link rules and skip words (see [`derived`]). Cheap
/// to construct; build once and share.
pub fn schema() -> Schema {
    let mut rows = entities::kinds();
    rows.extend(derived::derived_link_rules());
    schema_from_rows_with_skips(&rows, derived::derived_skip_words())
}

/// The hand-written rows only — the baseline the derivation proof harness
/// (`tests/derived_proof.rs`) measures against.
pub fn schema_hand_only() -> Schema {
    schema_from_rows_with_skips(&entities::kinds(), Vec::new())
}

/// Builds a schema from explicit rows and extra skip words.
pub(crate) fn schema_from_rows_with_skips(
    rows: &[pdxl_analysis::KindSpec],
    extra_skips: Vec<&'static str>,
) -> Schema {
    let mut skips: Vec<&'static str> = SCOPE_KEYWORDS.to_vec();
    skips.extend(extra_skips);
    let skips: &'static [&'static str] = Box::leak(skips.into_boxed_slice());
    let mut schema = Schema::new(
        rows,
        skips,
        &[], // no inline typed-def keywords observed yet
        KEYED_VALUE_DEFS,
        NESTED_VALUE_DEFS,
        &[], // no doc-ref aliases yet
        Some(CALL_KINDS),
        None,                      // .gui analysis off until the datafn registry exists
        Some(kinds::GAME_CONCEPT), // `[concept|e]` / `[Concept('concept')]`
    );
    schema.set_implicit_loc_patterns(&entities::implicit_loc_patterns());
    schema.set_loc_datafn_arg_refs(&entities::loc_datafn_arg_refs());
    schema
}
