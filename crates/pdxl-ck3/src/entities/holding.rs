//! Holding types (`common/holdings/`, from `_holdings.info`) — the settlement
//! kinds a barony can hold. Seven are scripted in vanilla and T4N alike:
//! `castle_holding`, `city_holding`, `church_holding`, `tribal_holding`,
//! `nomad_holding`, `herder_holding` and `temple_citadel_holding`.
//!
//! The readme and the corpus agree exactly here — every documented field is
//! used and no undocumented one appears — so nothing is marked *(corpus)*.
//!
//! References resolve from keys that cannot mean anything else, all
//! corpus-validated with zero unresolved: `holding` (province history, ~7000
//! sites), `has_holding_type`, `set_holding_type`, `begin_create_holding` and
//! `holding_type`. The three government keys — `primary_holding`,
//! `valid_holdings` and `required_county_holdings` — are gated to
//! `common/governments/`, their only home.
//!
//! The one value that is not a name is `holding = none`, the province-history
//! sentinel for "no holding here" (~4900 sites). `none` is never a definition
//! of any kind in the corpus, so it joins the game-wide skip list rather than
//! being special-cased here.
//!
//! Outgoing references live with their target kinds, per the loc.rs precedent:
//! `primary_building`/`buildings` in [`super::building`],
//! `required_heir_government_types` in [`super::government`].
//!
//! Each holding also auto-generates `<holding_type>_build_speed`,
//! `_build_gold_cost`, `_build_piety_cost`, `_build_prestige_cost` and the
//! `_holding_`-infixed variants; those live in the generated modifier tables,
//! not here.

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{OPAQUE, anywhere, toggle};

pub(crate) const HOLDINGS_DIR: &str = "common/holdings/";

/// A holding reference gated to the government directory — the only place
/// these three keys appear.
const fn in_governments(pattern: RefPattern) -> RefRule {
    RefRule {
        pattern,
        gate: Some(super::government::GOVERNMENTS_DIR),
        alt: &[],
    }
}

/// The body of one holding type (`_holdings.info`).
static HOLDING: StructSpec = StructSpec {
    name: "holding type",
    fields: &[
        (
            "primary_building",
            scalar(Setting).doc(
                "The building raised when a holding of this type is created \
                 (`common/buildings/`).",
            ),
        ),
        (
            "buildings",
            block(Struct(&OPAQUE)).doc(
                "First levels of every building constructible here, excluding the primary one \
                 (`common/buildings/`).",
            ),
        ),
        (
            "can_be_inherited",
            toggle("Whether a barony with this holding can be inherited. Default `yes`."),
        ),
        (
            "counts_toward_domain_limit_if_disabled",
            toggle(
                "Whether a barony with this holding counts toward the domain limit while \
                 the holding is disabled. Default `yes`.",
            ),
        ),
        (
            "required_heir_government_types",
            block(Struct(&OPAQUE)).doc(
                "Government types required to inherit a county whose capital province holds \
                 this type. When succession generates a character, the first entry is used \
                 (`common/governments/`). Default none.",
            ),
        ),
        (
            "parameters",
            block(Struct(&OPAQUE)).doc(
                "Arbitrary keys testable with `has_holding_parameter = X` — free-form strings, \
                 never definitions. Default none.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Holding;

impl Entity for Holding {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::HOLDING,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: HOLDINGS_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            // Province history (`holding = castle_holding`), triggers and
            // effects. `holding = none` is filtered by the game-wide skip list.
            anywhere(RefPattern::KeyValue("holding")),
            anywhere(RefPattern::KeyValue("has_holding_type")),
            anywhere(RefPattern::KeyValue("set_holding_type")),
            anywhere(RefPattern::KeyValue("holding_type")),
            // Also takes a block form, which the scalar rule skips for free.
            anywhere(RefPattern::KeyValue("begin_create_holding")),
            in_governments(RefPattern::KeyValue("primary_holding")),
            in_governments(RefPattern::KeyList("valid_holdings")),
            in_governments(RefPattern::KeyList("required_county_holdings")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(HOLDINGS_DIR, ClauseKind::Struct(&HOLDING))];
}
