//! Government reforms (`in_game/common/government_reforms/`, documented by
//! the directory readme) — 329 top-level defs, plus the government-type
//! kind they reference (`in_game/common/government_types/`: monarchy,
//! republic, theocracy, steppe_horde).
//!
//! References (corpus-validated, 0 unresolved): `government_reform:X`
//! literals are table-derived (`crate::derived`); `unlock_government_reform`
//! (22) lives here with its target; `government = X` (156, ungated) names a
//! government type; the reform's `age = X` rule lives with the AGE kind in
//! [`super::advance`].
//!
//! `societal_values = { X_focus … }` items are engine-derived from the
//! societal-value axis names (`mercantilism_vs_free_trade` →
//! `mercantilism_focus`) — no static definitions exist, so they stay
//! unmodeled until a name-derivation mechanism justifies itself.
//! `content_priority`/`icon` are corpus-real but absent from the readme.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Effect, Struct, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, FieldSpec, StructSpec, block, scalar};
use pdxl_analysis::{IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{OPAQUE, SCALED_MODIFIER};
use super::scripted::def_only;

pub(crate) const REFORMS_DIR: &str = "in_game/common/government_reforms/";

/// A yes/no toggle field.
const fn toggle(doc: &'static str) -> FieldSpec {
    scalar(Setting).doc(doc).values(&["yes", "no"])
}

/// The body of one government reform (readme + corpus).
static GOVERNMENT_REFORM: StructSpec = StructSpec {
    name: "government reform",
    fields: &[
        (
            "age",
            scalar(Setting).doc("Optional age the reform can be used from."),
        ),
        (
            "government",
            scalar(Setting).doc("Optional government type that supports the reform."),
        ),
        (
            "major",
            toggle("Exclusive: only one major reform per country."),
        ),
        ("unique", toggle("Has additional UI fluff.")),
        ("block_for_rebel", toggle("Cannot be used by rebels.")),
        (
            "locked",
            block(Trigger)
                .doc("Whether the reform is currently locked (cannot be interacted with)."),
        ),
        (
            "potential",
            block(Trigger).doc("Whether the reform is possible at all (root = country)."),
        ),
        (
            "allow",
            block(Trigger).doc("Whether the reform can start (root = country)."),
        ),
        (
            "male_regnal_names",
            block(Struct(&OPAQUE)).doc("Optional male regnal names assumed by rulers."),
        ),
        (
            "female_regnal_names",
            block(Struct(&OPAQUE)).doc("Optional female regnal names assumed by rulers."),
        ),
        (
            "years",
            scalar(Setting).doc("Implementation time (years part)."),
        ),
        (
            "months",
            scalar(Setting).doc("Implementation time (months part)."),
        ),
        (
            "weeks",
            scalar(Setting).doc("Implementation time (weeks part)."),
        ),
        (
            "days",
            scalar(Setting).doc("Implementation time (days part)."),
        ),
        (
            "on_activate",
            block(Effect).doc("Fired when the reform is chosen (root = country)."),
        ),
        (
            "on_fully_activated",
            block(Effect)
                .doc("Fired at 100% implementation (instant when there is no time delay)."),
        ),
        (
            "on_deactivate",
            block(Effect).doc("Fired when the reform is removed (root = country)."),
        ),
        (
            "country_modifier",
            block(Struct(&SCALED_MODIFIER))
                .doc("Scaled, triggered modifier applied to the whole country."),
        ),
        (
            "province_modifier",
            block(Struct(&SCALED_MODIFIER)).doc("Scaled, triggered modifier applied to provinces."),
        ),
        (
            "location_modifier",
            block(Struct(&SCALED_MODIFIER)).doc("Scaled, triggered modifier applied to locations."),
        ),
        (
            "societal_values",
            block(Struct(&OPAQUE)).doc(
                "Societal-value foci this reform pushes (engine-derived names — \
                 `<axis side>_focus`; not statically resolvable). *(corpus)*",
            ),
        ),
        (
            "content_priority",
            scalar(Setting).doc("UI ordering priority. *(corpus)*"),
        ),
        ("icon", scalar(Setting).doc("Icon override. *(corpus)*")),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct GovernmentReform;

impl Entity for GovernmentReform {
    const KINDS: &'static [KindSpec] = &[
        KindSpec {
            refs: &[RefRule {
                pattern: RefPattern::KeyValue("unlock_government_reform"),
                gate: None,
                alt: &[],
            }],
            ..def_only(kinds::GOVERNMENT_REFORM, IconHint::Action, REFORMS_DIR)
        },
        KindSpec {
            refs: &[RefRule {
                pattern: RefPattern::KeyValue("government"),
                gate: None,
                alt: &[],
            }],
            ..def_only(
                kinds::GOVERNMENT_TYPE,
                IconHint::Hierarchy,
                "in_game/common/government_types/",
            )
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(REFORMS_DIR, ClauseKind::Struct(&GOVERNMENT_REFORM))];
}
