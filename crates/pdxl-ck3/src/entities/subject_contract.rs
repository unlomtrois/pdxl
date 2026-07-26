//! Subject contracts (`common/subject_contracts/`) — the obligations a vassal
//! or tributary owes their liege, and the groups that bundle them per
//! government type.
//!
//! Three kinds live here:
//!
//! - **contracts** (`contracts/`, 73 defs) — one negotiable obligation each.
//! - **groups** (`groups/`, 27 defs) — the contract bundle a government uses.
//!
//! **Obligation levels are modeled structurally but are not a kind.** The 201
//! rungs inside `obligation_levels = { … }` look like nested definitions — the
//! faiths-inside-religions shape — and `parent = X` links a rung to the one it
//! steps from, with all 119 distinct parents resolving. But level names are
//! scoped *per contract*, not globally: `default` occurs in ten contracts,
//! `salary_low` and `prestige_transfer_none` in several each. Registering them
//! as symbols produced 21 false "redefined" diagnostics against the corpus,
//! because this symbol table is global and has no per-parent scoping (the one
//! scoped channel, file-local `@constants`, is a bespoke mechanism). So levels
//! get a full [`StructSpec`] — hover, completion and field validation all work
//! inside them — while `parent` stays an ordinary setting rather than a
//! resolvable reference.
//!
//! The directory is `subject_contracts`, not the `common/vassal_contracts/`
//! the governments readme still names; that path does not exist in either
//! vanilla or T4N. Script keys keep the older `vassal_` prefix throughout,
//! which is why the reference rules read inconsistently with the directory.
//!
//! Two reference shapes carry a contract inside a block rather than as a
//! value: `vassal_contract_set_obligation_level = { type = X level = N }` and
//! its `tributary_` twin. `type` is far too common a key to match on its own —
//! it names event types, interaction types, task-contract types — so those use
//! [`RefPattern::KeyBlockField`], which is tree-shaped and catches the inline
//! and multi-line forms alike. `start_tributary = { contract_group = X }` is
//! the same shape for groups.
//!
//! **Deliberate omissions.** `flag`, `gui_tags` and `appointment_trait_flag`
//! are free-form tokens checked in script, never definitions —
//! `appointment_trait_flag` names a *trait flag*, not a trait.
//! `suzerain_line_type`/`tributary_line_type` point at `gfx/lines/lines.lines`,
//! which this schema does not model. `tributary_contract_obligation_level_can_be_decreased`
//! is left out because the corpus never uses it, though its `_increased` twin
//! is attested 16 times; add it when a corpus does.
//!
//! Corpus-only fields — used but absent from either readme — are marked
//! *(corpus)*: the obligation-level `color`, `prestige` and `piety`, and the
//! contract-level `icon`.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{
    self, DynamicDesc, ScriptValue, StaticModifier, Struct, Trigger,
};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, color, scalar, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{OPAQUE, anywhere};

pub(crate) const CONTRACTS_DIR: &str = "common/subject_contracts/contracts/";
pub(crate) const GROUPS_DIR: &str = "common/subject_contracts/groups/";

/// A reference gated to the group directory.
const fn in_groups(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(GROUPS_DIR),
        alt: &[],
    }
}

/// One rung of a contract — the level a subject's obligation currently sits at.
static OBLIGATION_LEVEL: StructSpec = StructSpec {
    name: "obligation level",
    fields: &[
        (
            "levies",
            scalar_or_block(Setting, ScriptValue)
                .doc("Share of levies owed, 0..1. Default 0. Accepts script math."),
        ),
        (
            "tax",
            scalar_or_block(Setting, ScriptValue)
                .doc("Share of gold income owed, 0..1. Default 0. Accepts script math."),
        ),
        (
            "herd",
            scalar_or_block(Setting, ScriptValue)
                .doc("Share of herd income owed, 0..1. Default 0. Accepts script math."),
        ),
        (
            "barter_goods",
            scalar_or_block(Setting, ScriptValue)
                .doc("Share of barter-goods income owed, 0..1. Default 0. Accepts script math."),
        ),
        (
            "prestige",
            scalar_or_block(Setting, ScriptValue).doc("Prestige owed. *(corpus)*"),
        ),
        (
            "piety",
            scalar_or_block(Setting, ScriptValue).doc("Piety owed. *(corpus)*"),
        ),
        (
            "min_levies",
            scalar_or_block(Setting, ScriptValue).doc("Optional floor under `levies`."),
        ),
        (
            "min_tax",
            scalar_or_block(Setting, ScriptValue).doc("Optional floor under `tax`."),
        ),
        (
            "min_herd",
            scalar_or_block(Setting, ScriptValue).doc("Optional floor under `herd`."),
        ),
        (
            "min_barter_goods",
            scalar_or_block(Setting, ScriptValue).doc("Optional floor under `barter_goods`."),
        ),
        (
            "tax_factor",
            scalar_or_block(Setting, ScriptValue).doc(
                "Multiplier on the contract's total tax. Beware stacking when several are \
                 active at once.",
            ),
        ),
        (
            "levies_factor",
            scalar_or_block(Setting, ScriptValue).doc("Multiplier on the contract's total levies."),
        ),
        (
            "herd_factor",
            scalar_or_block(Setting, ScriptValue).doc("Multiplier on the contract's total herd."),
        ),
        (
            "contribution_desc",
            block(DynamicDesc)
                .doc("Description for the tax, levies and herd contribution breakdown."),
        ),
        (
            "tax_contribution_postfix",
            scalar(Setting).doc("Postfix appended to the tax contribution breakdown."),
        ),
        (
            "levies_contribution_postfix",
            scalar(Setting).doc("Postfix appended to the levies contribution breakdown."),
        ),
        (
            "herd_contribution_postfix",
            scalar(Setting).doc("Postfix appended to the herd contribution breakdown."),
        ),
        (
            "unclamped_contribution_label",
            scalar(Setting).doc("Breakdown label for the unclamped contribution."),
        ),
        (
            "min_contribution_label",
            scalar(Setting).doc("Breakdown label for the minimum the value is clamped to."),
        ),
        (
            "subject_opinion",
            scalar(Setting)
                .doc("Opinion of the liege added to the subject while this level is active."),
        ),
        (
            "flag",
            scalar(Setting).doc(
                "Arbitrary token, checked in script to see whether any level of the current \
                 contract carries it. Not a definition.",
            ),
        ),
        (
            "gui_tags",
            block(Struct(&OPAQUE))
                .doc("Tags driving size, color and so on in gui views. Free-form."),
        ),
        (
            "score",
            scalar(Setting).doc(
                "Positive favours the subject, negative the liege, 0 is neutral. Compared \
                 against the current level when obligations change. Defaults to definition \
                 order.",
            ),
        ),
        (
            "ai_liege_desire",
            scalar_or_block(Setting, ScriptValue)
                .doc("How much the liege wants this level. Desires at or below zero are ignored."),
        ),
        (
            "ai_subject_desire",
            scalar_or_block(Setting, ScriptValue).doc("How much the subject wants this level."),
        ),
        (
            "liege_modifier",
            block(StaticModifier).doc("Character modifiers applied to the liege."),
        ),
        (
            "subject_modifier",
            block(StaticModifier).doc("Character modifiers applied to the subject."),
        ),
        (
            "is_shown",
            block(Trigger).doc("Whether this level is visible. Invisible levels are also invalid."),
        ),
        (
            "is_valid",
            block(Trigger).doc("Whether this level is valid."),
        ),
        (
            "enable_title_maa",
            scalar_or_block(Setting, ScriptValue).doc(
                "Allow title men-at-arms based on the title or governor. Requires the \
                 government's `administrative` rule. Default `yes`; when several obligations \
                 define it, the first in the contract group wins.",
            ),
        ),
        (
            "enable_character_maa",
            scalar_or_block(Setting, ScriptValue)
                .doc("Allow character men-at-arms. Same defaulting as `enable_title_maa`."),
        ),
        (
            "appointment_trait_flag",
            scalar(Setting).doc(
                "Meritocratic appointment succession: heirs and candidates need a trait \
                 carrying this *flag*. Levels with one are treated as valid at load time to \
                 dodge initialization ordering. Names a trait flag, not a trait.",
            ),
        ),
        (
            "default",
            scalar(Setting)
                .doc("Marks this the default level; otherwise the first one is.")
                .values(&["yes", "no"]),
        ),
        (
            "parent",
            scalar(Setting).doc(
                "The level this one steps from, and can step back to. Names a level of the \
                 *same contract*; not a global symbol, so it is not resolved as a reference.",
            ),
        ),
        (
            "position",
            block(Struct(&OPAQUE)).doc(
                "`{ x y }` placement of the icon in the modify-contract view, scaled by \
                 `NSubjectContract::OBLIGATION_OFFSET`.",
            ),
        ),
        ("icon", scalar(Setting).doc("Icon shown in the UI.")),
        (
            "color",
            color().doc("Color used for this level in the contract view. *(corpus)*"),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `obligation_levels = { … }` — keyed by level name, each a rung.
static OBLIGATION_LEVELS: StructSpec = StructSpec {
    name: "obligation levels",
    fields: &[],
    fallback: Fallback::Struct(&OBLIGATION_LEVEL),
};

/// The body of one subject contract (`_subject_contracts.info`).
static SUBJECT_CONTRACT: StructSpec = StructSpec {
    name: "subject contract",
    fields: &[
        (
            "obligation_levels",
            block(Struct(&OBLIGATION_LEVELS)).doc(
                "The rungs this contract can sit on, keyed by level name — the key is also \
                 the localization key.",
            ),
        ),
        (
            "display_mode",
            scalar(Setting)
                .doc("How the obligation is drawn in the negotiate-contract UI.")
                .values(&["tree", "radiobutton", "checkbox", "hidden"]),
        ),
        (
            "is_shown",
            block(Trigger)
                .doc("Whether this obligation is shown. Same scopes as `obligation_levels`."),
        ),
        (
            "can_be_changed",
            block(Trigger).doc(
                "Whether the option can be modified. Blockers show in the tooltip and the \
                 option stays visible but unclickable. Scopes: `liege`, `subject`/`vassal`, \
                 `tax_slot`, `tax_collector`, and `opinion_of_liege` when \
                 `uses_opinion_of_liege = yes`.",
            ),
        ),
        (
            "defaults_to_highest_valid_level",
            scalar(Setting)
                .doc("Default to the highest-scoring valid level rather than a fixed one. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "uses_opinion_of_liege",
            scalar(Setting)
                .doc(
                    "Makes `scope:opinion_of_liege` available in the levies and tax script \
                     math. Updated daily for player contracts, on \
                     `NSubjectContract::OPINION_OF_LIEGE_UPDATE_INTERVAL` for the AI. Default \
                     `no`, for performance.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "joins_suzerain_wars",
            scalar(Setting)
                .doc("Whether a tributary joins their suzerain's wars automatically. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "icon",
            scalar(Setting).doc("Icon shown for the contract itself. *(corpus)*"),
        ),
    ],
    fallback: Fallback::Deny,
};

/// The body of one contract group (`_subject_contract_groups.info`).
static CONTRACT_GROUP: StructSpec = StructSpec {
    name: "subject contract group",
    fields: &[
        (
            "contracts",
            block(Struct(&OPAQUE)).doc("The subject contracts bundled into this group."),
        ),
        (
            "admin_province_contract",
            scalar(Setting).doc(
                "Which contract in this group is the administrative province-type contract, \
                 used by the interface. It may also be listed in `contracts`.",
            ),
        ),
        (
            "modify_contract_layout",
            scalar(Setting).doc(
                "String read by `SubjectContract.HasModifyContractLayout` in gui script to \
                 pick the Modify Contract window layout. Default `default`.",
            ),
        ),
        (
            "is_tributary",
            scalar(Setting)
                .doc("Whether this group is specifically for tributaries.")
                .values(&["yes", "no"]),
        ),
        (
            "is_valid_tributary_contract",
            block(Trigger).doc(
                "Whether the contract is valid. `ROOT` is the subject, `scope:suzerain` the \
                 suzerain.",
            ),
        ),
        (
            "tributary_can_break_free",
            block(Trigger)
                .doc("Whether the subject can break free unaided. Same scopes as `is_valid`."),
        ),
        (
            "suzerain_line_type",
            scalar(Setting).doc(
                "Map line drawn toward a selected character's suzerain, from \
                 `gfx/lines/lines.lines`. Omit for no line.",
            ),
        ),
        (
            "tributary_line_type",
            scalar(Setting).doc("Map line drawn toward a selected character's tributaries."),
        ),
        (
            "should_show_as_suzerain_realm_name",
            scalar(Setting)
                .doc("Draw the tributary's realm under the suzerain's realm name. Default `no`.")
                .values(&["yes", "no"]),
        ),
        (
            "should_show_as_suzerain_realm_color",
            scalar(Setting)
                .doc(
                    "Draw the tributary's realm in the suzerain's realm color — actually an \
                     interpolation at `TRIBUTARY_REALM_COLOR_FACTOR`. Default `no`.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "tributary_heir_succession",
            scalar(Setting)
                .doc("Whether the tributary's heirs stay tributaries on succession. Default `yes`.")
                .values(&["yes", "no"]),
        ),
        (
            "suzerain_heir_succession",
            scalar(Setting)
                .doc("Whether the suzerain's primary heir takes over as suzerain. Default `yes`.")
                .values(&["yes", "no"]),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct SubjectContract;

impl Entity for SubjectContract {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::SUBJECT_CONTRACT,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: CONTRACTS_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                anywhere(RefPattern::KeyValue(
                    "vassal_contract_obligation_level_can_be_increased",
                )),
                anywhere(RefPattern::KeyValue(
                    "vassal_contract_obligation_level_can_be_decreased",
                )),
                anywhere(RefPattern::KeyValue(
                    "vassal_contract_increase_obligation_level",
                )),
                anywhere(RefPattern::KeyValue(
                    "vassal_contract_decrease_obligation_level",
                )),
                anywhere(RefPattern::KeyValue(
                    "tributary_contract_obligation_level_can_be_increased",
                )),
                // `type` alone is far too common; the wrapper key disambiguates.
                anywhere(RefPattern::KeyBlockField(
                    "vassal_contract_set_obligation_level",
                    "type",
                )),
                anywhere(RefPattern::KeyBlockField(
                    "tributary_contract_set_obligation_level",
                    "type",
                )),
                in_groups(RefPattern::KeyList("contracts")),
                in_groups(RefPattern::KeyValue("admin_province_contract")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::SUBJECT_CONTRACT_GROUP,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: GROUPS_DIR,
                shape: DefShape::TopLevel,
            }),
            refs: &[
                anywhere(RefPattern::KeyValue("has_subject_contract_group")),
                anywhere(RefPattern::KeyValue("vassal_contract_group")),
                anywhere(RefPattern::KeyBlockField(
                    "start_tributary",
                    "contract_group",
                )),
            ],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[
        (CONTRACTS_DIR, ClauseKind::Struct(&SUBJECT_CONTRACT)),
        (GROUPS_DIR, ClauseKind::Struct(&CONTRACT_GROUP)),
    ];
}
