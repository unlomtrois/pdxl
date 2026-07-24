//! Character traits (`common/traits/`, from `_traits.info`) — the densest
//! symbol kind in the game: 323 corpus defs referenced tens of thousands of
//! times. The body mixes ~45 documented properties with free-form modifier
//! tags ("any other unknown property is read in as a modifier"), which is
//! exactly what [`Fallback::Modifier`] expresses.
//!
//! References (all corpus-validated at 0 unresolved):
//! - `add_trait` / `remove_trait` / `has_trait` scalars anywhere, and the XP
//!   block forms (`add_trait_xp`/`has_trait_xp` `{ trait = X }`);
//! - history characters' `trait = X` (body and dated blocks), gated;
//! - `opposites = { X Y … }` lists and `compatibility = { X = n … }` block
//!   *keys* (the [`RefPattern::KeyBlockKeys`] shape exists for this), both
//!   gated to the traits dir — both may also name trait *groups*, which the
//!   alias mechanism already resolves.
//!
//! Aliases: `group` / `group_equivalence` names resolve like trait names.
//! The `.info` spells it "group_equivelence" — a doc typo; the corpus uses
//! `group_equivalence` (16×) and `group_inheritance` never appears.
//!
//! Not modeled as refs: `triggered_opinion.opinion_modifier` (opinion
//! modifiers are not a schema kind yet — see BACKLOG), culture/faith
//! `parameter` values, `genetic_constraint_*` (genes db), and `flag` (a
//! trait-flag namespace, 74 distinct).

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, DynamicDesc, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{OPAQUE, anywhere};

const TRAITS_DIR: &str = "common/traits/";

/// A `yes`/`no` toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// A reference rule gated to the traits directory.
/// A reference rule gated to the religion tree (virtue/sin lists).
const fn in_religion(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some("common/religion/"),
        alt: &[],
    }
}

const fn in_traits(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(TRAITS_DIR),
        alt: &[],
    }
}

/// `triggered_opinion = { … }` — a conditional opinion impact.
static TRIGGERED_OPINION: StructSpec = StructSpec {
    name: "triggered_opinion",
    fields: &[
        (
            "opinion_modifier",
            scalar(Setting).doc("The opinion modifier applied when the conditions hold."),
        ),
        (
            "parameter",
            scalar(Setting).doc("A boolean doctrine parameter to require."),
        ),
        (
            "check_missing",
            toggle("Require the parameter to be unset/false instead of true."),
        ),
        (
            "same_faith",
            toggle("Only between characters of the same faith."),
        ),
        (
            "same_dynasty",
            toggle("Only between characters of the same dynasty."),
        ),
        (
            "ignore_opinion_value_if_same_trait",
            toggle(
                "If both characters have this trait, skip the opinion effect (punishment reasons still apply).",
            ),
        ),
        ("male_only", toggle("Only when the trait holder is male.")),
        (
            "female_only",
            toggle("Only when the trait holder is female (mutually exclusive with `male_only`)."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// One XP threshold inside a track: `20 = { <modifiers> }` — the numeric keys
/// are XP scores (0–100, ascending), their bodies are modifier collections.
static XP_LEVEL: StructSpec = StructSpec {
    name: "trait XP level",
    fields: &[],
    fallback: Fallback::Modifier,
};

/// `track = { 20 = {…} … }` / one named track inside `tracks`.
static TRACK: StructSpec = StructSpec {
    name: "trait track",
    fields: &[],
    fallback: Fallback::Struct(&XP_LEVEL),
};

/// `tracks = { <name> = { 20 = {…} } … }` — named XP tracks.
static TRACKS: StructSpec = StructSpec {
    name: "trait tracks",
    fields: &[],
    fallback: Fallback::Struct(&TRACK),
};

/// `monthly_track_xp_degradation = { min = 20 change = 5 }`.
static XP_DEGRADATION: StructSpec = StructSpec {
    name: "monthly_track_xp_degradation",
    fields: &[
        ("min", scalar(Setting).doc("XP never degrades below this.")),
        ("change", scalar(Setting).doc("XP lost per month.")),
    ],
    fallback: Fallback::Deny,
};

/// A conditional modifier collection gated on a culture/faith parameter.
static PARAM_MODIFIER: StructSpec = StructSpec {
    name: "parameter modifier",
    fields: &[(
        "parameter",
        scalar(Setting).doc("The culture/doctrine parameter that must be set."),
    )],
    fallback: Fallback::Modifier,
};

/// The body of one trait definition (`_traits.info`). Unknown keys are
/// modifier tags applied to every holder of the trait.
static TRAIT: StructSpec = StructSpec {
    name: "trait",
    fields: &[
        // Loc/icon (default: trait_<key>, trait_<key>_desc, <key>.dds).
        ("name", scalar_or_block(LocKey, DynamicDesc).doc("Name override — a loc key or a dynamic description (root = the character; may not exist, so guard with `NOT = { exists = this }` first).")),
        ("desc", scalar_or_block(LocKey, DynamicDesc).doc("Description override (defaults to `trait_<key>_desc`).")),
        ("icon", scalar_or_block(Setting, DynamicDesc).doc("Icon override — a `.dds` name or a dynamic description (default `gfx/interface/icons/traits/<trait>.dds`).")),
        (
            "category",
            scalar(Setting)
                .doc("The trait's unique category (gameplay + sorting implications).")
                .values(&[
                    "personality",
                    "education",
                    "childhood",
                    "commander",
                    "winter_commander",
                    "lifestyle",
                    "court_type",
                    "fame",
                    "health",
                ]),
        ),
        // Validation.
        (
            "valid_sex",
            scalar(Setting)
                .doc("Which sex can hold the trait (default `all`).")
                .values(&["all", "male", "female"]),
        ),
        ("minimum_age", scalar(Setting).doc("Minimum age required.")),
        ("maximum_age", scalar(Setting).doc("Maximum age allowed.")),
        (
            "potential",
            block(Trigger).doc(
                "Must pass for the trait to be given (not re-checked after; does not run \
                 in ruler designer; non-potential traits purge on game start).",
            ),
        ),
        // Special flags.
        (
            "inheritance_blocker",
            scalar(Setting)
                .doc("Blocks title inheritance (`dynasty` = only within the same dynasty).")
                .values(&["none", "dynasty", "all"]),
        ),
        (
            "claim_inheritance_blocker",
            scalar(Setting)
                .doc("Blocks claim inheritance (`dynasty` = only within the same dynasty).")
                .values(&["none", "dynasty", "all"]),
        ),
        ("add_commander_trait", toggle("Auto-generated characters with this trait also get commander traits.")),
        ("incapacitating", toggle("The character cannot rule directly and requires a regent.")),
        ("physical", toggle("A physical aspect of the character's body.")),
        ("disables_combat_leadership", toggle("The character cannot be a commander.")),
        ("can_have_children", toggle("Whether the character can have children at all (default `yes`).")),
        ("genetic", toggle("Genetic inheritance: active parent trait inherits at 100%, inactive at 50%; from both parents → active, from one → inactive. Mutually exclusive with manual inherit_chance.")),
        ("good", toggle("A \"good\" genetic trait.")),
        ("inherit_from_real_father", toggle("Inherit from the real father (genetic/manual-chance traits; default `yes`).")),
        ("inherit_from_real_mother", toggle("Inherit from the real mother (genetic/manual-chance traits; default `yes`).")),
        ("enables_inbred", toggle("Children become eligible for the inbred trait (only with common ancestors).")),
        ("shown_in_encyclopedia", toggle("Show in the encyclopedia (default `yes`).")),
        ("shown_in_ruler_designer", toggle("Show in the ruler designer (default `yes`).")),
        ("immortal", toggle("Stops visual aging and natural death (script can still kill; see `set_immortal_age`).")),
        (
            "bastard",
            scalar(Setting)
                .doc("Marks the character a bastard of this type.")
                .values(&["none", "illegitimate", "legitimate"]),
        ),
        (
            "trait_exclusive_if_realm_contains",
            block(Struct(&OPAQUE)).doc(
                "Terrain types: this commander trait is only randomly assigned when the \
                 commander's culture holds a province with one of them.",
            ),
        ),
        // Generation & inheritance chances.
        ("birth", scalar(Setting).doc("% of characters born with this trait when not inherited (0–100).")),
        ("random_creation", scalar(Setting).doc("% chance on non-birth character creation (inheritable/genetic traits only).")),
        ("random_creation_weight", scalar(Setting).doc("Relative weight for generated characters' personality/education/childhood picks (default 1; 0 = never; ignored for genetic/inheritable).")),
        ("inherit_chance", scalar(Setting).doc("% inheritance chance (manual; cannot be set on genetic traits).")),
        ("both_parent_has_trait_inherit_chance", scalar(Setting).doc("% inheritance chance when both parents have the trait (manual; not for genetic).")),
        (
            "parent_inheritance_sex",
            scalar(Setting)
                .doc("Which parent can pass the trait on (default `all`).")
                .values(&["male", "female", "all"]),
        ),
        (
            "child_inheritance_sex",
            scalar(Setting)
                .doc("Which children can inherit the trait (default `all`).")
                .values(&["male", "female", "all"]),
        ),
        // Portrait impacts.
        ("genetic_constraint_all", scalar(Setting).doc("Genetic constraint applied on gaining the trait.")),
        ("genetic_constraint_men", scalar(Setting).doc("Genetic constraint applied for men.")),
        ("genetic_constraint_women", scalar(Setting).doc("Genetic constraint applied for women.")),
        ("portrait_extremity_shift", scalar(Setting).doc("Shift every morph gene toward its nearest extreme by this fraction on gain.")),
        ("ugliness_portrait_extremity_shift", scalar(Setting).doc("Like `portrait_extremity_shift`, but only the character's most extreme feature.")),
        // Opinions.
        ("same_opinion", scalar(Setting).doc("Opinion between two holders of the trait.")),
        ("same_opinion_if_same_faith", scalar(Setting).doc("Opinion between two same-faith holders.")),
        ("opposite_opinion", scalar(Setting).doc("Opinion toward holders of opposite traits.")),
        (
            "triggered_opinion",
            block(Struct(&TRIGGERED_OPINION))
                .doc("A conditional opinion impact (doctrine parameter, same faith/dynasty, sex gates)."),
        ),
        // Relations & groups.
        (
            "compatibility",
            block(Struct(&OPAQUE)).doc(
                "`<trait> = <value>` compatibility scores vs another character's traits \
                 (used by `compatibility_modifier` and the `trait_compatibility` trigger \
                 — not an opinion).",
            ),
        ),
        (
            "opposites",
            block(Struct(&OPAQUE)).doc("Traits this trait is the opposite of."),
        ),
        ("level", scalar(Setting).doc("The trait's level within its group.")),
        ("group", scalar(Setting).doc("Trait group — used for both inheritance and equivalence; the group name resolves like a trait in `has_trait` etc.")),
        ("group_equivalence", scalar(Setting).doc("Trait group used only for equivalence (the `.info` misspells this \"group_equivelence\").")),
        ("group_inheritance", scalar(Setting).doc("Trait group used only for inheritance (unused in the current corpus).")),
        // Misc.
        ("ruler_designer_cost", scalar(Setting).doc("Ruler designer point cost (default 0).")),
        ("flag", scalar(Setting).doc("A trait flag (repeatable), localized as `TRAIT_FLAG_DESC_<name>`.")),
        ("culture_succession_prio", scalar(Setting).doc("If the title-holder's culture has this flag, children with the trait sort as oldest (or youngest under ultimogeniture).")),
        // Level tracks.
        (
            "track",
            block(Struct(&TRACK)).doc(
                "Single-track shorthand: XP thresholds (0–100, ascending) mapping to \
                 modifier collections; the track is named after the trait. Used with \
                 `add_trait_xp` / `has_trait_xp`. Localize as `trait_track_<key>`.",
            ),
        ),
        (
            "tracks",
            block(Struct(&TRACKS)).doc("Named XP tracks (each `<name> = { <xp> = { <modifiers> } … }`)."),
        ),
        (
            "monthly_track_xp_degradation",
            block(Struct(&XP_DEGRADATION)).doc("Monthly XP decay: `{ min = 20 change = 5 }`."),
        ),
        // Conditional modifiers.
        (
            "culture_modifier",
            block(Struct(&PARAM_MODIFIER))
                .doc("Modifiers applied when the holder's culture has `parameter`."),
        ),
        (
            "faith_modifier",
            block(Struct(&PARAM_MODIFIER))
                .doc("Modifiers applied when the holder's faith has a doctrine with `parameter`."),
        ),
    ],
    // "Any other unknown property is read in as a modifier applied to anyone
    // who holds the trait."
    fallback: Fallback::Modifier,
};

pub(crate) struct Traits;

impl Entity for Traits {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::TRAIT,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: TRAITS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("add_trait")),
            anywhere(RefPattern::KeyValue("remove_trait")),
            anywhere(RefPattern::KeyValue("has_trait")),
            // XP effects/triggers name the trait in a block: `{ trait = X … }`.
            anywhere(RefPattern::KeyBlockField("add_trait_xp", "trait")),
            anywhere(RefPattern::KeyBlockField("has_trait_xp", "trait")),
            // History characters list starting/dated traits as `trait = X`
            // (both the body and the dated blocks; corpus 0 unresolved).
            RefRule {
                pattern: RefPattern::KeyValue("trait"),
                gate: Some("history/characters/"),
                alt: &[],
            },
            // Trait relations inside trait bodies (both also accept trait
            // *group* names, which resolve via the alias mechanism).
            in_traits(RefPattern::KeyList("opposites")),
            in_traits(RefPattern::KeyBlockKeys("compatibility")),
            // Religious virtues/sins (religions + doctrines): the lists mix
            // loose names (`brave`), scaled names (`brave = 0.5`), and block
            // forms (`brave = { scale = … }`) — the List and BlockKeys shapes
            // together cover all three. Trait groups resolve via aliases.
            in_religion(RefPattern::KeyList("virtues")),
            in_religion(RefPattern::KeyBlockKeys("virtues")),
            in_religion(RefPattern::KeyList("sins")),
            in_religion(RefPattern::KeyBlockKeys("sins")),
        ],
        // CK3 traits expose group / group_equivalence names as valid refs.
        aliases: &["group", "group_equivalence"],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(TRAITS_DIR, ClauseKind::Struct(&TRAIT))];
}
