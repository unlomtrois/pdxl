//! Title history (`history/titles/`) — dated holder/liege/government changes
//! per landed title. Like province history, the files **define nothing**:
//! every top-level key names a `common/landed_titles/` title, and every child
//! is a date block mutating it. There is no `_*.info` for this directory; the
//! corpus (game + T4N, 147 files) is the spec.
//!
//! References, all corpus-validated:
//! - the top-level `k_x = { … }` keys → titles
//!   ([`RefPattern::TopLevelBlockKeys`], the province-history precedent);
//! - inside a date block, structure-carried: `holder` /
//!   `holder_ignore_head_of_faith_requirement` (79k + 20 uses) → characters,
//!   `liege` / `de_jure_liege` and `tributary_of.suzerain` → titles,
//!   `government` → governments, `tributary_of.contract_group` → subject
//!   contract groups, `name` → a loc key renaming the title;
//! - `succession_laws = { X }` lists `common/laws/` keys — a gated `KeyList`,
//!   since the field is a loose list, not a keyed value.
//!
//! `holder = 0` (1439 uses) and `liege = 0` (1182) mean *vacate* — `0` joins
//! the game-wide `SCOPE_KEYWORDS` skip list in `lib.rs`, since nothing in the
//! corpus is ever named `0` (province ids start at 1, and numeric dynasty /
//! character ids never take it).
//!
//! `effect` runs with the title as root ([`block_scoped`] → `landed_title`),
//! matching vanilla usage (`set_title_name`, `set_title_color`, variables).
//!
//! Corpus at adoption: ~79k holder + 12.7k liege + 6.1k government + 1.5k
//! succession-law references extracted. Game + T4N resolves at 100%; vanilla
//! alone has **39 unresolved, all genuine bugs** — holders naming characters
//! `history/characters/` never defines (`karluk0033` where `karluk0032`
//! exists, the `304503`–`304511` block, `bobo0060`, `cisalpine0195`, …).

use pdxl_analysis::context::ClauseKind::{self, Config, Effect, Struct};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar};
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use crate::kinds;

use super::Entity;

const DIR: &str = "history/titles/";

/// `tributary_of = { suzerain = X contract_group = Y }` — a dated tributary
/// relationship.
static TRIBUTARY_OF: StructSpec = StructSpec {
    name: "tributary_of",
    fields: &[
        (
            "suzerain",
            scalar(Setting)
                .refs(kinds::TITLE)
                .doc("The title whose holder becomes the suzerain."),
        ),
        (
            "contract_group",
            scalar(Setting)
                .refs(kinds::SUBJECT_CONTRACT_GROUP)
                .doc("The subject-contract group the tributary relation uses."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one dated block (`1066.9.15 = { … }`).
static TITLE_DATE: StructSpec = StructSpec {
    name: "title history date",
    fields: &[
        (
            "holder",
            scalar(Setting)
                .refs(kinds::CHARACTER)
                .doc("The holder from this date; `0` vacates the title."),
        ),
        (
            "holder_ignore_head_of_faith_requirement",
            scalar(Setting)
                .refs(kinds::CHARACTER)
                .doc("As `holder`, bypassing the head-of-faith holding restriction."),
        ),
        (
            "liege",
            scalar(Setting)
                .refs(kinds::TITLE)
                .doc("The liege title from this date; `0` breaks the vassalage."),
        ),
        (
            "de_jure_liege",
            scalar(Setting)
                .refs(kinds::TITLE)
                .doc("Move the title under this de jure liege from this date."),
        ),
        (
            "government",
            scalar(Setting)
                .refs(kinds::GOVERNMENT)
                .doc("The holder's government form from this date."),
        ),
        (
            "succession_laws",
            block(Config).doc("Succession laws (`common/laws/`) active from this date."),
        ),
        (
            "change_development_level",
            scalar(Setting).doc("Set the county's development level at this date."),
        ),
        (
            "name",
            scalar(LocKey).doc("Rename the title from this date (a localization key)."),
        ),
        (
            "tributary_of",
            block(Struct(&TRIBUTARY_OF)).doc("Make the title's holder a tributary."),
        ),
        (
            "effect",
            block_scoped(Effect, "landed_title")
                .doc("Arbitrary effects run at this date; `root` is the title."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one `<title key> = { … }` entry: date blocks only.
static TITLE_HISTORY: StructSpec = StructSpec {
    name: "title history",
    fields: &[],
    // Every block-valued child is a date (`867.1.1 = { … }`).
    fallback: Fallback::Struct(&TITLE_DATE),
};

pub(crate) struct TitleHistory;

impl Entity for TitleHistory {
    const KINDS: &'static [KindSpec] = &[
        // The top-level keys reference titles; the dir defines nothing.
        KindSpec {
            kind: kinds::TITLE,
            icon: IconHint::Hierarchy,
            defs: None,
            refs: &[RefRule {
                pattern: RefPattern::TopLevelBlockKeys,
                gate: Some(DIR),
                alt: &[],
            }],
            aliases: &[],
        },
        // `succession_laws = { X Y }` — a loose list, so a gated rule rather
        // than a FieldSpec ref.
        KindSpec {
            kind: kinds::LAW,
            icon: IconHint::Tag,
            defs: None,
            refs: &[RefRule {
                pattern: RefPattern::KeyList("succession_laws"),
                gate: Some(DIR),
                alt: &[],
            }],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(DIR, Struct(&TITLE_HISTORY))];
}
