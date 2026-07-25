//! The culture–language domain: cultures (`in_game/common/cultures/`, 2,087
//! defs, documented by `00_cultures.info`), culture groups
//! (`in_game/common/culture_groups/`, 209 defs, `00_culture_groups.info`),
//! languages (`in_game/common/languages/`, 528 top-level defs plus nested
//! `dialects = { X = {} }` children), and language families
//! (`in_game/common/language_families/`, 56 defs).
//!
//! References (corpus-validated): `culture_definition = X` (2,337, ungated);
//! `culture_groups = { … }` list items (cultures only) and
//! `has_culture_group = X` (968, ungated); `language = X` (ungated — the
//! `language:`/`scope:` chains skip for free); `family = X` (348, gated to
//! languages) and `language_family = X` (66, ungated). The `culture:`,
//! `culture_group:`, and `language:` scope literals are table-derived
//! (`crate::derived`); the `color = X` named-color rules live in
//! [`super::named_color`].
//!
//! `tags = { … }` items and `opinions = { <tag> = <stance> }` keys are the
//! per-culture gfx-tag vocabulary — declared nowhere, so unmodeled;
//! name-list items (`male_names`, `dynasty_names`, …) are raw names.
//! Accepted noise: 3 duplicate-definition warnings where a dialect is named
//! identically to its parent language (greek/hindustani/french — real game
//! data shape).

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, StaticModifier, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, color, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::scripted::def_only;

pub(crate) const CULTURES_DIR: &str = "in_game/common/cultures/";
pub(crate) const CULTURE_GROUPS_DIR: &str = "in_game/common/culture_groups/";
pub(crate) const LANGUAGES_DIR: &str = "in_game/common/languages/";
pub(crate) const LANGUAGE_FAMILIES_DIR: &str = "in_game/common/language_families/";

/// A yes/no toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// An ungated `key = X` reference.
const fn keyval(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: None,
        alt: &[],
    }
}

/// A list/map body whose items carry no schema (gfx tags, raw names).
static VOCAB: StructSpec = StructSpec {
    name: "vocabulary list",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// The body of one culture (`00_cultures.info` + corpus).
static CULTURE: StructSpec = StructSpec {
    name: "culture",
    fields: &[
        ("language", scalar(Setting).doc("The culture's language or dialect (`in_game/common/languages/`).")),
        ("color", color().doc("Jomini color — a named color or literal.")),
        ("tags", block(Struct(&VOCAB)).doc("Gfx tags (`catalan_gfx swedish_gfx …`) — free vocabulary, also matched by other cultures' opinions.")),
        ("culture_groups", block(Struct(&VOCAB)).doc("Culture groups this culture belongs to (more unique ones first).")),
        ("opinions", block(Struct(&VOCAB)).doc("Stance towards gfx tags: `<tag> = enemy|negative|neutral|positive|kindred`.")),
        ("country_modifier", block(StaticModifier).doc("Modifier when a country's primary culture is this.")),
        ("location_modifier", block(StaticModifier).doc("Modifier when a location's dominant culture is this.")),
        ("character_modifier", block(StaticModifier).doc("Modifier when a character's culture is this.")),
        ("goods_demand_modifier", block(StaticModifier).doc("Per-good pop-demand modifiers for pops of this culture. *(corpus)*")),
        ("suppress_no_pops_error", toggle("For cultures deliberately without startup pops (historical/future ones); default no.")),
        ("use_patronym", toggle("Characters of this culture use patronyms. *(corpus)*")),
        ("dynasty_name_type", scalar(Setting).doc("Dynasty-naming style override. *(corpus)*")),
        ("adjective_keys", block(Struct(&VOCAB)).doc("Loc-key adjectives override. *(corpus)*")),
        ("noun_keys", block(Struct(&VOCAB)).doc("Loc-key nouns override. *(corpus)*")),
        ("active", toggle("Whether the culture is active. *(corpus)*")),
    ],
    fallback: Fallback::Deny,
};

/// The body of one culture group (`00_culture_groups.info` + corpus).
static CULTURE_GROUP: StructSpec = StructSpec {
    name: "culture group",
    fields: &[
        (
            "country_modifier",
            block(StaticModifier).doc("Modifier when the primary culture belongs to this group."),
        ),
        (
            "location_modifier",
            block(StaticModifier).doc("Modifier when the dominant culture belongs to this group."),
        ),
        (
            "character_modifier",
            block(StaticModifier)
                .doc("Modifier when the character's culture belongs to this group."),
        ),
        (
            "goods_demand_modifier",
            block(StaticModifier)
                .doc("Per-good pop-demand modifiers for pops of this group. *(corpus)*"),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `dialects = { <name> = { …name lists… } }` — nested dialect defs.
static DIALECTS: StructSpec = StructSpec {
    name: "dialects",
    fields: &[],
    fallback: Fallback::Ignore,
};

/// A raw-name list field.
const fn names(doc: &'static str) -> FieldSpec {
    block(Struct(&VOCAB)).doc(doc)
}

/// The body of one language (corpus-documented; no `.info` ships).
static LANGUAGE: StructSpec = StructSpec {
    name: "language",
    fields: &[
        (
            "color",
            color().doc("Map color — a named color or literal."),
        ),
        (
            "family",
            scalar(Setting).doc("The language family (`in_game/common/language_families/`)."),
        ),
        (
            "fallback",
            scalar(Setting).doc("Fallback language for missing name lists. *(corpus)*"),
        ),
        (
            "dialects",
            block(Struct(&DIALECTS))
                .doc("Nested dialect definitions (usable wherever a language is)."),
        ),
        ("male_names", names("Male first names.")),
        ("female_names", names("Female first names.")),
        ("dynasty_names", names("Dynasty names.")),
        ("lowborn", names("Lowborn dynasty names.")),
        ("ship_names", names("Ship names. *(corpus)*")),
        (
            "dynasty_template_keys",
            names("Dynasty template keys. *(corpus)*"),
        ),
        (
            "character_name_order",
            scalar(Setting).doc("Name ordering style. *(corpus)*"),
        ),
        (
            "character_name_short_regnal_number",
            scalar(Setting).doc("Short regnal-number style. *(corpus)*"),
        ),
        (
            "first_name_conjoiner",
            scalar(Setting).doc("Conjoiner between first names. *(corpus)*"),
        ),
        (
            "patronym_prefix_son",
            scalar(Setting).doc("Patronym prefix (sons). *(corpus)*"),
        ),
        (
            "patronym_prefix_son_vowel",
            scalar(Setting).doc("Patronym prefix before vowels (sons). *(corpus)*"),
        ),
        (
            "patronym_prefix_daughter",
            scalar(Setting).doc("Patronym prefix (daughters). *(corpus)*"),
        ),
        (
            "patronym_prefix_daughter_vowel",
            scalar(Setting).doc("Patronym prefix before vowels (daughters). *(corpus)*"),
        ),
        (
            "patronym_suffix",
            scalar(Setting).doc("Patronym suffix. *(corpus)*"),
        ),
        (
            "patronym_suffix_son",
            scalar(Setting).doc("Patronym suffix (sons). *(corpus)*"),
        ),
        (
            "patronym_suffix_daughter",
            scalar(Setting).doc("Patronym suffix (daughters). *(corpus)*"),
        ),
        (
            "descendant_prefix",
            scalar(Setting).doc("Descendant dynasty prefix. *(corpus)*"),
        ),
        (
            "descendant_prefix_male",
            scalar(Setting).doc("Descendant prefix (male). *(corpus)*"),
        ),
        (
            "descendant_prefix_female",
            scalar(Setting).doc("Descendant prefix (female). *(corpus)*"),
        ),
        (
            "descendant_suffix",
            scalar(Setting).doc("Descendant dynasty suffix. *(corpus)*"),
        ),
        (
            "descendant_suffix_male",
            scalar(Setting).doc("Descendant suffix (male). *(corpus)*"),
        ),
        (
            "descendant_suffix_female",
            scalar(Setting).doc("Descendant suffix (female). *(corpus)*"),
        ),
        (
            "location_prefix",
            scalar(Setting).doc("Location-name prefix. *(corpus)*"),
        ),
        (
            "location_prefix_vowel",
            scalar(Setting).doc("Location-name prefix before vowels. *(corpus)*"),
        ),
        (
            "location_prefix_elision",
            scalar(Setting).doc("Location-name prefix with elision. *(corpus)*"),
        ),
        (
            "location_prefix_ancient",
            scalar(Setting).doc("Ancient location-name prefix. *(corpus)*"),
        ),
        (
            "location_prefix_ancient_vowel",
            scalar(Setting).doc("Ancient location-name prefix before vowels. *(corpus)*"),
        ),
        (
            "location_suffix",
            scalar(Setting).doc("Location-name suffix. *(corpus)*"),
        ),
        (
            "require_genitive_location_names",
            toggle("Location names require the genitive. *(corpus)*"),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one language family (corpus: color only).
static LANGUAGE_FAMILY: StructSpec = StructSpec {
    name: "language family",
    fields: &[(
        "color",
        color().doc("Map color — a named color or literal."),
    )],
    fallback: Fallback::Deny,
};

pub(crate) struct Culture;

impl Entity for Culture {
    const KINDS: &'static [KindSpec] = &[
        // The `culture:` literal is table-derived (`crate::derived`).
        KindSpec {
            refs: &[keyval("culture_definition")],
            ..def_only(kinds::CULTURE, IconHint::Object, CULTURES_DIR)
        },
        // The `culture_group:` literal is table-derived.
        KindSpec {
            refs: &[
                keyval("has_culture_group"),
                RefRule {
                    pattern: RefPattern::KeyList("culture_groups"),
                    gate: Some(CULTURES_DIR),
                    alt: &[],
                },
            ],
            ..def_only(
                kinds::CULTURE_GROUP,
                IconHint::Hierarchy,
                CULTURE_GROUPS_DIR,
            )
        },
        // Languages: top-level defs plus nested dialect children (usable
        // wherever a language is). The `language:` literal is table-derived.
        KindSpec {
            refs: &[keyval("language")],
            ..def_only(kinds::LANGUAGE, IconHint::Tag, LANGUAGES_DIR)
        },
        KindSpec {
            kind: kinds::LANGUAGE,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: LANGUAGES_DIR,
                shape: DefShape::ChildrenOf {
                    containers: &["dialects"],
                },
            }),
            refs: &[],
            aliases: &[],
        },
        KindSpec {
            refs: &[
                keyval("language_family"),
                RefRule {
                    pattern: RefPattern::KeyValue("family"),
                    gate: Some(LANGUAGES_DIR),
                    alt: &[],
                },
            ],
            ..def_only(
                kinds::LANGUAGE_FAMILY,
                IconHint::Hierarchy,
                LANGUAGE_FAMILIES_DIR,
            )
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (CULTURES_DIR, ClauseKind::Struct(&CULTURE)),
        (CULTURE_GROUPS_DIR, ClauseKind::Struct(&CULTURE_GROUP)),
        (LANGUAGES_DIR, ClauseKind::Struct(&LANGUAGE)),
        (LANGUAGE_FAMILIES_DIR, ClauseKind::Struct(&LANGUAGE_FAMILY)),
    ];
}
