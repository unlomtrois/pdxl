//! History characters (`history/characters/`, from `_characters.info`) —
//! numeric- or string-ID definitions whose bodies mix static attributes with
//! dated event blocks (`1039.1.1 = { birth = … }`).
//!
//! Cross-references gated to this directory (all corpus-validated, ~25
//! genuinely dangling character IDs out of 85k refs in vanilla):
//! - `father` / `mother` and the spouse/concubine effects → characters;
//! - `trait` → traits (`add_trait`/`remove_trait` are already global rules);
//! - `culture`, `religion`, `faith`, `dynasty`, `dynasty_house` rules live
//!   with their target kinds (culture.rs / faith.rs / dynasty.rs).
//!
//! `employer` is documented but **not** a ref: `employer = 0` is a
//! "clear employer" sentinel (29 vanilla hits) that would be phantom noise.
//!
//! Date-block bodies fall back to **Effect** — history uses scripted effects
//! (`contract_disease_effect = …`) and builtins (`add_pressed_claim`) freely,
//! so completion/hover there behaves like an effect block with a few extra
//! historical fields (`birth`, `death`, `add_spouse`, …).

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Effect, Struct};
use pdxl_analysis::context::ScalarKind::{Setting, Target};
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::OPAQUE;

const CHARACTERS_DIR: &str = "history/characters/";

/// A `key = X` reference gated to the history-characters directory.
const fn in_history(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(CHARACTERS_DIR),
        alt: &[],
    }
}

/// A skill attribute (base value; random spread applies on top).
const fn skill(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc)
}

/// `death = { death_reason = … killer = … }` — the block form of `death`.
static DEATH: StructSpec = StructSpec {
    name: "death",
    fields: &[
        (
            "death_reason",
            scalar(Setting).doc("A death reason (`common/deathreasons/`), e.g. `death_murder`."),
        ),
        ("killer", scalar(Setting).doc("The killing character's ID.")),
    ],
    fallback: Fallback::Deny,
};

/// `portrait_override = { … }` — appearance override.
static PORTRAIT_OVERRIDE: StructSpec = StructSpec {
    name: "portrait_override",
    fields: &[
        (
            "portrait_modifier_overrides",
            block(Struct(&OPAQUE))
                .doc("`modifier_category = modifier` pairs, e.g. `clothes = western_low_nobles`."),
        ),
        (
            "hair",
            block(Struct(&OPAQUE)).doc("Hair color as `{ R G B }`, e.g. `{ 0.592 0.314 0.176 }`."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one dated block (`1039.1.1 = { … }`): historical fields plus
/// arbitrary effects run at that date.
static CHARACTER_DATE: StructSpec = StructSpec {
    name: "character date",
    fields: &[
        (
            "birth",
            scalar(Setting).doc("The character is born at this date (`yes` or the date itself)."),
        ),
        (
            "death",
            scalar_or_block(Setting, Struct(&DEATH)).doc(
                "The character dies at this date — `yes`, the date, or a block with \
                 `death_reason` and optional `killer`.",
            ),
        ),
        (
            "name",
            scalar(Setting).doc("Rename the character at this date."),
        ),
        (
            "employer",
            scalar(Setting).doc("Join this character's court (`0` to clear the employer)."),
        ),
        (
            "add_spouse",
            scalar(Setting).doc("Marry the given character at this date."),
        ),
        (
            "add_matrilineal_spouse",
            scalar(Setting).doc("Marry matrilineally at this date."),
        ),
        (
            "add_same_sex_spouse",
            scalar(Setting).doc("Same-sex marriage at this date."),
        ),
        (
            "add_concubine",
            scalar(Setting).doc("Take the given character as a concubine."),
        ),
        (
            "remove_spouse",
            scalar(Setting).doc("Divorce the given character."),
        ),
        (
            "trait",
            scalar(Setting).doc("Add this trait at this date (no on-gain effects)."),
        ),
        (
            "give_nickname",
            scalar(Setting).doc("Give this nickname (`common/nicknames/`)."),
        ),
        (
            "capital",
            scalar(Setting).doc("Set the capital to this barony title."),
        ),
        (
            "move_to_pool",
            scalar(Setting).doc("Move the character to the character pool."),
        ),
        ("dynasty", scalar(Setting).doc("Change the dynasty.")),
        (
            "dynasty_house",
            scalar(Setting).doc("Change the dynasty house."),
        ),
        ("religion", scalar(Setting).doc("Change the faith.")),
        ("faith", scalar(Setting).doc("Change the faith.")),
        ("culture", scalar(Setting).doc("Change the culture.")),
        (
            "effect",
            block(Effect).doc("Arbitrary effects run at this date (`root` is the character)."),
        ),
    ],
    // History freely mixes in scripted effects and builtins at the date level.
    fallback: Fallback::Effect,
};

/// The body of one character definition (`_characters.info`). Unknown
/// block-valued keys are dates opening [`CHARACTER_DATE`].
static CHARACTER: StructSpec = StructSpec {
    name: "character",
    fields: &[
        (
            "name",
            scalar(Setting).doc("The character's name (a name key or literal)."),
        ),
        ("dna", scalar(Setting).doc("A portrait DNA string ID.")),
        (
            "female",
            scalar(Setting)
                .doc("Whether the character is female (default `no`).")
                .values(&["yes", "no"]),
        ),
        ("martial", skill("Base martial skill.")),
        ("prowess", skill("Base prowess skill.")),
        ("diplomacy", skill("Base diplomacy skill.")),
        ("intrigue", skill("Base intrigue skill.")),
        ("stewardship", skill("Base stewardship skill.")),
        ("learning", skill("Base learning skill.")),
        (
            "trait",
            scalar(Setting).doc("A starting trait (repeatable)."),
        ),
        ("father", scalar(Setting).doc("The father's character ID.")),
        ("mother", scalar(Setting).doc("The mother's character ID.")),
        (
            "disallow_random_traits",
            scalar(Setting)
                .doc("Don't fill missing traits randomly at game start.")
                .values(&["yes", "no"]),
        ),
        (
            "religion",
            scalar(Setting).doc("The character's faith (legacy key; same as `faith`)."),
        ),
        ("faith", scalar(Setting).doc("The character's faith.")),
        ("culture", scalar(Setting).doc("The character's culture.")),
        ("dynasty", scalar(Setting).doc("The dynasty ID.")),
        (
            "dynasty_house",
            scalar(Setting).doc("The dynasty house (instead of `dynasty` for cadet houses)."),
        ),
        (
            "give_nickname",
            scalar(Setting).doc("A starting nickname (`common/nicknames/`)."),
        ),
        (
            "sexuality",
            scalar(Setting)
                .doc("The character's sexuality (random if unset).")
                .values(&["heterosexual", "homosexual", "bisexual", "asexual", "none"]),
        ),
        ("health", scalar(Setting).doc("Base health.")),
        ("fertility", scalar(Setting).doc("Base fertility.")),
        (
            "employer",
            scalar(Target).doc("The court this character starts in."),
        ),
        (
            "portrait_override",
            block(Struct(&PORTRAIT_OVERRIDE)).doc("Override the character's appearance."),
        ),
    ],
    // Unknown block-valued keys are dates (`1039.1.1 = { … }`).
    fallback: Fallback::Struct(&CHARACTER_DATE),
};

pub(crate) struct Character;

impl Entity for Character {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::CHARACTER,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: CHARACTERS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            in_history("father"),
            in_history("mother"),
            in_history("add_spouse"),
            in_history("add_matrilineal_spouse"),
            in_history("add_same_sex_spouse"),
            in_history("add_concubine"),
            in_history("remove_spouse"),
            // `death = { killer = X }` (488 corpus refs, 0 unresolved).
            in_history("killer"),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(CHARACTERS_DIR, ClauseKind::Struct(&CHARACTER))];
}
