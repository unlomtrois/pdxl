//! Secret types (`common/secret_types/`) — top-level `NAME = { … }` definitions
//! (from `_secret_types.info`). Referenced by `type = X` inside the secret
//! effects and iterators (`add_secret`, `any_secret`, `random_secret`, …), so
//! the rule is gated to those keys — a bare `type =` is far too common.

use pdxl_analysis::context::ClauseKind::{self, Effect, Trigger};
use pdxl_analysis::context::ScalarKind::Setting;
use pdxl_analysis::context::{Fallback, StructSpec, block, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule, SymbolKind};

use super::Entity;
use super::common::anywhere;

/// `type = X` inside `add_key = { … }` names a secret type.
const fn secret_ref(add_key: &'static str) -> RefRule {
    anywhere(RefPattern::KeyBlockField(add_key, "type"))
}

/// The body of one `NAME = { … }` secret-type definition.
static SECRET_TYPE: StructSpec = StructSpec {
    name: "secret_type",
    fields: &[
        (
            "category",
            scalar(Setting).doc(
                "Optional category string. Drives the icon \
                 `gfx/interface/icons/secret_categories/<category>.dds` and the name \
                 `secret_category_<category>`.",
            ),
        ),
        (
            "is_valid",
            block(Trigger).doc(
                "The secret persists while this passes. Scopes: `scope:secret_owner` (whom the \
                 secret is about), `scope:secret_target` (a related character, e.g. the lover).",
            ),
        ),
        (
            "is_shunned",
            block(Trigger).doc(
                "Whether another character views the secret as shunned. `is_valid` scopes plus \
                 `scope:target` (the viewer).",
            ),
        ),
        (
            "is_criminal",
            block(Trigger).doc(
                "Whether another character views the secret as criminal. `is_valid` scopes plus \
                 `scope:target`.",
            ),
        ),
        (
            "on_discover",
            block(Effect).doc(
                "Effect when another character discovers the secret. `is_valid` scopes plus \
                 `root` = the secret and `scope:discoverer`.",
            ),
        ),
        (
            "on_expose",
            block(Effect).doc(
                "Effect when a character exposes the secret. `is_valid` scopes plus `root` = the \
                 secret and `scope:secret_exposer`.",
            ),
        ),
        (
            "on_owner_death",
            block(Effect).doc(
                "Effect when the owner dies (the owner is already the next participant). If no \
                 alive owner remains after this, the secret is removed.",
            ),
        ),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct SecretType;

impl Entity for SecretType {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: SymbolKind::SecretType,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/secret_types/",
            shape: DefShape::TopLevel,
        }),
        refs: &[
            secret_ref("add_secret"),
            secret_ref("any_secret"),
            secret_ref("every_secret"),
            secret_ref("random_secret"),
            secret_ref("ordered_secret"),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[("common/secret_types/", ClauseKind::Struct(&SECRET_TYPE))];
}
