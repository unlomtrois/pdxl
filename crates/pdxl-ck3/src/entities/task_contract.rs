//! Task contracts (`common/task_contracts/`, from `_task_contracts.info`) —
//! the jobs landless adventurers and administrative governors take on:
//! validity gates, lifecycle effects, weighted spawning, and named reward
//! outcomes.
//!
//! Two kinds. The contract type (159 vanilla + 2 T4N defs) and its rewards as
//! `ScopedChildrenOf` under `task_contract_reward` — the same reward key
//! recurs under nearly every contract (`success_standard`,
//! `failure_standard`, `success_critical`), so a repeat gap-fills instead of
//! reporting a duplicate, and `complete_task_contract = success_standard`
//! (455 uses) navigates to them.
//!
//! Cross-references, each corpus-validated over game + T4N:
//! - `task_contract_type:X` resolves through the table-derived scope-link
//!   rule (`derived.rs`).
//! - `task_contract_type = X` — one rule covers both `create_task_contract`'s
//!   mandatory parameter and the optional filter of all twelve
//!   `any/every/random/ordered_[character_[active_]]task_contract` iterator
//!   forms; every corpus value is a literal type key.
//! - `can_create_task_contract` in both documented forms: scalar and
//!   `{ type_name = X employer = … }` (285 block uses).
//! - `has_task_contract_type = X`.
//!
//! Where the info and the corpus disagree, both directions this time:
//! - `<key>_desc_title` is the contract's display name (153/159) — the info
//!   never mentions it, documenting only `<key>_desc` and `<key>_request`.
//! - `desc`, `task_contract_request` and `should_show_toast_on_complete` are
//!   documented fields with **zero** corpus uses (the loc-key defaults do all
//!   the work); kept, since the engine reads them.
//!
//! Implicit localization (measured over the 159 vanilla contracts):
//! `<key>_desc` 153, `<key>_desc_title` 153 *(corpus)*, `<key>_request` 121.
//! Bare `<key>` (41/159) is *not* a convention and is omitted, as is loc for
//! reward names (none exists — reward text lives in the effects).
//!
//! Deliberate omission: contract *groups* (`group = laamp_contracts_…`,
//! `has_task_contract_group = X`, `populate_task_contracts_for_area`'s
//! `group` list) are tags with no defining site — they exist only by
//! appearing in contracts' `group` fields. The nested-value-defs mechanism
//! does not fit (ungated, any-file, and duplicate-tracked: 159 contracts
//! sharing ~10 tags would diagnose hundreds of duplicates), so groups stay
//! unmodeled, like activity guest subsets.

use pdxl_analysis::context::ClauseKind::{self, DynamicDesc, Effect, ScriptValue, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, block_scoped, scalar_or_block};
use pdxl_analysis::{DefShape, DefSource, IconHint, ImplicitLocPattern, KindSpec, RefPattern};

use crate::kinds;

use super::Entity;
use super::common::{anywhere, toggle};

const DIR: &str = "common/task_contracts/";

/// One `task_contract_reward = { <name> = { … } }` entry.
static REWARD: StructSpec = StructSpec {
    name: "task_contract_reward",
    fields: &[
        (
            "effect",
            block_scoped(Effect, "task_contract")
                .doc("The reward (or penalty) effects, run on completion."),
        ),
        (
            "visible",
            toggle(
                "Show this possible reward in the UI beforehand. It still prints in the \
                 completion effect either way. Default `yes`.",
            ),
        ),
        (
            "positive",
            toggle("Listed 'Upon Success' rather than 'Upon Failure'. Default `yes`."),
        ),
        (
            "should_print_on_complete",
            toggle("Print this reward's effect description on completion. Default `no`."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `task_contract_reward = { <name> = { … } }`.
static REWARDS: StructSpec = StructSpec {
    name: "task_contract_rewards",
    fields: &[],
    fallback: Fallback::Struct(&REWARD),
};

/// The body of one task-contract type.
static TASK_CONTRACT: StructSpec = StructSpec {
    name: "task_contract_type",
    fields: &[
        (
            "group",
            pdxl_analysis::context::scalar(Setting).doc(
                "Grouping tag for `populate_task_contracts_for_area` and \
                 `has_task_contract_group`; may determine the map icon. Tags are \
                 free-form — they exist only by being named here.",
            ),
        ),
        (
            "icon",
            pdxl_analysis::context::scalar(Setting).doc("Icon path used in the UI."),
        ),
        (
            "desc",
            scalar_or_block(LocKey, DynamicDesc).doc(
                "Back-story of the contract; defaults to `<key>_desc`. Root is the \
                 task-contract type. The corpus always uses the default. The display \
                 name is `<key>_desc_title` *(corpus)*.",
            ),
        ),
        (
            "task_contract_request",
            scalar_or_block(LocKey, DynamicDesc).doc(
                "The 'what to do' request text; defaults to `<key>_request`. The corpus \
                 always uses the default.",
            ),
        ),
        (
            "travel",
            toggle(
                "The owner must travel to the contract location to accept, and stay for \
                 the duration. Default `no`.",
            ),
        ),
        (
            "is_criminal",
            toggle("The contract is of a criminal nature. Default `no`."),
        ),
        (
            "use_diplomatic_range",
            toggle(
                "`yes`: offered within diplomatic range of the employer; `no`: within \
                 the ADVENTURER_DISTANCE_RESTRICTION radius. Default `no`.",
            ),
        ),
        (
            "valid_to_create",
            block_scoped(Trigger, "character")
                .doc("Can the contract appear? Root is the owner; `scope:employer` may be empty."),
        ),
        (
            "valid_to_accept",
            block_scoped(Trigger, "character")
                .doc("Can it be accepted? Root is the owner; `scope:employer` may be empty."),
        ),
        (
            "valid_to_continue",
            block_scoped(Trigger, "task_contract")
                .doc("Failing this invalidates an accepted contract. Root is the contract."),
        ),
        (
            "valid_to_keep",
            block_scoped(Trigger, "task_contract")
                .doc("Failing this invalidates a not-yet-taken contract. Root is the contract."),
        ),
        (
            "on_create",
            block_scoped(Effect, "task_contract")
                .doc("Fires when the contract is created (`create_task_contract`)."),
        ),
        (
            "on_accepted",
            block_scoped(Effect, "task_contract")
                .doc("Fires when the contract is accepted (`accept_task_contract`)."),
        ),
        (
            "on_completed",
            block_scoped(Effect, "task_contract").doc(
                "Fires on successful completion, alongside the picked reward \
                 (`complete_task_contract`).",
            ),
        ),
        (
            "on_invalidated",
            block_scoped(Effect, "task_contract").doc(
                "Fires when the contract invalidates (`valid_to_continue` fails, or \
                 `invalidate_task_contract`).",
            ),
        ),
        (
            "should_show_toast_on_complete",
            toggle("Show the completed-contract toast animation. Default `no`."),
        ),
        (
            "task_contract_reward",
            block(Struct(&REWARDS))
                .doc("Named reward outcomes; `complete_task_contract = <name>` picks one."),
        ),
        (
            "weight",
            block_scoped(ScriptValue, "character").doc(
                "How likely this type is picked when populating an area. Root is the \
                 owner; `scope:employer` may be empty.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct TaskContract;

impl Entity for TaskContract {
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[
        ImplicitLocPattern {
            kind: kinds::TASK_CONTRACT_TYPE,
            suffix: "_desc",
        },
        // The display name; corpus-only — the info never mentions it.
        ImplicitLocPattern {
            kind: kinds::TASK_CONTRACT_TYPE,
            suffix: "_desc_title",
        },
        ImplicitLocPattern {
            kind: kinds::TASK_CONTRACT_TYPE,
            suffix: "_request",
        },
    ];

    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            kind: kinds::TASK_CONTRACT_TYPE,
            icon: IconHint::Action,
            defs: Some(DefSource {
                dir_prefix: DIR,
                shape: DefShape::TopLevel,
            }),
            // `task_contract_type:X` comes from the derived scope-link rule.
            refs: &[
                // `create_task_contract`'s mandatory parameter and the twelve
                // iterators' optional filter share one field key.
                anywhere(RefPattern::KeyValue("task_contract_type")),
                anywhere(RefPattern::KeyValue("can_create_task_contract")),
                anywhere(RefPattern::KeyBlockField(
                    "can_create_task_contract",
                    "type_name",
                )),
                anywhere(RefPattern::KeyValue("has_task_contract_type")),
            ],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::TASK_CONTRACT_REWARD,
            icon: IconHint::Tag,
            defs: Some(DefSource {
                dir_prefix: DIR,
                // Scoped: `success_standard` recurs under nearly every
                // contract, so a repeat gap-fills rather than duplicating.
                shape: DefShape::ScopedChildrenOf {
                    containers: &["task_contract_reward"],
                },
            }),
            refs: &[anywhere(RefPattern::KeyValue("complete_task_contract"))],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(DIR, Struct(&TASK_CONTRACT))];
}
