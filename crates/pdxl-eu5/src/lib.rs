//! The EU5 rules crate: game knowledge as data over the `pdxl-analysis`
//! engine. Starter schema — EU5 ships the same Jomini script layer as CK3
//! (scripted effects/triggers, script values, namespaced events), but under
//! module roots (`in_game/`, `main_menu/`) instead of a flat game dir, so
//! every directory prefix here carries the `in_game/` module.
//!
//! Public surface mirrors `pdxl-ck3` — consumers reach whichever game was
//! compiled in through the `pdxl-game` facade, so both crates must expose
//! the same modules and functions.
//!
//! Known noise (starter): EU5's `gfx/map/city_data/` files use an asset
//! convention where objects declare `name = "X"` and are referenced as
//! `@X` — the engine reads those as (unresolved) script constants, ~242
//! diagnostics in vanilla. Modeling that name/`@` relationship as its own
//! kind is a planned refinement, not a rule bug.

pub mod contexts;
pub mod kinds;
pub mod tables;

use pdxl_analysis::{CallKinds, DefShape, DefSource, IconHint, KindId, KindSpec, Schema};

/// Relative-scope keywords (`prev`, `this`, …) skipped during resolution.
const SCOPE_KEYWORDS: &[&str] = &["root", "this", "prev", "from", "fromfrom"];

/// `namespace = X` declares an event namespace (same convention as CK3).
const KEYED_VALUE_DEFS: &[(&str, KindId)] = &[("namespace", kinds::NAMESPACE)];

const CALL_KINDS: CallKinds = CallKinds {
    effect: kinds::SCRIPTED_EFFECT,
    trigger: kinds::SCRIPTED_TRIGGER,
    value: kinds::SCRIPT_VALUE,
};

/// One definition-only kind row (references grow with corpus validation).
const fn def_only(kind: KindId, icon: IconHint, dir: &'static str) -> KindSpec {
    KindSpec {
        kind,
        icon,
        defs: Some(DefSource {
            dir_prefix: dir,
            shape: DefShape::TopLevel,
        }),
        refs: &[],
        aliases: &[],
    }
}

/// The EU5 kind rows (starter: universal script kinds, defs + call-by-name).
const KIND_ROWS: &[KindSpec] = &[
    def_only(
        kinds::SCRIPTED_TRIGGER,
        IconHint::Function,
        "in_game/common/scripted_triggers/",
    ),
    def_only(
        kinds::SCRIPTED_EFFECT,
        IconHint::Function,
        "in_game/common/scripted_effects/",
    ),
    KindSpec {
        kind: kinds::SCRIPT_VALUE,
        icon: IconHint::Function,
        defs: Some(DefSource {
            dir_prefix: "in_game/common/script_values/",
            shape: DefShape::TopLevelValued,
        }),
        refs: &[],
        aliases: &[],
    },
    def_only(kinds::EVENT, IconHint::Event, "in_game/events/"),
    KindSpec {
        kind: kinds::NAMESPACE,
        icon: IconHint::Tag,
        defs: None,
        refs: &[],
        aliases: &[],
    },
];

/// The compiled datafunction registry — empty until EU5's `DumpDataTypes`
/// output is generated into `tables`.
pub fn datafn_registry() -> &'static pdxl_gui::datafn::DataFnRegistry {
    static REG: std::sync::OnceLock<pdxl_gui::datafn::DataFnRegistry> = std::sync::OnceLock::new();
    REG.get_or_init(|| pdxl_gui::datafn::DataFnRegistry::from_rows(tables::DATA_FNS))
}

/// Builds the EU5 schema. Cheap to construct; build once and share.
pub fn schema() -> Schema {
    Schema::new(
        KIND_ROWS,
        SCOPE_KEYWORDS,
        &[], // no inline typed-def keywords observed yet
        KEYED_VALUE_DEFS,
        &[], // no doc-ref aliases yet
        Some(CALL_KINDS),
        None, // .gui analysis off until the datafn registry exists
        None, // no loc-concept convention verified yet
    )
}
