//! CK3 rules for `pdxl-analysis` — the game's conventions as *data*.
//!
//! Originally a transcription of the Go `internal/validate/schema_ck3.go`
//! registry. Since the landed-titles addition (`ANALYSIS_VERSION` 2) this
//! schema has grown past the Go implementation — the analysis layer's Go
//! oracle is retired; regressions are pinned by the facts golden snapshots in
//! this crate (`tests/facts.rs`) instead.
//!
//! Everything about one game concept lives in one file under [`entities`]:
//! its schema row(s) ([`KindSpec`] — def directory, reference shapes,
//! aliases, icon) and its structural context (see [`contexts`]). Adding a
//! concept is adding a file and one registry line — the schema-scaling
//! design (`rust/docs/SCHEMA-SCALING.md`).
//!
//! Deliberately hand-written and small: deep CK3 validation is ck3-tiger's
//! territory; this schema stays just rich enough to power editor features.
//! Bump [`pdxl_analysis::ANALYSIS_VERSION`] when a change alters what
//! previously extracted facts mean.
//!
//! [`KindSpec`]: pdxl_analysis::KindSpec

use pdxl_analysis::{CallKinds, GuiKinds, KindId, Schema};

mod entities;

pub mod contexts;
pub mod coverage;
pub mod derived;
pub mod kinds;
pub mod tables;

/// Reference values that are never symbol names, so no rule should try to
/// resolve them. Mostly relative-scope keywords a value may hold at runtime
/// (`has_trait = prev`), unresolvable without scope tracking. Game-wide — they
/// belong to no single concept.
const SCOPE_KEYWORDS: &[&str] = &[
    // The universal "nothing here" sentinel — `holding = none` in province
    // history is ~4900 of its uses. Nothing in the corpus is ever *named*
    // `none`, so skipping it game-wide costs no resolution.
    "none",
    // Title history's vacate sentinel — `holder = 0` (1439) and `liege = 0`
    // (1182). No symbol is ever named `0`: province ids start at 1, and no
    // numeric dynasty or character id takes it.
    "0",
    "root",
    "this",
    "prev",
    "prevprev",
    "prevprevprev",
    "prevprevprevprev",
    // Coat-of-arms color values that are relative references, not names:
    // `color1 = color2` reuses another slot of the same CoA, and
    // `color1 = list "x"` picks from a color list (the quoted list name is a
    // separate scalar the rule never sees).
    "color1",
    "color2",
    "color3",
    "color4",
    "color5",
    "list",
];

/// Typed-definition keywords: `KEYWORD NAME = { … }` defines `NAME` of the
/// given kind regardless of directory. CK3 uses these inline in event files
/// (`scripted_effect elope_outcome_effect = { … }`), and the same names are
/// invoked as call-by-name references (`NAME = yes`).
const TYPED_DEFS: &[(&str, KindId)] = &[
    ("scripted_effect", kinds::SCRIPTED_EFFECT),
    ("scripted_trigger", kinds::SCRIPTED_TRIGGER),
];

/// Keyed-value definitions: a top-level `KEY = value` whose *value* is the
/// symbol. `namespace = X` declares event namespace `X`.
const KEYED_VALUE_DEFS: &[(&str, KindId)] = &[("namespace", kinds::NAMESPACE)];

/// Doc-comment reference aliases: `![scheme:X]` pins the lookup to a kind.
const DOC_REF_ALIASES: &[(&str, KindId)] = &[
    ("effect", kinds::SCRIPTED_EFFECT),
    ("trigger", kinds::SCRIPTED_TRIGGER),
    ("value", kinds::SCRIPT_VALUE),
    ("trait", kinds::TRAIT),
    ("event", kinds::EVENT),
    ("decision", kinds::DECISION),
    ("on_action", kinds::ON_ACTION),
    ("character", kinds::CHARACTER),
    ("title", kinds::TITLE),
    ("culture", kinds::CULTURE),
    ("faith", kinds::FAITH),
    ("law", kinds::LAW),
    ("law_group", kinds::LAW_GROUP),
    ("scheme", kinds::SCHEME),
    ("government", kinds::GOVERNMENT),
    ("holding", kinds::HOLDING),
    ("modifier", kinds::MODIFIER),
    ("animation", kinds::PORTRAIT_ANIMATION),
    ("background", kinds::EVENT_BACKGROUND),
    ("theme", kinds::EVENT_THEME),
    ("template", kinds::SCRIPTED_CHARACTER_TEMPLATE),
    ("secret", kinds::SECRET_TYPE),
    ("interaction", kinds::CHARACTER_INTERACTION),
    ("activity", kinds::ACTIVITY_TYPE),
    ("intent", kinds::ACTIVITY_INTENT),
    ("task_contract", kinds::TASK_CONTRACT_TYPE),
    ("namespace", kinds::NAMESPACE),
    ("loc", kinds::LOC_KEY),
];

/// The kinds call-by-name references resolve to (scripted effects/triggers in
/// key position, script values in value position).
const CALL_KINDS: CallKinds = CallKinds {
    effect: kinds::SCRIPTED_EFFECT,
    trigger: kinds::SCRIPTED_TRIGGER,
    value: kinds::SCRIPT_VALUE,
};

/// The kinds interface-script (`.gui`) symbols get.
const GUI_KINDS: GuiKinds = GuiKinds {
    template: kinds::GUI_TEMPLATE,
    ty: kinds::GUI_TYPE,
    arg_refs: &[
        ("GetScriptedGui", kinds::SCRIPTED_GUI),
        ("ScriptValue", kinds::SCRIPT_VALUE),
        ("Custom", kinds::CUSTOM_LOC),
        ("Custom2", kinds::CUSTOM_LOC),
        // The ByKey/WithKey lookup family (kinds the schema models; the
        // unmodeled ones — GetTopParticipantGroupByKey, GetTraitTrackByKey,
        // GetReligionByKey — are omitted until their kinds exist).
        ("GetDecisionWithKey", kinds::DECISION),
        ("GetTitleByKey", kinds::TITLE),
        ("GetCultureByKey", kinds::CULTURE),
        ("GetFaithByKey", kinds::FAITH),
    ],
    // `raw_text`/`raw_tooltip` are deliberately absent: 95% of their
    // identifier-like corpus values are literal prose, not loc keys.
    loc_fields: &["text", "tooltip"],
};

/// The compiled datafunction registry (gui `[…]` expression typing), built
/// once from the generated `DumpDataTypes` table.
pub fn datafn_registry() -> &'static pdxl_gui::datafn::DataFnRegistry {
    static REG: std::sync::OnceLock<pdxl_gui::datafn::DataFnRegistry> = std::sync::OnceLock::new();
    REG.get_or_init(|| pdxl_gui::datafn::DataFnRegistry::from_rows(tables::DATA_FNS))
}

/// Builds the CK3 schema: the hand-written entity rows plus the table-derived
/// scope-link rules and skip words (see [`derived`]). Cheap to construct; build
/// once and share.
pub fn schema() -> Schema {
    let mut rows = entities::kinds();
    rows.extend(derived::derived_link_rules());
    schema_from_rows(&rows)
}

/// The hand-written rows only — the baseline the derivation proof harness
/// (`tests/derived_proof.rs`) measures against.
pub fn schema_hand_only() -> Schema {
    schema_from_rows(&entities::kinds())
}

/// Builds a schema from explicit rows.
fn schema_from_rows(rows: &[pdxl_analysis::KindSpec]) -> Schema {
    let mut schema = Schema::new(
        rows,
        SCOPE_KEYWORDS,
        TYPED_DEFS,
        KEYED_VALUE_DEFS,
        &[], // no nested keyed-value definitions in CK3
        DOC_REF_ALIASES,
        Some(CALL_KINDS),
        Some(GUI_KINDS),
        Some(kinds::GAME_CONCEPT),
    );
    schema.set_implicit_loc_patterns(&entities::implicit_loc_patterns());
    // Names the engine uses itself, so hover can explain a zero reference count
    // instead of leaving it to look like dead content.
    schema.set_intrinsics(&entities::intrinsics());
    // Hands extraction the modeled bodies, so a `FieldSpec` carrying a
    // `ref_kind` is itself a reference — no `RefRule` restating the same key.
    schema.set_contexts(contexts::context_schema());
    schema
}
