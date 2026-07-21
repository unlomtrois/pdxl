//! Structural context for the `create_character = { … }` built-in effect, from
//! its `effects.log` entry. Not a directory-rooted concept — it's registered as
//! an *effect struct* (see [`crate::contexts`]) so the block reads as this
//! documented structure wherever the effect is used.

use pdxl_analysis::context::ClauseKind::{Effect, ScriptValue, Struct};
use pdxl_analysis::context::ScalarKind::{Setting, Target};
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar, scalar_or_block};

/// A skill field (`martial = 8` or a script value), random unless set.
const fn skill(doc: &'static str) -> FieldSpec {
    scalar_or_block(Setting, ScriptValue).doc(doc)
}

/// `key = { <triggers> }` per named entry (a faith/culture/ethnicity name whose
/// value is a trigger block). No fixed fields; entries fall to triggers.
static NAMED_TRIGGER_LIST: StructSpec = StructSpec {
    name: "weighted list",
    fields: &[],
    fallback: Fallback::Trigger,
};

/// `random_traits_list = { count = … traitID = { <triggers> } … }`.
static RANDOM_TRAITS_LIST: StructSpec = StructSpec {
    name: "random_traits_list",
    fields: &[(
        "count",
        scalar_or_block(Setting, ScriptValue)
            .doc("How many traits to pick (a number or `{ min max }` range; 1 if unset)."),
    )],
    fallback: Fallback::Trigger,
};

/// The body of `create_character = { … }`.
pub(crate) static CREATE_CHARACTER: StructSpec = StructSpec {
    name: "create_character",
    fields: &[
        (
            "save_event_target_as",
            scalar(Setting).doc("Save the created character as an event target."),
        ),
        (
            "save_temporary_event_target_as",
            scalar(Setting).doc("Save the created character as a temporary event target."),
        ),
        (
            "name",
            scalar(Setting).doc("The character's name (a name key)."),
        ),
        (
            "age",
            scalar_or_block(Setting, ScriptValue).doc("Starting age (a number or script value)."),
        ),
        (
            "gender",
            scalar(Setting).doc("`male` / `female`, or a character scope to copy the gender from."),
        ),
        (
            "gender_female_chance",
            scalar_or_block(Setting, ScriptValue)
                .doc("Chance the character is female, 0–100 (a script value)."),
        ),
        (
            "opposite_gender",
            scalar(Target).doc("A character scope; set the opposite gender to theirs."),
        ),
        (
            "trait",
            scalar(Setting).doc("Add this trait to the character."),
        ),
        (
            "random_traits_list",
            block(Struct(&RANDOM_TRAITS_LIST)).doc(
                "Pick `count` traits from the listed ones whose triggers pass (scopes as at the \
                 `create_character` site). Repeatable.",
            ),
        ),
        (
            "random_traits",
            scalar(Setting).doc("`yes`/`no` — whether to also give random traits."),
        ),
        (
            "health",
            scalar_or_block(Setting, ScriptValue).doc("Starting health."),
        ),
        (
            "fertility",
            scalar_or_block(Setting, ScriptValue).doc("Starting fertility."),
        ),
        (
            "mother",
            scalar(Target).doc("The mother (a character scope)."),
        ),
        (
            "father",
            scalar(Target).doc("The father (a character scope)."),
        ),
        (
            "real_father",
            scalar(Target).doc("The biological father, only if different from `father`."),
        ),
        (
            "employer",
            scalar(Target).doc(
                "The character joins this court; becomes a pool character unless landed or a \
                 parent is landed. Mutually exclusive with `location`.",
            ),
        ),
        (
            "location",
            scalar(Target).doc("A pool province. Mutually exclusive with `employer`."),
        ),
        (
            "template_character",
            scalar(Target)
                .doc("Copy faith / culture / dynasty from this character scope, unless set below."),
        ),
        (
            "faith",
            scalar(Setting).doc("A faith tag or a faith scope."),
        ),
        (
            "random_faith",
            block(Struct(&NAMED_TRIGGER_LIST))
                .doc("Pick a random faith among the listed ones whose triggers pass."),
        ),
        (
            "random_faith_in_religion",
            scalar(Setting).doc("A religion tag or faith scope; random faith within it."),
        ),
        (
            "culture",
            scalar(Setting).doc("A culture name or a culture scope."),
        ),
        (
            "random_culture",
            block(Struct(&NAMED_TRIGGER_LIST))
                .doc("Pick a random culture among the listed ones whose triggers pass."),
        ),
        (
            "random_culture_in_group",
            scalar(Setting)
                .doc("A culture-group name or a culture scope; random culture within it."),
        ),
        (
            "dynasty_house",
            scalar(Setting).doc("A dynasty-house name or scope."),
        ),
        (
            "dynasty",
            scalar(Setting).doc(
                "`generate` / `inherit` / `none` when `dynasty_house` is unset (generate by \
                 default).",
            ),
        ),
        (
            "ethnicity",
            scalar(Setting).doc(
                "`culture` / `mother` / `father` / `parents` / `<ethnicity>` — how to pick \
                 ethnicity (culture by default).",
            ),
        ),
        (
            "ethnicities",
            block(Struct(&NAMED_TRIGGER_LIST))
                .doc("Pick randomly among these ethnicities whose triggers pass."),
        ),
        ("martial", skill("Martial skill (random unless specified).")),
        (
            "diplomacy",
            skill("Diplomacy skill (random unless specified)."),
        ),
        (
            "intrigue",
            skill("Intrigue skill (random unless specified)."),
        ),
        (
            "stewardship",
            skill("Stewardship skill (random unless specified)."),
        ),
        (
            "learning",
            skill("Learning skill (random unless specified)."),
        ),
        ("prowess", skill("Prowess skill (random unless specified).")),
        (
            "after_creation",
            block(Effect).doc(
                "Effects run after creation. Scope is the new character, with the creating scope \
                 as PREV and the same top scope and saved targets.",
            ),
        ),
    ],
    // Every valid key is listed; unknown keys are not create_character options.
    fallback: Fallback::Deny,
};
