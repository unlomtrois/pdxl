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
//! functions; behavior is identical (once oracle-checked against Go, now
//! pinned by golden snapshots), only ownership moved.
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
pub use kind::{CallKinds, GuiKinds, KindId, LOC_KEY, SCRIPT_CONSTANT};
pub use model::{CallTargets, FileFacts, Ref, Symbol};
pub use resolve::{RefDiag, merge_and_resolve, resolve_refs};
pub use schema::{DefRule, DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule, Schema};
pub use table::{Duplicate, SymbolTable};

/// Version of the fact extraction semantics **and** schema shape. A future
/// facts cache must embed this in its keys (alongside content hash and
/// rel_path) and treat mismatches as misses; bump it whenever extraction rules
/// or the [`FileFacts`] model change meaning.
pub const ANALYSIS_VERSION: u32 = 60; // 60: EU5 country domain (setup/countries + formable tag aliases + c: literals, cultures/religions/named colors/description categories def kinds + gated refs, documented country body); 59: game rules (common/game_rules/ GroupedBlocks setting defs + has_game_rule/default refs + player/ai/all apply_modifier literals to static modifiers + documented bodies); 58: religion localization maps as loc refs (new KeyBlockValues shape — dynamic keys, scalar or listed loc-key values, gated to common/religion/); 57: religion domain schematized (doctrines/holy sites/religion families as kinds with defs + ungated/gated refs; full religion+faith bodies from _religion_types.info; religious_head/county/barony title refs; virtues/sins trait refs); 56: partial religion/faith structural context (faiths container + faith color as ClauseKind::Color for the LSP swatch); 55: named colors (common/named_colors/ ChildrenOf defs + gated color/color1-5 refs + Fallback::Color container context; coa slot self-refs and list selector skipped); 54: ClauseKind::Color + color() field helper ({ r g b } / hsv / rgb / hsv360 blocks or a named color; adopted by terrain, culture, culture_pillar, situation map_color); 53: file-local script constants (@name = value defs + @name refs, per-file resolution + LSP nav/rename; @ values excluded from key-rule refs and TopLevelValued defs); 52: terrain types (common/terrain_types/ defs + ungated terrain = X refs + documented body from _terrains.info); 51: province-history culture/religion/faith refs (gated to history/provinces/, rules live with their target kinds); 50: provinces (map_data/definition.csv IdCsv defs + history/provinces top-level-key refs + landed_titles province ref + province: scope literal + documented province-history body); 49: scheme text fields as loc refs (desc/success_desc/discovery_desc gated to scheme_types); 48: scheme entity extended (corpus-only fields on_start/phases_per_agent_charge/discovery_desc/base_maximum_success/starting_agent_slots + skill/category/target_type enums + yes/no toggles); 47: situations (common/situation/{situations,catalysts,situation_group_types} defs + situation: scope/situation_type refs + gated situation_group_type/catalyst refs + documented bodies); 46: loc-layer game-concept refs ([concept|E] encyclopedia links + [Concept('key',…)] in localization text); 45: game concepts (common/game_concepts/ defs + alias-list resolvable names + gated parent refs + documented body); 44: trait entity extended (documented body + Fallback::Modifier, opposites/compatibility refs via KeyBlockKeys); 43: gui text/tooltip loc-key refs (GuiKinds::loc_fields); 42: ByKey/WithKey datafn arg refs (GetDecisionWithKey/GetTitleByKey/GetCultureByKey/GetFaithByKey); 41: scripted GUIs (common/scripted_guis/ defs + body) + gui datafn argument refs (GetScriptedGui/ScriptValue/Custom → symbols); 40: multi-kind references (RefRule::alt) + custom_description text → trigger_loc|effect_loc|loc_key; 39: effect/trigger localization (defs both dirs + person/tense loc-key refs + documented body); 38: buildings (common/buildings/ defs + next_building/has_building/add_building/unlock_building/province-history refs + documented body); 37: customizable localization (common/customizable_localization/ defs + gated parent refs + documented body); 36: schema-coverage survey (coverage module + schema-gaps bin); 35: gui property docs (curated CK3/EU5-wiki table, hover + completion); 34: gui completion (corpus-mined widget properties + value enums, using targets, datafn roots/members); 33: gui semantic tokens (dialect keywords, template/type names+bases, datafn segments, instantiation refs); 32: gui datafunction typing (DumpDataTypes registry, chain validation, hover); 31: interface scripts (.gui dialect parsing, gui_template/gui_type symbols, name-gated using/base/instantiation refs); 30: culture domain (pillars/traditions/eras/innovations/name lists defs + refs, def-only aesthetics bundles/creation names/name equivalencies, documented culture bodies, unlock_casus_belli/unlock_law refs); 29: death reasons (common/deathreasons/ defs + death_reason refs + slot ref + history killer ref); 28: nicknames (common/nicknames/ defs + give_nickname/has_nickname refs + body context); 27: history characters extended (trait/culture/religion/faith/father/mother/spouse refs, dated-block Effect context) + dynasties/houses (defs + dynasty/dynasty_house/culture refs); 26: casus belli (common/casus_belli_types/ + _groups/ defs, casus_belli/cb/using_cb refs, gated group ref, documented bodies); 25: enum-valued struct fields (FieldSpec::values — slot types, slot category, artifact rarity, history entry types; value completion + hover); 24: artifacts (common/artifacts/{types,templates,visuals,features,feature_groups,blueprints,slots} defs + create_artifact/blueprint/feature-group refs + body contexts + create_artifact effect struct); 23: interaction text fields as loc refs (desc/notification_text/…); 22: interaction categories (common/character_interaction_categories/ defs + gated category ref); 21: character interactions (common/character_interactions/ defs + interaction refs + body context); 20: KindId open registry (dump order = registration order, loc_key last); 19: secret types (common/secret_types/ defs + *_secret type refs + body context); 18: namespace declarations (namespace = X keyed-value defs); 17: scripted character templates (common/scripted_character_templates/ defs + create_character template ref); 16: trait refs in add_trait_xp/has_trait_xp block (trait = X); 15: portrait animations (gfx/portraits/portrait_animations/ defs + events-gated animation refs); 14: script values (common/script_values/ scalar|block defs + value-position name-gated refs); 13: static modifiers (common/modifiers/ defs + add_*_modifier refs); 12: typed-def keywords (scripted_effect/trigger NAME = {}) + call-by-name refs; 11: event themes (top-level defs + theme reference); 10: scalar override_background = X reference; 9: event backgrounds (top-level defs + background reference); 8: schemes (top-level defs + scheme-type refs); 7: laws (grouped-block defs + realm-law refs); 6: loc-key symbols + text-field refs; 5: full on_action refs; 4: nested faith defs; 3: gated capital→title refs; 2: landed titles
