//! EU5 religions and their related definition families. Religious aspects,
//! factions and focuses follow their directory readmes; figures and schools
//! are modeled from their complete vanilla corpora.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Effect, ScriptValue, StaticModifier, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar, scalar_or_block};
use pdxl_analysis::{
    DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern, RefRule,
};

use super::Entity;

pub(crate) const RELIGIONS_DIR: &str = "in_game/common/religions/";
const GROUPS_DIR: &str = "in_game/common/religion_groups/";
const ASPECTS_DIR: &str = "in_game/common/religious_aspects/";
const FACTIONS_DIR: &str = "in_game/common/religious_factions/";
const FIGURES_DIR: &str = "in_game/common/religious_figures/";
const FOCUSES_DIR: &str = "in_game/common/religious_focuses/";
const SCHOOLS_DIR: &str = "in_game/common/religious_schools/";

static RELIGION: StructSpec = StructSpec {
    name: "religion",
    fields: &[
        ("color", scalar_or_block(Setting, ClauseKind::Config)),
        ("group", scalar(Setting).doc("Religion group.")),
        ("language", scalar(Setting)),
        ("culture_locked", scalar(Setting).values(&["yes", "no"])),
        ("definition_modifier", block(StaticModifier)),
        (
            "opinions",
            block(ClauseKind::Config).doc("Opinion values keyed by religion."),
        ),
        ("tags", block(ClauseKind::Config).doc("Graphical tags.")),
        (
            "custom_tags",
            block(ClauseKind::Config).doc("Religion-system behavior tags."),
        ),
        (
            "unique_names",
            block(ClauseKind::Config).doc("Character-name vocabulary used by the religion."),
        ),
        (
            "enable",
            scalar(Setting).doc("Date on which the religion becomes available."),
        ),
        (
            "religious_aspects",
            scalar(Setting).doc("Number of simultaneously selectable religious aspects."),
        ),
        (
            "has_religious_influence",
            scalar(Setting).values(&["yes", "no"]),
        ),
        ("ai_wants_convert", scalar(Setting).values(&["yes", "no"])),
        ("needs_reform", scalar(Setting).values(&["yes", "no"])),
        ("has_religious_head", scalar(Setting).values(&["yes", "no"])),
        ("has_cardinals", scalar(Setting).values(&["yes", "no"])),
        ("has_canonization", scalar(Setting).values(&["yes", "no"])),
        (
            "has_autocephalous_patriarchates",
            scalar(Setting).values(&["yes", "no"]),
        ),
        ("has_patriarchs", scalar(Setting).values(&["yes", "no"])),
        ("has_karma", scalar(Setting).values(&["yes", "no"])),
        ("has_yanantin", scalar(Setting).values(&["yes", "no"])),
        ("has_purity", scalar(Setting).values(&["yes", "no"])),
        ("has_omens", scalar(Setting).values(&["yes", "no"])),
        ("has_honor", scalar(Setting).values(&["yes", "no"])),
        ("has_avatars", scalar(Setting).values(&["yes", "no"])),
        ("use_icons", scalar(Setting).values(&["yes", "no"])),
        (
            "tithe",
            scalar(Setting).doc("Share of income paid as tithe."),
        ),
        (
            "important_country",
            scalar(Setting).doc("Country central to this religion."),
        ),
        ("max_sects", scalar(Setting)),
        ("max_religious_figures_for_religion", scalar(Setting)),
        ("num_religious_focuses_needed_for_reform", scalar(Setting)),
        ("saints_concept", scalar(Setting)),
        ("goods_demand_modifier", block(ClauseKind::Config)),
        ("clergy_goods_demand_modifier", block(ClauseKind::Config)),
        ("religious_school", scalar(Setting)),
        ("religious_focuses", block(ClauseKind::Config)),
        ("factions", block(ClauseKind::Config)),
    ],
    fallback: Fallback::Ignore,
};

static GROUP: StructSpec = StructSpec {
    name: "religion group",
    fields: &[
        ("color", scalar_or_block(Setting, ClauseKind::Config)),
        (
            "allow_slaves_of_same_group",
            scalar(Setting).values(&["yes", "no"]),
        ),
        (
            "convert_slaves_at_start",
            scalar(Setting).values(&["yes", "no"]),
        ),
        ("modifier", block(StaticModifier)),
        ("goods_demand_modifier", block(ClauseKind::Config)),
        ("clergy_goods_demand_modifier", block(ClauseKind::Config)),
    ],
    fallback: Fallback::Ignore,
};

static ASPECT: StructSpec = StructSpec {
    name: "religious aspect",
    fields: &[
        (
            "religion",
            scalar(Setting).doc("Religion receiving this aspect; repeatable."),
        ),
        ("visible", block_scoped(Trigger, "country")),
        ("enabled", block_scoped(Trigger, "country")),
        ("modifier", block(StaticModifier)),
        ("opinions", block(ClauseKind::Config)),
        ("icon", scalar(Setting)),
        ("saints_concept", scalar(Setting)),
    ],
    fallback: Fallback::Ignore,
};

static FACTION: StructSpec = StructSpec {
    name: "religious faction",
    fields: &[
        (
            "visible",
            block_scoped(Trigger, "international_organization"),
        ),
        (
            "enabled",
            block_scoped(Trigger, "international_organization"),
        ),
        ("actions", block(ClauseKind::Config)),
    ],
    fallback: Fallback::Deny,
};

static FIGURE: StructSpec = StructSpec {
    name: "religious figure",
    fields: &[("enabled_for_religion", block_scoped(Trigger, "religion"))],
    fallback: Fallback::Deny,
};

static FOCUS: StructSpec = StructSpec {
    name: "religious focus",
    fields: &[
        ("potential", block_scoped(Trigger, "country")),
        ("allow", block_scoped(Trigger, "country")),
        ("monthly_progress", scalar_or_block(Setting, ScriptValue)),
        ("modifier_while_progressing", block(StaticModifier)),
        ("modifier_on_completion", block(StaticModifier)),
        ("effect_on_completion", block_scoped(Effect, "country")),
        ("ai_will_do", scalar_or_block(Setting, ScriptValue)),
    ],
    fallback: Fallback::Deny,
};

static SCHOOL: StructSpec = StructSpec {
    name: "religious school",
    fields: &[
        ("color", scalar_or_block(Setting, ClauseKind::Config)),
        ("enabled_for_country", block_scoped(Trigger, "country")),
        ("enabled_for_character", block_scoped(Trigger, "character")),
        ("modifier", block(StaticModifier)),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Religion;

impl Entity for Religion {
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        loc(kinds::RELIGION, ""),
        loc(kinds::RELIGION, "_desc"),
        loc(kinds::RELIGION, "_ADJ"),
        loc(kinds::RELIGION_GROUP, ""),
        loc(kinds::RELIGION_GROUP, "_desc"),
        loc(kinds::RELIGION_GROUP, "_ADJ"),
        loc(kinds::RELIGIOUS_ASPECT, ""),
        loc(kinds::RELIGIOUS_ASPECT, "_desc"),
        loc(kinds::RELIGIOUS_FACTION, ""),
        loc(kinds::RELIGIOUS_FACTION, "_desc"),
        loc(kinds::RELIGIOUS_FIGURE, ""),
        loc(kinds::RELIGIOUS_FIGURE, "_desc"),
        loc(kinds::RELIGIOUS_FOCUS, ""),
        loc(kinds::RELIGIOUS_FOCUS, "_desc"),
        loc(kinds::RELIGIOUS_SCHOOL, ""),
        loc(kinds::RELIGIOUS_SCHOOL, "_desc"),
    ];

    const LOC_DATAFN_ARG_REFS: &'static [(&'static str, pdxl_analysis::KindId)] = &[
        ("GetReligionByKey", kinds::RELIGION),
        ("ShowReligionName", kinds::RELIGION),
        ("ShowReligionNameWithNoTooltip", kinds::RELIGION),
        ("ShowReligionAdjective", kinds::RELIGION),
        ("ShowReligionAdjectiveWithNoTooltip", kinds::RELIGION),
        ("ShowReligionGroupName", kinds::RELIGION_GROUP),
        ("ShowReligionGroupNameWithNoTooltip", kinds::RELIGION_GROUP),
        ("ShowReligionGroupAdjective", kinds::RELIGION_GROUP),
        (
            "ShowReligionGroupAdjectiveWithNoTooltip",
            kinds::RELIGION_GROUP,
        ),
        (
            "ShowReligiousGroupAdjectiveWithNoTooltip",
            kinds::RELIGION_GROUP,
        ),
        ("ShowReligiousAspectName", kinds::RELIGIOUS_ASPECT),
        (
            "ShowReligiousAspectNameWithNoTooltip",
            kinds::RELIGIOUS_ASPECT,
        ),
        ("ShowReligiousSchoolName", kinds::RELIGIOUS_SCHOOL),
        (
            "ShowReligiousSchoolNameWithNoTooltip",
            kinds::RELIGIOUS_SCHOOL,
        ),
    ];

    const KINDS: &'static [KindSpec] = &[
        kind(
            kinds::RELIGION,
            RELIGIONS_DIR,
            &[
                RefRule {
                    pattern: RefPattern::KeyValue("religion_definition"),
                    gate: None,
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyValue("religion"),
                    gate: Some(ASPECTS_DIR),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyBlockKeys("opinions"),
                    gate: Some(RELIGIONS_DIR),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyBlockKeys("opinions"),
                    gate: Some(ASPECTS_DIR),
                    alt: &[],
                },
            ],
        ),
        kind(
            kinds::RELIGION_GROUP,
            GROUPS_DIR,
            &[RefRule {
                pattern: RefPattern::KeyValue("group"),
                gate: Some(RELIGIONS_DIR),
                alt: &[],
            }],
        ),
        kind(kinds::RELIGIOUS_ASPECT, ASPECTS_DIR, &[]),
        kind(
            kinds::RELIGIOUS_FACTION,
            FACTIONS_DIR,
            &[RefRule {
                pattern: RefPattern::KeyList("factions"),
                gate: Some(RELIGIONS_DIR),
                alt: &[],
            }],
        ),
        kind(kinds::RELIGIOUS_FIGURE, FIGURES_DIR, &[]),
        kind(
            kinds::RELIGIOUS_FOCUS,
            FOCUSES_DIR,
            &[RefRule {
                pattern: RefPattern::KeyList("religious_focuses"),
                gate: Some(RELIGIONS_DIR),
                alt: &[],
            }],
        ),
        kind(
            kinds::RELIGIOUS_SCHOOL,
            SCHOOLS_DIR,
            &[
                RefRule {
                    pattern: RefPattern::KeyValue("religious_school"),
                    gate: Some(RELIGIONS_DIR),
                    alt: &[],
                },
                RefRule {
                    pattern: RefPattern::KeyDescendantKeys("religion_manager", &["relation"]),
                    gate: Some(super::setup_manager::START_SETUP_DIR),
                    alt: &[],
                },
            ],
        ),
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (RELIGIONS_DIR, ClauseKind::Struct(&RELIGION)),
        (GROUPS_DIR, ClauseKind::Struct(&GROUP)),
        (ASPECTS_DIR, ClauseKind::Struct(&ASPECT)),
        (FACTIONS_DIR, ClauseKind::Struct(&FACTION)),
        (FIGURES_DIR, ClauseKind::Struct(&FIGURE)),
        (FOCUSES_DIR, ClauseKind::Struct(&FOCUS)),
        (SCHOOLS_DIR, ClauseKind::Struct(&SCHOOL)),
    ];
}

const fn loc(kind: pdxl_analysis::KindId, suffix: &'static str) -> ImplicitLocPattern {
    ImplicitLocPattern { kind, suffix }
}

const fn kind(
    kind: pdxl_analysis::KindId,
    dir: &'static str,
    refs: &'static [RefRule],
) -> KindSpec {
    KindSpec {
        kind,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: dir,
            shape: DefShape::TopLevel,
        }),
        refs,
        aliases: &[],
    }
}
