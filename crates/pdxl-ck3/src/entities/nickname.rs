//! Nicknames (`common/nicknames/`, from `_nicknames.info`) — top-level
//! `nick_* = { … }` definitions whose key doubles as the loc key (with a
//! `_desc` companion). Referenced by the `give_nickname` effect and the
//! `has_nickname` trigger — both unambiguous corpus-wide (0 unresolved);
//! `has_any_nickname` takes `yes`/`no`, so it is not a reference.

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::context::{Fallback, StructSpec};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern};

use super::Entity;
use super::common::{anywhere, toggle};

const NICKNAMES_DIR: &str = "common/nicknames/";

/// The body of one nickname definition.
static NICKNAME: StructSpec = StructSpec {
    name: "nickname",
    fields: &[
        (
            "is_prefix",
            toggle("Is the nickname a prefixed nickname? (default `no`)"),
        ),
        ("is_bad", toggle("Is the nickname bad? (default `no`)")),
    ],
    fallback: Fallback::Deny,
};

pub(crate) struct Nickname;

impl Entity for Nickname {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::NICKNAME,
        icon: IconHint::Tag,
        defs: Some(DefSource {
            dir_prefix: NICKNAMES_DIR,
            shape: DefShape::TopLevel,
        }),
        refs: &[
            anywhere(RefPattern::KeyValue("give_nickname")),
            anywhere(RefPattern::KeyValue("has_nickname")),
        ],
        aliases: &[],
    }];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(NICKNAMES_DIR, ClauseKind::Struct(&NICKNAME))];
}
