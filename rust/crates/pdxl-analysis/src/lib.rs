//! Per-file semantic fact extraction over `pdxl-ast` trees.
//!
//! Port of the facts half of `internal/validate`: one AST walk distills a
//! parsed file into [`FileFacts`] — the definitions it declares, the alias
//! names it exposes, and the references it makes. Facts are deterministic from
//! the file's content **and path** (directory location decides what a
//! definition means), small, and independent per file; that is what makes the
//! whole-project analysis incremental: replace one file's facts, rebuild the
//! table from all facts (Milestone 6).
//!
//! This crate is game-agnostic: every game-specific decision (which
//! directories define what, which keys are references, which values to skip)
//! arrives as data in a [`Schema`], supplied by a rules crate such as
//! `pdxl-ck3`. The Go implementation hardcodes these in its extraction
//! functions; behavior is identical (oracle-checked by `pdxl-parity`), only
//! ownership moved.
//!
//! Deliberate deviation from Go, per the project's measured-simplification
//! plan: the on-disk `FactStore` is **not** ported in this milestone. Facts
//! are cheap to re-extract (one allocation-light tree walk), and the cold-path
//! benchmark decides whether a facts cache ever earns its complexity.

pub mod context;
mod extract;
mod kind;
mod model;
mod resolve;
mod schema;
mod table;

pub use extract::{extract_calls, extract_facts};
pub use kind::{CallKinds, KindId, LOC_KEY};
pub use model::{CallTargets, FileFacts, Ref, Symbol};
pub use resolve::{RefDiag, merge_and_resolve, resolve_refs};
pub use schema::{DefRule, DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule, Schema};
pub use table::{Duplicate, SymbolTable};

/// Version of the fact extraction semantics **and** schema shape. A future
/// facts cache must embed this in its keys (alongside content hash and
/// rel_path) and treat mismatches as misses; bump it whenever extraction rules
/// or the [`FileFacts`] model change meaning.
pub const ANALYSIS_VERSION: u32 = 30; // 30: culture domain (pillars/traditions/eras/innovations/name lists defs + refs, def-only aesthetics bundles/creation names/name equivalencies, documented culture bodies, unlock_casus_belli/unlock_law refs); 29: death reasons (common/deathreasons/ defs + death_reason refs + slot ref + history killer ref); 28: nicknames (common/nicknames/ defs + give_nickname/has_nickname refs + body context); 27: history characters extended (trait/culture/religion/faith/father/mother/spouse refs, dated-block Effect context) + dynasties/houses (defs + dynasty/dynasty_house/culture refs); 26: casus belli (common/casus_belli_types/ + _groups/ defs, casus_belli/cb/using_cb refs, gated group ref, documented bodies); 25: enum-valued struct fields (FieldSpec::values — slot types, slot category, artifact rarity, history entry types; value completion + hover); 24: artifacts (common/artifacts/{types,templates,visuals,features,feature_groups,blueprints,slots} defs + create_artifact/blueprint/feature-group refs + body contexts + create_artifact effect struct); 23: interaction text fields as loc refs (desc/notification_text/…); 22: interaction categories (common/character_interaction_categories/ defs + gated category ref); 21: character interactions (common/character_interactions/ defs + interaction refs + body context); 20: KindId open registry (dump order = registration order, loc_key last); 19: secret types (common/secret_types/ defs + *_secret type refs + body context); 18: namespace declarations (namespace = X keyed-value defs); 17: scripted character templates (common/scripted_character_templates/ defs + create_character template ref); 16: trait refs in add_trait_xp/has_trait_xp block (trait = X); 15: portrait animations (gfx/portraits/portrait_animations/ defs + events-gated animation refs); 14: script values (common/script_values/ scalar|block defs + value-position name-gated refs); 13: static modifiers (common/modifiers/ defs + add_*_modifier refs); 12: typed-def keywords (scripted_effect/trigger NAME = {}) + call-by-name refs; 11: event themes (top-level defs + theme reference); 10: scalar override_background = X reference; 9: event backgrounds (top-level defs + background reference); 8: schemes (top-level defs + scheme-type refs); 7: laws (grouped-block defs + realm-law refs); 6: loc-key symbols + text-field refs; 5: full on_action refs; 4: nested faith defs; 3: gated capital→title refs; 2: landed titles
