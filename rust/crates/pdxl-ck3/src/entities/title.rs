//! Landed titles (`common/landed_titles/`) — a nested tree of tier-prefixed
//! definitions, referenced by `title:x` scope literals and (inside the tree)
//! by the `capital` attribute.

use crate::kinds;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::anywhere;

/// Where landed titles are defined — and the only place a bare
/// `capital = c_x` attribute unambiguously names a title.
const LANDED_TITLES_DIR: &str = "common/landed_titles/";

/// Landed-title tier prefixes, as observed in vanilla + real mods: empire,
/// kingdom, duchy, county, barony, and the hegemony tier (`h_china`, …).
/// A key in `common/landed_titles/` is a title definition iff it starts with
/// one of these AND has a block body (which excludes loc-key decoys like
/// `cultural_names = { x = k_something }`).
const TITLE_TIER_PREFIXES: &[&str] = &["e_", "k_", "d_", "c_", "b_", "h_"];

pub(crate) struct Title;

impl Entity for Title {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::TITLE,
        icon: IconHint::Hierarchy,
        defs: Some(DefSource {
            dir_prefix: LANDED_TITLES_DIR,
            shape: DefShape::Tree {
                key_prefixes: TITLE_TIER_PREFIXES,
            },
        }),
        refs: &[
            // Self-identifying scope literals: `title:e_hre`,
            // `title:k_x = { … }`, `title:e_byzantium.holder` — anywhere,
            // any position.
            anywhere(RefPattern::ScopePrefix("title")),
            // Inside title definitions, `capital = c_x` names the county a
            // title is administered from. Gated: elsewhere `capital` sets a
            // holding/realm capital by other means and is ambiguous.
            RefRule {
                pattern: RefPattern::KeyValue("capital"),
                gate: Some(LANDED_TITLES_DIR),
            },
        ],
        aliases: &[],
    }];
}
