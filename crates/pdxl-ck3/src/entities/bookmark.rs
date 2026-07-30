//! Bookmarks (`common/bookmarks/`) — the start-screen entries: `bookmarks/`
//! (the selectable starts), `groups/` (start-date groupings) and
//! `challenge_characters/` (the achievement-hunting picks). From
//! `_bookmarks.info`, `_bookmark_groups.info` and
//! `_challenge_characters.info` — though all three are thin, and the
//! character block's doc is literally "See 'character = {}' in
//! 00_bookmarks.txt", so the corpus is the spec here.
//!
//! No engine surface at all: no trigger, effect, scope link or datafunction
//! takes a bookmark key (the frontend consumes them before a game state
//! exists), so every reference is structure-carried — this directory is the
//! `FieldSpec::ref_kind` showcase. A bookmark character resolves eight ways:
//! `history_id` → character, `dynasty` (numeric id *or* key) → dynasty,
//! `dynasty_house` → house, `title` → title, `government` /
//! `fallback_government` → government, `culture` → culture, `religion` → a
//! *faith* key despite the name, and `name` / `relation` / `difficulty` →
//! quoted loc keys (the extractor trims quotes before resolving).
//!
//! The character block recurses: alternate characters (`relation = X`) nest
//! inside their parent, so the spec references itself.
//!
//! Corpus-only fields the infos never mention: the whole portrait side of
//! the character block (`type`, `birth`, `animation`, `position`,
//! `difficulty`, `relation`, `dynasty_splendor_level`, `display`),
//! `fallback_government` (5), bookmark `test_default` (2), the challenge
//! character's `achievements` list (52/53 defs), and the character-designer
//! quartet (`character_design_type`, `target_title`, `tutorial`,
//! `title_text_override`, 2 uses each). The info's `bookmark_type` exists
//! but has just 2 uses. Designer starts put the `BOOKMARK_CHARACTER_ANY_FAITH`
//! / `_ANY_CULTURE` loc-key sentinels in `religion` / `culture`, so both
//! fields resolve through a faith/culture-then-loc alt chain.
//!
//! Implicit localization (measured): bookmarks and challenge characters use
//! `<key>` + `<key>_desc` (19/19 and 52/52); groups use bare `<key>` (3/3,
//! per the info: "key of the bookmark is used as localization tag").
//!
//! Deliberate omission: `achievements` entries name `common/achievements/`
//! defs — a real directory this schema does not model yet; the list stays
//! plain data until an achievement kind exists (then: `KeyList` gated here,
//! or a `ref_kind` on a modeled list shape).

use pdxl_analysis::context::ClauseKind::{self, Config, ScriptValue, Struct};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec};

use crate::kinds;

use super::Entity;

const BOOKMARKS_DIR: &str = "common/bookmarks/bookmarks/";
const GROUPS_DIR: &str = "common/bookmarks/groups/";
const CHALLENGE_DIR: &str = "common/bookmarks/challenge_characters/";

/// A `yes`/`no` toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// A bookmark character — the block is shared by bookmarks and challenge
/// characters, and nests recursively for the alternate characters shown
/// beside the main one.
static BOOKMARK_CHARACTER: StructSpec = StructSpec {
    name: "bookmark_character",
    fields: &[
        (
            "name",
            scalar(LocKey).doc("Localization key for the character's display name."),
        ),
        (
            "relation",
            scalar(LocKey).doc(
                "Label for an alternate character's relation to the main one \
                 (`BOOKMARK_RELATION_*` loc keys) *(corpus)*.",
            ),
        ),
        (
            "history_id",
            scalar(Setting)
                .refs(kinds::CHARACTER)
                .doc("The historical character this entry plays as."),
        ),
        (
            "bookmark_type",
            scalar(Setting)
                .doc(
                    "What the character starts as. The landless kinds read their start \
                     location from `title`. Default `existing_ruler`.",
                )
                .values(&[
                    "existing_ruler",
                    "new_landless_adventurer",
                    "new_noble_family",
                ]),
        ),
        (
            "type",
            scalar(Setting)
                .doc("Portrait body type *(corpus)*.")
                .values(&["male", "female", "boy", "girl"]),
        ),
        ("birth", scalar(Setting).doc("Birth date *(corpus)*.")),
        (
            "dynasty",
            scalar(Setting)
                .refs(kinds::DYNASTY)
                .doc("Dynasty, by numeric id or key *(corpus)*."),
        ),
        (
            "dynasty_house",
            scalar(Setting)
                .refs(kinds::DYNASTY_HOUSE)
                .doc("House, where it differs from the dynasty *(corpus)*."),
        ),
        (
            "dynasty_splendor_level",
            scalar(Setting).doc("Splendor level shown for the dynasty coat of arms *(corpus)*."),
        ),
        (
            "title",
            scalar(Setting).refs(kinds::TITLE).doc(
                "Primary title — or, for the landless bookmark types, the starting \
                 location *(corpus)*.",
            ),
        ),
        (
            "government",
            scalar(Setting)
                .refs(kinds::GOVERNMENT)
                .doc("Government shown on the start screen *(corpus)*."),
        ),
        (
            "fallback_government",
            scalar(Setting)
                .refs(kinds::GOVERNMENT)
                .doc("Government used when the primary one is DLC-locked *(corpus)*."),
        ),
        (
            "culture",
            scalar(Setting)
                .refs_any(kinds::CULTURE, &[kinds::LOC_KEY])
                .doc(
                    "Culture — or, for character-designer starts, the \
                     `BOOKMARK_CHARACTER_ANY_CULTURE` loc-key sentinel *(corpus)*.",
                ),
        ),
        (
            "religion",
            scalar(Setting)
                .refs_any(kinds::FAITH, &[kinds::LOC_KEY])
                .doc(
                    "A *faith* key, despite the field name — or the \
                     `BOOKMARK_CHARACTER_ANY_FAITH` loc-key sentinel *(corpus)*.",
                ),
        ),
        (
            "character_design_type",
            scalar(Setting)
                .doc(
                    "Opens the character designer for a new-family start; `target_title` \
                     and `title` configure it *(corpus)*.",
                )
                .values(&["noble_family"]),
        ),
        (
            "target_title",
            scalar(Setting)
                .refs(kinds::TITLE)
                .doc("The realm a designed noble family starts under *(corpus)*."),
        ),
        (
            "tutorial",
            toggle("Marks the tutorial start (see the tutorial-lesson files) *(corpus)*."),
        ),
        (
            "title_text_override",
            scalar(LocKey).doc(
                "Loc key overriding the displayed title name (`\"WEST_FRANCIA\"`) *(corpus)*.",
            ),
        ),
        (
            "difficulty",
            scalar(LocKey)
                .doc("Difficulty label shown under the portrait *(corpus)*.")
                .values(&[
                    "BOOKMARK_CHARACTER_DIFFICULTY_EASY",
                    "BOOKMARK_CHARACTER_DIFFICULTY_MEDIUM",
                    "BOOKMARK_CHARACTER_DIFFICULTY_HARD",
                    "BOOKMARK_CHARACTER_DIFFICULTY_VERY_HARD",
                ]),
        ),
        (
            "position",
            block(Config).doc("`{ x y }` position of the portrait on the map *(corpus)*."),
        ),
        (
            "animation",
            scalar(Setting).doc("Portrait animation on the start screen *(corpus)*."),
        ),
        (
            "display",
            toggle("Show this character on the start screen. Default `yes` *(corpus)*."),
        ),
        (
            "character",
            block(Struct(&BOOKMARK_CHARACTER))
                .doc("An alternate character shown beside this one; carries `relation`."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one bookmark.
static BOOKMARK: StructSpec = StructSpec {
    name: "bookmark",
    fields: &[
        (
            "start_date",
            scalar(Setting)
                .doc("Start date; defaults to the group's `default_start_date` when grouped."),
        ),
        (
            "is_playable",
            toggle("Is this bookmark playable? Default `yes`."),
        ),
        (
            "recommended",
            toggle("Show the bookmark as recommended. Default `no`."),
        ),
        (
            "test_default",
            toggle("Marks the bookmark test builds default to *(corpus)*."),
        ),
        (
            "group",
            scalar(Setting)
                .refs(kinds::BOOKMARK_GROUP)
                .doc("The bookmark group this start belongs to; empty means ungrouped."),
        ),
        (
            "requires_dlc_flag",
            scalar(Setting).doc("DLC feature flag that must be active for the bookmark to show."),
        ),
        (
            "weight",
            block(ScriptValue).doc(
                "Weight for being the default bookmark (highest wins). Runs before a \
                 game state exists, so no gamestate triggers. Default -1.",
            ),
        ),
        (
            "character",
            block(Struct(&BOOKMARK_CHARACTER)).doc("A playable start; repeat for each portrait."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one bookmark group.
static BOOKMARK_GROUP: StructSpec = StructSpec {
    name: "bookmark_group",
    fields: &[(
        "default_start_date",
        scalar(Setting).doc("Start date bookmarks of this group use unless they set their own."),
    )],
    fallback: Fallback::Deny,
};

/// The body of one challenge character.
static CHALLENGE_CHARACTER: StructSpec = StructSpec {
    name: "challenge_character",
    fields: &[
        ("start_date", scalar(Setting).doc("The game start date.")),
        (
            "achievements",
            block(Config).doc(
                "Achievements this start is suited for (`common/achievements/` keys) \
                 *(corpus)*.",
            ),
        ),
        (
            "character",
            block(Struct(&BOOKMARK_CHARACTER)).doc("The character, exactly as in a bookmark."),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Bookmark;

impl Entity for Bookmark {
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::BOOKMARK,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::BOOKMARK,
            suffix: "_desc",
        },
        ImplicitLocPattern {
            kind: kinds::BOOKMARK_GROUP,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::CHALLENGE_CHARACTER,
            suffix: "",
        },
        ImplicitLocPattern {
            kind: kinds::CHALLENGE_CHARACTER,
            suffix: "_desc",
        },
    ];

    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::BOOKMARK,
            icon: IconHint::Event,
            defs: Some(DefSource {
                dir_prefix: BOOKMARKS_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::BOOKMARK_GROUP,
            icon: IconHint::Hierarchy,
            defs: Some(DefSource {
                dir_prefix: GROUPS_DIR,
                shape: DefShape::TopLevel,
            }),
            // The one reference is `group = X`, carried by the bookmark body.
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::CHALLENGE_CHARACTER,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: CHALLENGE_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (BOOKMARKS_DIR, Struct(&BOOKMARK)),
        (GROUPS_DIR, Struct(&BOOKMARK_GROUP)),
        (CHALLENGE_DIR, Struct(&CHALLENGE_CHARACTER)),
    ];
}
