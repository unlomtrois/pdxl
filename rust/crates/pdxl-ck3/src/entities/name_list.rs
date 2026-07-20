//! Name lists (`common/culture/name_lists/`, from `_name_lists.info`) —
//! virtually everything regarding naming in cultures.
//!
//! Cross-reference (corpus-validated, vanilla + T4N, 0 unresolved):
//! `name_list = X` — a culture (or aesthetics bundle) picks its name list.
//! The key is not overloaded anywhere in the corpus (the ungated total equals
//! the `common/culture/cultures/` gated total), so the rule ships ungated and
//! also covers `common/culture/aesthetics_bundles/` bodies.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{OPAQUE, anywhere};
use super::culture_shared::NAME_LISTS_DIR;

/// A `yes`/`no` toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// A name-after-relative percent chance.
const fn name_chance(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc)
}

/// The body of one name list (`_name_lists.info`).
static NAME_LIST: StructSpec = StructSpec {
    name: "name_list",
    fields: &[
        (
            "founder_named_dynasties",
            toggle(
                "Use the founder's name when creating new dynasties or cadet branches (default `no`).",
            ),
        ),
        (
            "house_based_map_names",
            toggle("Can a house name be used as realm name on the map? (default `no`)."),
        ),
        (
            "suggest_family_names",
            toggle("Can names be suggested from within the family? (default `yes`)."),
        ),
        (
            "suggest_ancestor_names",
            toggle("Can names be suggested from an ancestor? (default `yes`)."),
        ),
        (
            "mercenary_names",
            block(Struct(&OPAQUE)).doc(
                "Names and coats of arms usable by mercenaries of this culture: \
                 `{ name = \"…\" coat_of_arms = \"…\" }` entries.",
            ),
        ),
        (
            "male_names",
            block(Struct(&OPAQUE)).doc(
                "Male names, either a single list or weighted groups (`10 = { … }`; higher \
                 = more common). `nameX_baseY` marks nameX as a variant of base name baseY \
                 (e.g. `Jan_John`).",
            ),
        ),
        (
            "female_names",
            block(Struct(&OPAQUE)).doc(
                "Female names, either a single list or weighted groups — same format as \
                 `male_names`.",
            ),
        ),
        (
            "dynasty_names",
            block(Struct(&OPAQUE)).doc(
                "Dynasty names (no weights). `{ dynnp_von dynn_Pommern }` adds an optional \
                 prefix before the base name; the braces are only required with a prefix.",
            ),
        ),
        (
            "cadet_dynasty_names",
            block(Struct(&OPAQUE)).doc(
                "Names used when creating cadet branches — same format as `dynasty_names` \
                 (undocumented in `_name_lists.info`, but shown in `_example.info` and \
                 used by most vanilla name lists).",
            ),
        ),
        (
            "dynasty_name_first",
            toggle(
                "Display the dynasty name before the personal name (undocumented in \
                 `_name_lists.info`; used by vanilla East Asian-style name lists).",
            ),
        ),
        (
            "grammar_transform",
            scalar(Setting).doc(
                "Grammar transformation applied when composing names (undocumented in \
                 `_name_lists.info`; the only corpus value is `french`, eliding \
                 e.g. \"de\" before a vowel).",
            ),
        ),
        (
            "bastard_dynasty_prefix",
            scalar(Setting).doc(
                "Dynasty-name prefix for bastard-founded dynasties, e.g. `\"dynnp_fitz\"` \
                 (undocumented in `_name_lists.info`; vanilla Anglo-Norman lists).",
            ),
        ),
        (
            "dynasty_of_location_prefix",
            scalar(Setting).doc("Prefix added when generating a dynasty name based on a title."),
        ),
        (
            "pat_grf_name_chance",
            name_chance(
                "Chance of male children being named after their paternal grandfather. The \
                 three male chances must not sum past 100.",
            ),
        ),
        (
            "mat_grf_name_chance",
            name_chance("Chance of male children being named after their maternal grandfather."),
        ),
        (
            "father_name_chance",
            name_chance("Chance of male children being named after their father."),
        ),
        (
            "pat_grm_name_chance",
            name_chance(
                "Chance of female children being named after their paternal grandmother. \
                 The three female chances must not sum past 100.",
            ),
        ),
        (
            "mat_grm_name_chance",
            name_chance("Chance of female children being named after their maternal grandmother."),
        ),
        (
            "mother_name_chance",
            name_chance("Chance of female children being named after their mother."),
        ),
        (
            "patronym_prefix_male",
            scalar(Setting).doc("Patronym prefix for men (`dynnpat_pre_mac` → “Mac…”)."),
        ),
        (
            "patronym_prefix_male_vowel",
            scalar(Setting)
                .doc("Patronym prefix for men when the parent's name starts with a vowel."),
        ),
        (
            "patronym_prefix_female",
            scalar(Setting).doc("Patronym prefix for women."),
        ),
        (
            "patronym_prefix_female_vowel",
            scalar(Setting)
                .doc("Patronym prefix for women when the parent's name starts with a vowel."),
        ),
        (
            "patronym_suffix_male",
            scalar(Setting).doc("Patronym suffix for men (`dynnpat_suf_son` → “…son”)."),
        ),
        (
            "patronym_suffix_female",
            scalar(Setting).doc("Patronym suffix for women."),
        ),
        (
            "always_use_patronym",
            toggle(
                "Display patronyms in names regardless of government (otherwise only when \
                 the character's or liege's government has `always_use_patronym = yes`). \
                 Default `no`.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct NameList;

impl Entity for NameList {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::NAME_LIST,
        icon: IconHint::Text,
        defs: Some(DefSource {
            dir_prefix: NAME_LISTS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[anywhere(RefPattern::KeyValue("name_list"))],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(NAME_LISTS_DIR, ClauseKind::Struct(&NAME_LIST))];
}
