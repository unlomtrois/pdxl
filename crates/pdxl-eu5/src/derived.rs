//! Table-driven reference derivation — the productionized narrow slice of
//! the table-driven-references experiment (see `tests/derived_proof.rs`,
//! the measurement harness that decided its scope):
//!
//! - **Scope-link `ScopePrefix` rules**: every data-carrying link in the
//!   generated `SCOPE_LINKS` table (`c:`, `culture:`, `estate_type:`, …)
//!   is a literal-name reference by construction (`requires_data: yes`,
//!   typed `output_scopes`). One curated scope-type → kind map turns them
//!   all into rules; regenerating the tables after a game patch grows
//!   navigation automatically.
//! - **Skip words**: every argument-less link (`owner`, `overlord`, …) and
//!   code-saved scope name is relative navigation, never a literal key —
//!   derived instead of hand-curated.
//!
//! What the proof rejected: deriving `KeyValue` rules from effect/trigger
//! `targets` — those document runtime scope typing, and the arguments are
//! scope expressions, not literal names (+531 keys yielded zero refs).
//! Literal-name keys (`tag`, `has_or_had_tag`) stay hand-curated.
//!
//! Corpus validation at adoption (game + mod): culture 4,945 literal refs
//! (4 unresolved — cut cultures `manekenk/cunco/yaros_culture`), religion
//! 4,150, government_reform 1,374, casus_belli 761, and the long tail —
//! everything else 0 unresolved.

use pdxl_analysis::{IconHint, KindId, KindSpec, RefPattern, RefRule};

use crate::kinds;
use crate::tables;

/// Output scope-type → kind (+ alternates). The one curated artifact:
/// grows when a new kind is modeled, not per link.
const TARGET_KINDS: &[(&str, KindId, &[KindId])] = &[
    (
        "country",
        kinds::COUNTRY,
        &[
            kinds::FORMABLE_COUNTRY,
            kinds::START_COUNTRY,
            kinds::DYNAMIC_COUNTRY,
        ],
    ),
    ("culture", kinds::CULTURE, &[]),
    ("religion", kinds::RELIGION, &[]),
    ("estate_type", kinds::ESTATE, &[]),
    ("age", kinds::AGE, &[]),
    ("advance_type", kinds::ADVANCE, &[]),
    ("subject_type", kinds::SUBJECT_TYPE, &[]),
    ("building", kinds::BUILDING, &[]),
    ("law", kinds::LAW, &[]),
    ("casus_belli", kinds::CASUS_BELLI, &[]),
    ("government_reform", kinds::GOVERNMENT_REFORM, &[]),
    ("production_method", kinds::PRODUCTION_METHOD, &[]),
    ("unit_type", kinds::UNIT, &[]),
    ("unit_ability", kinds::UNIT_ABILITY, &[]),
    ("relation_type", kinds::RELATION_TYPE, &[]),
    ("character_interaction", kinds::CHARACTER_INTERACTION, &[]),
    ("country_interaction", kinds::COUNTRY_INTERACTION, &[]),
    ("formable_country", kinds::FORMABLE_COUNTRY, &[]),
    // Both the IO type-tag link (`international_organization:hre`) and the
    // type-comparison link resolve to the type defs.
    (
        "international_organization",
        kinds::INTERNATIONAL_ORGANIZATION,
        &[],
    ),
    (
        "international_organization_type",
        kinds::INTERNATIONAL_ORGANIZATION,
        &[],
    ),
    ("special_status", kinds::IO_SPECIAL_STATUS, &[]),
    ("culture_group", kinds::CULTURE_GROUP, &[]),
    ("language", kinds::LANGUAGE, &[]),
    ("parliament_type", kinds::PARLIAMENT_TYPE, &[]),
    ("institution", kinds::INSTITUTION, &[]),
    ("situation", kinds::SITUATION, &[]),
    ("location", kinds::LOCATION, &[]),
    ("province", kinds::PROVINCE, &[]),
    ("province_definition", kinds::PROVINCE, &[]),
    ("area", kinds::AREA, &[]),
    ("region", kinds::REGION, &[]),
    ("sub_continent", kinds::SUB_CONTINENT, &[]),
    ("continent", kinds::CONTINENT, &[]),
    ("religion", kinds::RELIGION, &[]),
    ("religious_aspect", kinds::RELIGIOUS_ASPECT, &[]),
    ("religious_faction", kinds::RELIGIOUS_FACTION, &[]),
    ("religious_figure", kinds::RELIGIOUS_FIGURE, &[]),
    ("religious_focus", kinds::RELIGIOUS_FOCUS, &[]),
    ("religious_school", kinds::RELIGIOUS_SCHOOL, &[]),
];

/// `ScopePrefix` rules derived from the scope-link table: one per
/// data-carrying link whose output scope-type is mapped. Leaked once (the
/// context-roots pattern) — rules live for the process.
pub fn derived_link_rules() -> Vec<KindSpec> {
    let mut out = Vec::new();
    for (scope_type, kind, alts) in TARGET_KINDS {
        let rules: Vec<RefRule> = tables::SCOPE_LINKS
            .iter()
            .filter(|l| l.requires_data && l.output_scopes.contains(scope_type))
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

/// Table-driven skip words: argument-less scope links and code-saved scope
/// names — relative navigation, never literal keys.
pub fn derived_skip_words() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = tables::SCOPE_LINKS
        .iter()
        .filter(|l| !l.requires_data)
        .map(|l| l.name)
        .collect();
    out.extend_from_slice(tables::CODE_SAVED_SCOPES);
    out
}
