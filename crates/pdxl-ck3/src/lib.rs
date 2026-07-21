//! CK3 rules for `pdxl-analysis` — the game's conventions as *data*.
//!
//! Originally a transcription of the Go `internal/validate/schema_ck3.go`
//! registry. Since the landed-titles addition (`ANALYSIS_VERSION` 2) this
//! schema has grown past the Go implementation — the analysis layer's Go
//! oracle is retired; regressions are pinned by golden snapshots in
//! `pdxl-parity` instead.
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
pub mod kinds;
pub mod tables;

/// Relative-scope keywords a reference value may hold at runtime
/// (`has_trait = prev`); unresolvable without scope tracking, so skipped.
/// Game-wide — they belong to no single concept.
const SCOPE_KEYWORDS: &[&str] = &[
    "root",
    "this",
    "prev",
    "prevprev",
    "prevprevprev",
    "prevprevprevprev",
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
    ("scheme", kinds::SCHEME),
    ("modifier", kinds::MODIFIER),
    ("animation", kinds::PORTRAIT_ANIMATION),
    ("background", kinds::EVENT_BACKGROUND),
    ("theme", kinds::EVENT_THEME),
    ("template", kinds::SCRIPTED_CHARACTER_TEMPLATE),
    ("secret", kinds::SECRET_TYPE),
    ("interaction", kinds::CHARACTER_INTERACTION),
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

/// Builds the CK3 schema from the entity registry. Cheap to construct; build
/// once and share.
pub fn schema() -> Schema {
    Schema::new(
        &entities::kinds(),
        SCOPE_KEYWORDS,
        TYPED_DEFS,
        KEYED_VALUE_DEFS,
        DOC_REF_ALIASES,
        Some(CALL_KINDS),
        Some(GUI_KINDS),
    )
}
