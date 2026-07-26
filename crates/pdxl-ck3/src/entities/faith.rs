//! Religions and faiths (`common/religion/religion_types/`, from
//! `_religion_types.info`). A file holds religion definitions; each faith is
//! a block child of the religion's `faiths = { }` container (the FAITH kind's
//! `ChildrenOf` def shape — religions themselves are not symbols today).
//!
//! Faith references: `faith:x` scope literals and the `religion =` / `faith =`
//! attributes of history characters and history provinces (both keys take a
//! *faith* name there; `religion` is the legacy spelling).
//!
//! The bodies' outbound references live with their target kinds (loc-rule
//! precedent): `doctrine`/`doctrine_selection_pair` in [`super::doctrine`],
//! `holy_site` in [`super::holy_site`], `family` in
//! [`super::religion_family`], `religious_head` in [`super::title`], the
//! `traits` virtue/sin lists in [`super::traits`].
//!
//! `reformed_icon` is corpus-real but undocumented in the `.info` (42 uses).

use crate::kinds;
use pdxl_analysis::context::ClauseKind::{self, Struct};
use pdxl_analysis::context::ScalarKind::{LocKey, Setting};
use pdxl_analysis::context::{Fallback, StructSpec, block, color, scalar};
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;
use super::common::{OPAQUE, anywhere};

pub(crate) const RELIGION_DIR: &str = "common/religion/religion_types/";

/// A `key = X` faith reference gated to one history directory.
const fn in_dir(dir: &'static str, key: &'static str) -> RefRule {
    RefRule {
        pattern: RefPattern::KeyValue(key),
        gate: Some(dir),
        alt: &[],
    }
}

/// `traits = { virtues = { … } sins = { … } }` — shared by religions and
/// doctrines. The lists hold trait names in three forms (`brave`,
/// `brave = 0.5`, `brave = { scale = 2 weight = 2 }`); the trait references
/// live in [`super::traits`].
pub(crate) static RELIGION_TRAITS: StructSpec = StructSpec {
    name: "religious traits",
    fields: &[
        (
            "virtues",
            block(Struct(&OPAQUE)).doc(
                "Traits that are virtues for all followers: `brave`, `brave = <scale>`, or \
                 `brave = { scale = … weight = … }`. Trait groups also work.",
            ),
        ),
        (
            "sins",
            block(Struct(&OPAQUE)).doc("Traits that are sins for all followers (same forms)."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// One `{ name = … coat_of_arms = … }` entry of `holy_order_names`.
static HOLY_ORDER_NAME: StructSpec = StructSpec {
    name: "holy order name",
    fields: &[
        ("name", scalar(LocKey).doc("The holy order's name key.")),
        (
            "coat_of_arms",
            scalar(Setting).doc("The holy order's coat of arms key."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// `holy_order_names = { { name = … } … }` — anonymous list entries.
static HOLY_ORDER_NAMES: StructSpec = StructSpec {
    name: "holy order names",
    fields: &[],
    fallback: Fallback::Struct(&HOLY_ORDER_NAME),
};

/// `doctrine_selection_pair = { … }` — a DLC-conditional doctrine.
pub(crate) static DOCTRINE_SELECTION_PAIR: StructSpec = StructSpec {
    name: "doctrine_selection_pair",
    fields: &[
        (
            "requires_dlc_flag",
            scalar(Setting).doc("The DLC flag that is evaluated."),
        ),
        (
            "doctrine",
            scalar(Setting).doc("Added when the DLC flag is present."),
        ),
        (
            "fallback_doctrine",
            scalar(Setting).doc("Optional: added instead when the flag is absent."),
        ),
    ],
    fallback: Fallback::Deny,
};

/// Fields shared verbatim by religions and faiths (precedence: faith >
/// religion > family).
macro_rules! inheritable_fields {
    () => {
        [
            (
                "graphical_faith",
                scalar(Setting).doc(
                    "The 3D-model set (currently temple assets). Precedence: faith > religion > \
                     family.",
                ),
            ),
            (
                "piety_icon_group",
                scalar(Setting).doc("The piety icon set. Precedence: faith > religion > family."),
            ),
            (
                "doctrine_background_icon",
                scalar(Setting)
                    .doc("The doctrine background icon. Precedence: faith > religion > family."),
            ),
            (
                "doctrine",
                scalar(Setting).doc("A doctrine (repeatable; `common/religion/doctrine_types/`)."),
            ),
            (
                "doctrine_selection_pair",
                block(Struct(&DOCTRINE_SELECTION_PAIR))
                    .doc("Add a doctrine only when a DLC flag is present."),
            ),
            (
                "reserved_male_names",
                block(Struct(&OPAQUE))
                    .doc("Names not chosen as random names by characters of other faiths."),
            ),
            (
                "reserved_female_names",
                block(Struct(&OPAQUE))
                    .doc("Names not chosen as random names by characters of other faiths."),
            ),
            (
                "localization",
                block(Struct(&OPAQUE)).doc(
                    "Key–value pairs for faith-dependent localization, accessed via \
                     `[Faith.Custom('key')]`. Faith keys inherit from the religion.",
                ),
            ),
            (
                "holy_order_names",
                block(Struct(&HOLY_ORDER_NAMES)).doc(
                    "Names and CoAs for holy orders (`{ name = … coat_of_arms = … }` \
                     entries). Faith entries take precedence over religion entries.",
                ),
            ),
        ]
    };
}

/// The body of one faith (`_religion_types.info`).
static FAITH_FIELDS: [(&str, pdxl_analysis::context::FieldSpec); 14] = {
    let base = inheritable_fields!();
    [
        (
            "color",
            color().doc(
                "The faith's color, used e.g. on the map — `{ r g b }`, `hsv { … }`, or a \
                 named color.",
            ),
        ),
        (
            "icon",
            scalar(Setting)
                .doc("The faith icon (`gfx/interface/icons/faith/%s.dds`), or another faith's."),
        ),
        (
            "reformed_icon",
            scalar(Setting)
                .doc("The icon used once the faith is reformed (corpus-real; not in the .info)."),
        ),
        (
            "religious_head",
            scalar(Setting).doc(
                "The title that is this faith's religious head; none if unset (unless created \
                 elsewhere in script).",
            ),
        ),
        (
            "holy_site",
            scalar(Setting).doc("A holy site (`common/religion/holy_site_types/`; repeatable)."),
        ),
        base[0],
        base[1],
        base[2],
        base[3],
        base[4],
        base[5],
        base[6],
        base[7],
        base[8],
    ]
};

static FAITH: StructSpec = StructSpec {
    name: "faith",
    fields: &FAITH_FIELDS,
    fallback: Fallback::Deny,
};

/// The `faiths = { … }` container: every block-valued child is a faith.
static FAITHS: StructSpec = StructSpec {
    name: "faiths",
    fields: &[],
    fallback: Fallback::Struct(&FAITH),
};

/// The body of one religion (`_religion_types.info`).
static RELIGION_FIELDS: [(&str, pdxl_analysis::context::FieldSpec); 15] = {
    let base = inheritable_fields!();
    [
        (
            "family",
            scalar(Setting).doc("The religion family (`common/religion/religion_family_types/`)."),
        ),
        (
            "pagan_roots",
            scalar(Setting)
                .doc(
                    "If yes, faiths without the unreformed doctrine are considered reformed \
                     by the interface.",
                )
                .values(&["yes", "no"]),
        ),
        (
            "traits",
            block(Struct(&RELIGION_TRAITS))
                .doc("Virtues and sins for all followers of the religion's faiths."),
        ),
        (
            "custom_faith_icons",
            block(Struct(&OPAQUE))
                .doc("The icons available when creating a custom faith based on this religion."),
        ),
        (
            "holy_order_maa",
            block(Struct(&OPAQUE)).doc(
                "Men-at-arms types for holy orders (the last type whose innovation the \
                 headquarters culture unlocked is used).",
            ),
        ),
        (
            "faiths",
            block(Struct(&FAITHS)).doc("The religion's faiths, one block per faith."),
        ),
        base[0],
        base[1],
        base[2],
        base[3],
        base[4],
        base[5],
        base[6],
        base[7],
        base[8],
    ]
};

static RELIGION: StructSpec = StructSpec {
    name: "religion",
    fields: &RELIGION_FIELDS,
    fallback: Fallback::Deny,
};

pub(crate) struct Faith;

impl Entity for Faith {
    const KINDS: &'static [KindSpec] = &[
        // Religions are the top-level blocks of the same files whose `faiths`
        // container holds the faiths below — two def rules over one directory.
        KindSpec {
            kind: kinds::RELIGION,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: RELIGION_DIR,
                shape: DefShape::TopLevel,
            }),
            // `religion_tag` is the only key naming a religion. Bare
            // `religion = X` means a *faith* and belongs to the rule below —
            // every value of it in history is a faith key.
            refs: &[anywhere(RefPattern::KeyValue("religion_tag"))],
            aliases: &[],
        },
        KindSpec {
            kind: kinds::FAITH,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: RELIGION_DIR,
                shape: DefShape::ChildrenOf {
                    containers: &["faiths"],
                },
            }),
            refs: &[
                anywhere(RefPattern::ScopePrefix("faith")),
                in_dir("history/characters/", "religion"),
                in_dir("history/characters/", "faith"),
                in_dir("history/provinces/", "religion"),
                in_dir("history/provinces/", "faith"),
            ],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] =
        &[(RELIGION_DIR, ClauseKind::Struct(&RELIGION))];
}
