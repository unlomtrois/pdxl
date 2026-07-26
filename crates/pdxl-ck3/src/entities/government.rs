//! Government types (`common/governments/`, from `_governments.info`) —
//! top-level definitions naming the ruleset a character plays under.
//!
//! References resolve from four ungated keys, all corpus-validated against
//! CK3 vanilla + T4N with zero unresolved values:
//!
//! - `has_government = X` (trigger, ~1100 sites) and `change_government = X`
//!   (effect, ~340) are unambiguous — the key names the concept.
//! - `government = X` is a common word, so it was checked exhaustively: every
//!   one of its ~6000 scalar values across `common/`, `events/` and
//!   `history/` is a government key. Most sit in `history/titles/` dated
//!   blocks. The only non-key values are four `$NEW_GOVERNMENT_TYPE$` macro
//!   parameters, which the extractor skips for free.
//! - `invalid_for_government = X` fires in `common/culture/` only, but the
//!   rule is left ungated because the key cannot mean anything else.
//!
//! `blocked_subject_courts` and `compatible_government_type_succession` are
//! government→government lists, gated to this directory since bare name lists
//! are ambiguous elsewhere.
//!
//! **Deliberate omissions.** Several documented fields point at databases this
//! schema does not model yet, so no rule is written for them and their values
//! stay unresolved-but-undiagnosed: `primary_holding`, `valid_holdings` and
//! `required_county_holdings` (`common/holdings/`), `vassal_contract_group`
//! (`common/vassal_contracts/`), `house_unity` (`common/house_unities/`),
//! `domicile_type` (`common/domiciles/`), `tax_slot_type`
//! (`common/tax_slot_types/`), and `preferred_religions` — CK3 models faiths
//! (`common/religion/religion_types/`, nested under `faiths`) but not the
//! religions containing them, so there is no kind to target. `flags` are
//! free-form strings consumed by `government_has_flag`, never definitions.
//!
//! Where the readme and the corpus disagree, the corpus wins and the readme
//! spelling is dropped: `vassal_contract_group` (readme says
//! `vassal_contract`) and `administrative_title_maa_setup` (readme says
//! `title_maa_setup`). Fields the corpus uses but the readme never documents
//! are marked *(corpus)*.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, DynamicDesc, ScriptValue, StaticModifier, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, color, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

pub(crate) const GOVERNMENTS_DIR: &str = "common/governments/";

/// A government→government name list, gated to this directory.
const fn gov_list(key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyList(key),
        gate: Some(GOVERNMENTS_DIR),
        alt: &[],
    }
}

/// Title tiers accepted by the administrative tier fields.
const TIERS: &[&str] = &["county", "duchy", "kingdom"];

/// `government_rules = { … }` — the engine-read bitmask of behaviour flags.
/// Every field is a `yes`/`no` toggle read from code; the readme lists the
/// default each one takes when omitted.
static GOVERNMENT_RULES: StructSpec = StructSpec {
    name: "government rules",
    fields: &[
        (
            "create_cadet_branches",
            scalar(Setting)
                .doc("Rulers can create cadet branches. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "religious",
            scalar(Setting)
                .doc("Rulers are considered clergy. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "court_generate_spouses",
            scalar(Setting)
                .doc("A new realm gets suitable spouses as courtiers. Default `yes`.")
                .values(&["yes", "no"]),
        ),
        (
            "council",
            scalar(Setting)
                .doc("The council is available. Default `yes`.")
                .values(&["yes", "no"]),
        ),
        (
            "rulers_should_have_dynasty",
            scalar(Setting)
                .doc("Rulers generate a dynasty. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "regiments_prestige_as_gold",
            scalar(Setting)
                .doc(
                    "Men-at-arms regiments are bought and reinforced with prestige \
                     (maintenance still costs gold). Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "dynasty_named_realms",
            scalar(Setting)
                .doc(
                    "The realm map name derives from dynasty and culture: culture head → \
                     \"The Mongols\", else dynasty head → \"The Borjigin Mongols\". Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "legitimacy",
            scalar(Setting)
                .doc("Rulers can have legitimacy where one is valid. Default `yes`.")
                .values(&["yes", "no"]),
        ),
        (
            "administrative",
            scalar(Setting)
                .doc(
                    "Enables the administrative mechanics — landless house-head vassals and \
                     title-owned men-at-arms. Requires the `admin_gov` dlc flag. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "admin_allows_holding_multiple_primary_tier_titles",
            scalar(Setting)
                .doc(
                    "Administrative rulers may hold multiple primary-tier titles. Only valid \
                     with `administrative = yes`, and only checked on appointment, \
                     appointment succession and stepping down. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "landless_playable",
            scalar(Setting)
                .doc(
                    "Rulers stay playable without a county. Requires the `landless_playable` \
                     dlc flag. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "allow_out_of_realm_inheritance",
            scalar(Setting)
                .doc("Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "use_as_base_on_landed",
            scalar(Setting)
                .doc(
                    "Switch to this government on gaining a first title, if the old holder \
                     used it. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "use_as_base_on_rank_up",
            scalar(Setting)
                .doc(
                    "Switch to this government when an independent ruler gains a higher top \
                     tier title from an independent ruler of this government. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "conditional_maa_refill",
            scalar(Setting)
                .doc(
                    "Men-at-arms reinforce only when their type trigger passes, and pay no \
                     upkeep. Costly to evaluate — use sparingly. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "mercenary",
            scalar(Setting)
                .doc(
                    "Unlanded rulers may offer themselves as mercenaries. Distinct from the \
                     `government_is_mercenary` flag, which names the mercenary company \
                     government specifically — that one does not use this rule. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "state_faith",
            scalar(Setting)
                .doc("Uses a state faith. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "treasury",
            scalar(Setting)
                .doc("Uses the Imperial Treasury resource. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "merit",
            scalar(Setting)
                .doc("Uses the Merit resource. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "uses_county_fertility",
            scalar(Setting)
                .doc("Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "replenishes_county_fertility",
            scalar(Setting)
                .doc("Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "obedience",
            scalar(Setting)
                .doc("Uses obedience. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "uses_culture_and_house_head_named_realms",
            scalar(Setting)
                .doc("Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "sticky_government",
            scalar(Setting)
                .doc("Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "subject_men_at_arms",
            scalar(Setting)
                .doc("Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "use_title_tier_modifiers",
            scalar(Setting)
                .doc(
                    "Passive prestige gain from held titles, and title tier modifiers. \
                     Default `yes`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "inherit_from_dynastic_government",
            scalar(Setting)
                .doc(
                    "Marks this government dynastic. Dynastic governments inherit freely from \
                     one another; a non-dynastic ruler cannot inherit from a dynastic one, \
                     which keeps unplayable governments from ending the game and stops \
                     inferior governments stealing land by inheritance. Default `yes`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "deny_powerful_vassal",
            scalar(Setting)
                .doc("Characters never become powerful vassals. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "use_maa_maintenance",
            scalar(Setting)
                .doc("Characters always pay men-at-arms maintenance. Default `yes`.")
                .values(&["yes", "no"]),
        ),
        (
            "no_capital_movement_cooldown",
            scalar(Setting)
                .doc("Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "redirects_wars_to_overlord",
            scalar(Setting).doc("Undocumented in the readme.").values(&["yes", "no"]),
        ),
        (
            "noble_families",
            scalar(Setting)
                .doc("Allows Noble Family titles to exist. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "house_aspirations",
            scalar(Setting)
                .doc("Use house aspirations rather than family attributes. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "replace_gold_cost_by_treasury",
            scalar(Setting)
                .doc(
                    "State expenses (title creation, holding buildings, mercenaries, title \
                     regiments) are paid from the treasury instead of gold. Requires \
                     `treasury = yes`. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "block_alliance_child_marriage",
            scalar(Setting)
                .doc("Children's weddings form no alliances. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "block_alliance_non_dominant_gender_child_marriage",
            scalar(Setting)
                .doc(
                    "Non-dominant-gender children's weddings form no alliances. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "always_use_patronym",
            scalar(Setting)
                .doc(
                    "Patronyms display when either the culture or the government sets this. \
                     Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "affected_by_development",
            scalar(Setting)
                .doc("Counties held under this government are affected by development.")
                .values(&["yes", "no"]),
        ),
        (
            "considers_piety_for_title_creation",
            scalar(Setting)
                .doc("Piety counts toward title creation. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "ask_for_tribute",
            scalar(Setting)
                .doc(
                    "Can ask others to become tributary — shows the tributarization_chance map \
                     mode and uses `offer_tributary_status_interaction`. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "barter",
            scalar(Setting)
                .doc("Enables the bartering system. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "buildings",
            scalar(Setting)
                .doc("Characters can build in their holdings. Default `yes`.")
                .values(&["yes", "no"]),
        ),
        (
            "count_tributaries_for_title_requirements",
            scalar(Setting)
                .doc(
                    "Tributaries' land counts toward title creation and usurpation \
                     requirements. At least one de jure county is still required. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "radiance",
            scalar(Setting)
                .doc("Grants access to the Radiance mechanics. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "disable_regnal_numbers",
            scalar(Setting)
                .doc("Disables regnal numbers for same-named title holders. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "allow_accolades",
            scalar(Setting)
                .doc("Rulers can grant accolades. *(corpus)*")
                .values(&["yes", "no"]),
        ),
        (
            "allow_as_base_for_baronies",
            scalar(Setting)
                .doc("Usable as the base government for baronies. *(corpus)*")
                .values(&["yes", "no"]),
        ),
        (
            "dynasty_named_non_independent_landed_rulers",
            scalar(Setting)
                .doc("Dynasty-based realm naming extends to non-independent landed rulers. *(corpus)*")
                .values(&["yes", "no"]),
        ),
        (
            "gain_legitimacy_becoming_tributary",
            scalar(Setting)
                .doc("Becoming a tributary grants legitimacy. *(corpus)*")
                .values(&["yes", "no"]),
        ),
        (
            "government_ignores_rightful_liege_penalties",
            scalar(Setting)
                .doc("Rightful-liege opinion penalties do not apply. *(corpus)*")
                .values(&["yes", "no"]),
        ),
        (
            "regiments_use_barter_goods_as_gold",
            scalar(Setting)
                .doc("Regiments are paid for with barter goods instead of gold. *(corpus)*")
                .values(&["yes", "no"]),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `ai = { … }` — per-government overrides of AI behaviour. Features may still
/// be disabled for other reasons (dependence, tier).
static GOVERNMENT_AI: StructSpec = StructSpec {
    name: "government ai",
    fields: &[
        (
            "use_lifestyle",
            scalar(Setting)
                .doc("The AI checks for lifestyles.")
                .values(&["yes", "no"]),
        ),
        (
            "arrange_marriage",
            scalar(Setting)
                .doc("Actively arrange marriages. Requests can still be received when off.")
                .values(&["yes", "no"]),
        ),
        (
            "use_goals",
            scalar(Setting)
                .doc("Use long-term goals — build holdings, take major decisions.")
                .values(&["yes", "no"]),
        ),
        (
            "use_decisions",
            scalar(Setting)
                .doc("Use minor decisions.")
                .values(&["yes", "no"]),
        ),
        (
            "use_scripted_guis",
            scalar(Setting)
                .doc("Evaluate scripted GUIs.")
                .values(&["yes", "no"]),
        ),
        (
            "use_legends",
            scalar(Setting)
                .doc("Create and promote legends.")
                .values(&["yes", "no"]),
        ),
        (
            "perform_religious_reformation",
            scalar(Setting)
                .doc("Attempt religious reformation.")
                .values(&["yes", "no"]),
        ),
        (
            "use_great_projects",
            scalar(Setting)
                .doc("Found and contribute to Great Projects. Default `no`.")
                .values(&["yes", "no"]),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `currency_levels_cap = { … }` — 0-based index of the highest level this
/// government permits per currency, capping whatever the character's
/// modifiers would otherwise reach. Defaults come from `00_define.txt`.
static CURRENCY_LEVELS_CAP: StructSpec = StructSpec {
    name: "currency levels cap",
    fields: &[
        ("piety", scalar(Setting).doc("Max Level of Devotion.")),
        ("prestige", scalar(Setting).doc("Max prestige level.")),
        ("influence", scalar(Setting).doc("Max influence level.")),
        ("merit", scalar(Setting).doc("Max merit level.")),
    ],
    fallback: Fallback::Deny,
};

/// The body of one government type (`_governments.info`, corpus-corrected).
static GOVERNMENT: StructSpec = StructSpec {
    name: "government type",
    fields: &[
        (
            "government_rules",
            block(Struct(&GOVERNMENT_RULES))
                .doc("Behaviour flags read from code and testable with `government_allows`/`government_disallows`."),
        ),
        (
            "mechanic_type",
            scalar(Setting)
                .doc(
                    "The government family this belongs to, used by code checks such as Nomad \
                     title creation. Unset by default; not every government declares one.",
                )
                .values(&[
                    "feudal",
                    "mercenary",
                    "holy_order",
                    "clan",
                    "theocracy",
                    "administrative",
                    "landless_adventurer",
                    "herder",
                    "nomad",
                    "mandala",
                ]),
        ),
        (
            "is_mechanic_type_default",
            scalar(Setting)
                .doc(
                    "This is the default government of its `mechanic_type`, used when spawning \
                     characters or changing government. Exactly one per type. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "fallback",
            scalar(Setting).doc(
                "Fallback priority — lower wins. At least one government must be a fallback; \
                 it is selected when nothing else is valid and when populating the map with \
                 holdings that have no county holder.",
            ),
        ),
        (
            "can_get_government",
            block_scoped(ClauseKind::Trigger, "character").doc(
                "Checked on becoming landed. Failing it denies this government — though a \
                 fallback still applies if nothing else is valid.",
            ),
        ),
        (
            "can_move_realm_capital",
            block_scoped(ClauseKind::Trigger, "character")
                .doc("Whether the ruler may move the realm capital. Allowed if unset."),
        ),
        (
            "primary_holding",
            scalar(Setting).doc("The primary holding type (`common/holdings/`)."),
        ),
        (
            "valid_holdings",
            block(Struct(&super::common::OPAQUE)).doc(
                "Holdings this government may hold directly (`common/holdings/`). The primary \
                 holding is always valid.",
            ),
        ),
        (
            "required_county_holdings",
            block(Struct(&super::common::OPAQUE)).doc(
                "Holdings that must exist in a county before more of a type can be built \
                 (`common/holdings/`).",
            ),
        ),
        (
            "generated_character_template",
            scalar(Setting).doc(
                "Template used to generate characters for this government \
                 (`common/scripted_character_templates/`). A generic random character is used \
                 if unset.",
            ),
        ),
        (
            "primary_heritages",
            block(Struct(&super::common::OPAQUE)).doc(
                "Heritages for which this government is valid and preferred (heritage-type \
                 entries of `common/culture/pillars/`). No cultural restriction applies when \
                 this and `primary_cultures` are both empty.",
            ),
        ),
        (
            "preferred_religions",
            block(Struct(&super::common::OPAQUE))
                .doc("Religions preferring this government (`common/religion/religion_types/`)."),
        ),
        (
            "court_generate_commanders",
            scalar(Setting).doc(
                "Generate commanders in courts of this government. `yes`/`no`, or an integer \
                 multiplier on the default count.",
            ),
        ),
        (
            "supply_limit_mult_for_others",
            scalar(Setting)
                .doc("Supply-limit multiplier applied to army owners of a different government."),
        ),
        (
            "prestige_opinion_override",
            block(Struct(&super::common::OPAQUE)).doc(
                "Overrides the opinion bonus per prestige level. The value count must match \
                 `NCharacterOpinion::PRESTIGIOUS`.",
            ),
        ),
        (
            "royal_court",
            scalar(Setting)
                .doc(
                    "Royal-court availability for rulers of the tier in \
                     `NRoyalCourt::MIN_ROYAL_COURT_TIER`. `none` — no court and no vassal \
                     limit; `any` — court, no vassal limit; `top_liege` — only independent \
                     rulers get a court, and no vassal of any government may have one.",
                )
                .values(&["none", "any", "top_liege"]),
        ),
        (
            "blocked_subject_courts",
            block(Struct(&super::common::OPAQUE)).doc(
                "Vassals of these government types may not hold their own royal court, on top \
                 of the `royal_court` restriction. Empty or absent adds no restriction.",
            ),
        ),
        (
            "main_administrative_tier",
            scalar(Setting)
                .doc("Title tier enabling most administrative mechanics — title troops, map modes.")
                .values(TIERS),
        ),
        (
            "min_appointment_tier",
            scalar(Setting)
                .doc("Title tier enabling appointment succession.")
                .values(TIERS),
        ),
        (
            "minimum_provincial_maa_tier",
            scalar(Setting)
                .doc(
                    "Lowest title tier allowed title troops. Administrative governments only; \
                     defaults to `duchy`.",
                )
                .values(TIERS),
        ),
        (
            "administrative_title_maa_setup",
            scalar(Setting)
                .doc(
                    "Which administrative titles may raise title troops. *(corpus — the readme \
                     calls this `title_maa_setup`.)*",
                )
                .values(&[
                    "main_administrative_tier_and_top_liege",
                    "vassals_and_top_liege",
                    "top_vassals_and_top_liege",
                ]),
        ),
        (
            "vassal_contract_group",
            scalar(Setting).doc(
                "The vassal obligation set used by vassals of this government \
                 (`common/vassal_contracts/`). *(corpus — the readme calls this \
                 `vassal_contract` and shows a block.)*",
            ),
        ),
        (
            "house_unity",
            scalar(Setting).doc("House-unity configuration (`common/house_unities/`)."),
        ),
        (
            "domicile_type",
            scalar(Setting).doc("Domicile configuration (`common/domiciles/`)."),
        ),
        (
            "tax_slot_type",
            scalar(Setting).doc("Tax-slot configuration (`common/tax_slot_types/`). *(corpus)*"),
        ),
        (
            "opinion_of_liege",
            block(ScriptValue)
                .doc("Vassal opinion of their liege, specific to this government. Variables: `liege`, `vassal`."),
        ),
        (
            "opinion_of_liege_desc",
            block(DynamicDesc)
                .doc("Description for `opinion_of_liege`. Variables: `liege`, `vassal`."),
        ),
        (
            "opinion_of_suzerain",
            block(ScriptValue).doc(
                "Tributary opinion of their suzerain, specific to this government. Variables: \
                 `suzerain`, `tributary`.",
            ),
        ),
        (
            "opinion_of_suzerain_desc",
            block(DynamicDesc)
                .doc("Description for `opinion_of_suzerain`. Variables: `suzerain`, `tributary`."),
        ),
        (
            "opinion_of_overlord",
            block(ScriptValue).doc(
                "Subject opinion of their overlord — use this instead of the liege and suzerain \
                 pair when it should apply to both. Variables: `overlord`, `subject`.",
            ),
        ),
        (
            "opinion_of_overlord_desc",
            block(DynamicDesc)
                .doc("Description for `opinion_of_overlord`. Variables: `overlord`, `subject`."),
        ),
        (
            "currency_levels_cap",
            block(Struct(&CURRENCY_LEVELS_CAP))
                .doc("Caps the piety, prestige, influence and merit levels this government allows."),
        ),
        (
            "compatible_government_type_succession",
            block(Struct(&super::common::OPAQUE)).doc(
                "Additional government types eligible for succession here — normally only \
                 characters of the same government can be appointed.",
            ),
        ),
        (
            "ai",
            block(Struct(&GOVERNMENT_AI)).doc("Overrides for AI behaviour under this government."),
        ),
        (
            "character_modifier",
            block(StaticModifier).doc("Modifier applied to any ruler of this government."),
        ),
        (
            "top_liege_character_modifier",
            block(StaticModifier).doc(
                "Modifier applied on top of `character_modifier` when the ruler is an \
                 independent top liege.",
            ),
        ),
        (
            "max_dread",
            scalar(Setting).doc("Caps the dread a ruler of this government accumulates. *(corpus)*"),
        ),
        ("color", color().doc("Color for the government map mode.")),
        (
            "realm_mask_scale",
            scalar(Setting).doc("Scale of the realm map mask texture. *(corpus)*"),
        ),
        (
            "realm_mask_offset",
            scalar(Setting).doc("Offset of the realm map mask texture. *(corpus)*"),
        ),
        (
            "ai_ruler_desired_kingdom_titles",
            scalar(Setting).doc(
                "How many kingdoms an AI ruler keeps, giving away the excess. Negative keeps \
                 all. Defaults to `AI_RULER_DESIRED_KINGDOM_TITLES_DEFAULT`. Root is the ruler.",
            ),
        ),
        (
            "ai_ruler_desired_empire_titles",
            scalar(Setting).doc(
                "How many empires an AI ruler keeps, giving away the excess. Negative keeps \
                 all. Defaults to `AI_RULER_DESIRED_EMPIRE_TITLES_DEFAULT`. Root is the ruler.",
            ),
        ),
        (
            "ai_can_reassign_council_positions",
            block_scoped(ClauseKind::Trigger, "character").doc(
                "Whether an AI ruler may reassign council positions. Default `yes`. Root is the \
                 council owner.",
            ),
        ),
        (
            "flags",
            block(Struct(&super::common::OPAQUE)).doc(
                "Free-form flags testable with `government_has_flag = X`. Not definitions — any \
                 string works.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Government;

impl Entity for Government {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::GOVERNMENT,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: GOVERNMENTS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("has_government")),
            anywhere(RefPattern::KeyValue("change_government")),
            anywhere(RefPattern::KeyValue("government")),
            anywhere(RefPattern::KeyValue("invalid_for_government")),
            anywhere(RefPattern::KeyValue("fallback_government")),
            gov_list("blocked_subject_courts"),
            gov_list("compatible_government_type_succession"),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(GOVERNMENTS_DIR, ClauseKind::Struct(&GOVERNMENT))];
}
