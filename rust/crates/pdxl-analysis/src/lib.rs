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
mod model;
mod resolve;
mod schema;
mod table;

pub use extract::extract_facts;
pub use model::{FileFacts, Ref, Symbol, SymbolKind};
pub use resolve::{RefDiag, merge_and_resolve, resolve_refs};
pub use schema::{DefRule, DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule, Schema};
pub use table::{Duplicate, SymbolTable};

/// Version of the fact extraction semantics **and** schema shape. A future
/// facts cache must embed this in its keys (alongside content hash and
/// rel_path) and treat mismatches as misses; bump it whenever extraction rules
/// or the [`FileFacts`] model change meaning.
pub const ANALYSIS_VERSION: u32 = 5; // 5: full on_action refs (fire lists, fallback, trigger_event on_action); 4: nested faith defs; 3: gated capital→title refs; 2: landed titles
