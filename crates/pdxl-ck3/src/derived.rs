//! Table-driven reference derivation, ported from `pdxl-eu5::derived` (whose
//! module doc records the measurement that decided the technique's scope).
//!
//! A link in the generated `SCOPE_LINKS` table names a literal key when it
//! carries data *and needs no input scope* — `requires_data: yes` with an empty
//! `input_scopes`. One curated scope-type → kind map turns those into
//! `ScopePrefix` rules, so regenerating the tables after a game patch grows
//! navigation with no schema edit.
//!
//! The empty-`input_scopes` half of that test is load-bearing, and its absence
//! is what made `character` look unusable at first: four links output a
//! `character` scope, but only `character:` takes a character key.
//! `court_position:`, `cp:` and `memory_participant:` navigate *from* an input
//! scope and their argument is a court-position type or a memory role — which
//! is why `character:` seemed to produce 2326 unresolved refs when the real
//! source was three sibling links being attributed to the same kind. Filtering
//! on input scopes rather than the `global_link` flag also keeps `dynasty:`,
//! whose data-carrying row is marked non-global yet takes a literal key.
//!
//! EU5 also derives **skip words** from the same table (argument-less links and
//! code-saved scope names). That half does *not* transplant, measured here and
//! rejected: CK3's `CODE_SAVED_SCOPES` contains `war`, `faith`, `dynasty`,
//! `culture`, `province`, `title` — and CK3 has event themes literally named
//! `war`, `travel`, `faith` and `dynasty`. Skipping those values globally cost
//! 617 event-theme refs, 286 portrait-animation refs and 213 game-concept refs,
//! all of which resolved. It would have bought 822 fewer bogus `loc_key` refs;
//! that is worth having, but as a targeted rule rather than by suppressing
//! every value that shares a name with a runtime scope.
//!
//! Corpus at adoption (game + T4N, `tests/derived_proof.rs`): **+2753 refs, 9
//! unresolved, 99.7%** — trait 1401, character 1085, doctrine 130, dynasty 36,
//! religion 34, decision 34, subject_contract 25, casus_belli 5, government 3.
//! Every miss is a dynasty and every one is real: T4N overrides
//! `common/dynasties/05_tgp_dynasties.txt` without carrying over
//! `japanese_yamato`, `japanese_minamoto_seiwa` or `japanese_taira_kanmu`, yet
//! still references them, and two numeric ids are defined nowhere.
//!
//! What this deliberately does **not** derive, per the EU5 proof: `KeyValue`
//! rules from effect/trigger `targets`. Those columns document runtime scope
//! typing, not literal names — measured on this corpus, 78 of 79
//! `add_companion` arguments and 42 of 44 `add_attacker` arguments are scope
//! expressions. Keys naming a literal (`send_interface_toast`'s `type`, whose
//! meaning lives only in prose the dumps do not structure) stay hand-written.

use pdxl_analysis::{IconHint, KindId, KindSpec, RefPattern, RefRule};

use crate::kinds;
use crate::tables;

/// Output scope-type → kind (+ alternates). The one curated artifact: it grows
/// when a kind is *modeled*, not when the game adds a link.
///
/// Scope-type names are the game's, and do not always match ours — `landed_title`
/// is our `title`, `government_type` our `government`, `vassal_contract` our
/// `subject_contract` (renamed by the game after the tables were named).
///
/// Deliberately absent, and why:
/// - `value`, `flag` — numbers and runtime flags, not symbols.
/// - `situation_participant_group`, `situation_sub_region`, `geographical_region`
///   — real entities we do not model yet.
/// - `accolade_type` (627 corpus uses), `struggle` (330), `legend_type` (76),
///   `great_project_type` (44), plus `activity_type`, `confederation_type`,
///   `council_task`, `court_position_type`, `epidemic_type`, `house_aspiration`,
///   `house_relation_type`/`_level`, `task_contract_type` — each becomes one
///   line here the day its kind is modeled, and navigation follows for free.
const TARGET_KINDS: &[(&str, KindId, &[KindId])] = &[
    ("landed_title", kinds::TITLE, &[]),
    ("character", kinds::CHARACTER, &[]),
    ("province", kinds::PROVINCE, &[]),
    ("culture", kinds::CULTURE, &[]),
    ("culture_pillar", kinds::CULTURE_PILLAR, &[]),
    ("culture_tradition", kinds::CULTURE_TRADITION, &[]),
    ("culture_innovation", kinds::CULTURE_INNOVATION, &[]),
    ("faith", kinds::FAITH, &[]),
    ("religion", kinds::RELIGION, &[]),
    ("doctrine", kinds::DOCTRINE, &[]),
    ("trait", kinds::TRAIT, &[]),
    ("decision", kinds::DECISION, &[]),
    ("dynasty", kinds::DYNASTY, &[]),
    ("dynasty_house", kinds::DYNASTY_HOUSE, &[]),
    ("casus_belli_type", kinds::CASUS_BELLI, &[]),
    ("government_type", kinds::GOVERNMENT, &[]),
    ("holding_type", kinds::HOLDING, &[]),
    ("situation", kinds::SITUATION_TYPE, &[]),
    ("vassal_contract", kinds::SUBJECT_CONTRACT, &[]),
    (
        "vassal_contract_obligation_level",
        kinds::OBLIGATION_LEVEL,
        &[],
    ),
];

/// `ScopePrefix` rules derived from the scope-link table: one per data-carrying
/// link whose output scope-type is mapped. Leaked once (the context-roots
/// pattern) — rules live for the process.
pub fn derived_link_rules() -> Vec<KindSpec> {
    let mut out = Vec::new();
    for (scope_type, kind, alts) in TARGET_KINDS {
        let rules: Vec<RefRule> = tables::SCOPE_LINKS
            .iter()
            .filter(|l| {
                l.requires_data && l.input_scopes.is_empty() && l.output_scopes.contains(scope_type)
            })
            .map(|l| RefRule {
                pattern: RefPattern::ScopePrefix(l.name),
                gate: None,
                alt: alts,
            })
            .collect();
        if !rules.is_empty() {
            out.push(KindSpec {
                kind: *kind,
                icon: IconHint::Object,
                defs: None,
                refs: Box::leak(rules.into_boxed_slice()),
                aliases: &[],
            });
        }
    }
    out
}
