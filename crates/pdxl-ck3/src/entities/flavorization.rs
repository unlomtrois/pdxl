//! Flavorization (`common/flavorization/`, from `_flavourization.info` — the
//! directory and the info spell it differently) — the contextual naming layer:
//! which word a character's title, a title's rank, or a domicile uses
//! (Emperor vs. High King, Duchy vs. Petty Kingdom).
//!
//! One kind, no engine surface at all — no trigger, effect, scope link or
//! datafunction takes a flavorization key. A definition is pure *conditions*;
//! when they pass, the key localizes itself ("the flavourization type will
//! localize a localization key of the same name"), so the bare `<key>`
//! implicit-loc pattern is the entire text side — and it is universal
//! (400/400 sampled).
//!
//! References (corpus-validated over game + T4N, 1539 defs): the condition
//! lists resolve through gated `KeyList` rules — `governments` (1306 uses) →
//! governments, `heritages` (427) → culture pillars, `name_lists` (320) →
//! name lists, `religions` (264) → religions, `faiths` (18) → faiths,
//! `titles` (97) and `de_jure_liege` (6) → landed titles. The scalar fields
//! carry their own refs: `domicile_type` (34), `holding` (17), and the
//! corpus-only singular `faith = catholic` (1 use, resolves).
//!
//! Where the info and the corpus disagree: `tier` accepts `hegemony`
//! *(corpus)* on top of the documented values (whose list misspells duchy as
//! "douchy").
//!
//! Deliberate omissions: `council_position` names a
//! `common/council_positions/` def — a database this schema does not model
//! yet; `flag` and `subject_contract_obligation_flags` are flag namespaces
//! (set on contracts and tax slots), not symbol references.

use pdxl_analysis::context::ClauseKind::{self, Config, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern, RefRule,
};

use crate::kinds;

use super::Entity;
use super::common::toggle;

const DIR: &str = "common/flavorization/";

/// A condition-list reference rule gated to the flavorization files.
const fn in_flavorization(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(DIR),
        alt: &[],
    }
}

/// `flavourization_rules = { … }` — the engine-evaluated rule bitmask.
static RULES: StructSpec = StructSpec {
    name: "flavourization_rules",
    fields: &[
        (
            "faction",
            toggle(
                "Apply to faction leaders; the causing title must have a faction. \
                 Default `no`.",
            ),
        ),
        (
            "only_independent",
            toggle("Apply only if the context character is independent. Default `no`."),
        ),
        (
            "spouse_takes_title",
            toggle("The context character's spouse is also valid. Default `yes`."),
        ),
        (
            "only_holder",
            toggle(
                "Apply only when the evaluated character *is* the context character — \
                 the holder of the causing title. Default `no`.",
            ),
        ),
        (
            "top_liege",
            toggle(
                "Run every context-character test against the top liege instead. \
                 Default `yes`.",
            ),
        ),
        (
            "only_vassals",
            toggle("Apply only if the evaluated character is not independent. Default `no`."),
        ),
        (
            "ignore_top_liege_government",
            toggle(
                "With `top_liege`, check government against the evaluated character \
                 rather than the top liege. Default `no`.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one flavorization: conditions on the character, context
/// character, and causing title. The key doubles as the localization key.
static FLAVORIZATION: StructSpec = StructSpec {
    name: "flavorization",
    fields: &[
        (
            "type",
            scalar(Setting)
                .doc("What the flavorization applies to.")
                .values(&["character", "title", "domicile"]),
        ),
        (
            "gender",
            scalar(Setting)
                .doc("Which gender it applies to (`type = character` only).")
                .values(&["male", "female"]),
        ),
        (
            "tier",
            scalar(Setting)
                .doc(
                    "The title tier it belongs to; `none` applies to all tiers, which is \
                     bad for performance in bulk. Character and title types only.",
                )
                .values(&[
                    "barony", "county", "duchy", "kingdom", "empire", "hegemony", "none",
                ]),
        ),
        (
            "special",
            scalar(Setting)
                .doc(
                    "Special category checked before regular flavorization \
                     (`type = character` only). Default `holder`.",
                )
                .values(&[
                    "head_of_faith",
                    "councillor",
                    "queen_mother",
                    "ruler_child",
                    "domicile",
                    "holder",
                ]),
        ),
        (
            "priority",
            scalar(Setting).doc("Highest priority wins; lower-priority entries are skipped."),
        ),
        (
            "flavourization_rules",
            block(Struct(&RULES)).doc("The engine-evaluated rule toggles."),
        ),
        (
            "flag",
            scalar(Setting).doc("The context character must have this (integer) flag."),
        ),
        (
            "governments",
            block(Config).doc("The context character's government must be one of these."),
        ),
        (
            "domicile_type",
            scalar(Setting)
                .refs(kinds::DOMICILE_TYPE)
                .doc("Required domicile type; mandatory when `type = domicile`."),
        ),
        (
            "name_lists",
            block(Config).doc(
                "The context character's culture must use one of these name lists \
                 (`common/culture/name_lists/`).",
            ),
        ),
        (
            "heritages",
            block(Config)
                .doc("The context character's culture must have one of these heritage pillars."),
        ),
        (
            "faiths",
            block(Config).doc("The context character's faith must be one of these."),
        ),
        (
            "faith",
            scalar(Setting)
                .refs(kinds::FAITH)
                .doc("Singular form of `faiths` *(corpus)*."),
        ),
        (
            "religions",
            block(Config).doc("The context character's religion must be one of these."),
        ),
        (
            "council_position",
            scalar(Setting).doc(
                "The character must hold this council position \
                 (`common/council_positions/`, not modeled yet).",
            ),
        ),
        (
            "de_jure_liege",
            block(Config)
                .doc("The causing title must fall under the de jure of one of these titles."),
        ),
        (
            "holding",
            scalar(Setting).refs(kinds::HOLDING).doc(
                "The holding at the causing title must be of this type \
                 (`type = title`, `tier = barony` only).",
            ),
        ),
        (
            "titles",
            block(Config)
                .doc("The causing title must be one of these (character and title types)."),
        ),
        (
            "subject_contract_obligation_flags",
            block(Config).doc(
                "The context character must have one of these flags from vassal/tributary \
                 contracts or tax slots (flag namespace, not symbol names).",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Flavorization;

impl Entity for Flavorization {
    /// The key localizes itself; there is no `_desc` or any other suffix.
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[ImplicitLocPattern {
        kind: kinds::FLAVORIZATION,
        suffix: "",
    }];

    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::FLAVORIZATION,
            icon: IconHint::Text,
            defs: Some(DefSource {
                dir_prefix: DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[],
            aliases: &[],
        },
        // The condition lists, each gated to this directory (their keys mean
        // other things elsewhere — `governments` is a building-asset block).
        KindSpec {
            kind: kinds::GOVERNMENT,
            icon: IconHint::Hierarchy,
            defs: None,
            refs: &[in_flavorization(RefPattern::KeyList("governments"))],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::NAME_LIST,
            icon: IconHint::Object,
            defs: None,
            refs: &[in_flavorization(RefPattern::KeyList("name_lists"))],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::CULTURE_PILLAR,
            icon: IconHint::Object,
            defs: None,
            refs: &[in_flavorization(RefPattern::KeyList("heritages"))],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::FAITH,
            icon: IconHint::Object,
            defs: None,
            refs: &[in_flavorization(RefPattern::KeyList("faiths"))],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::RELIGION,
            icon: IconHint::Object,
            defs: None,
            refs: &[in_flavorization(RefPattern::KeyList("religions"))],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::TITLE,
            icon: IconHint::Hierarchy,
            defs: None,
            refs: &[
                in_flavorization(RefPattern::KeyList("titles")),
                in_flavorization(RefPattern::KeyList("de_jure_liege")),
            ],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(DIR, Struct(&FLAVORIZATION))];
}
