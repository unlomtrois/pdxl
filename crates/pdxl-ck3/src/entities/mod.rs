//! CK3 game concepts, one module per domain.
//!
//! Each entity co-locates **everything** the analyzer knows about a concept:
//! its schema rows ([`KindSpec`] — def directory, reference shapes, aliases,
//! icon) and its structural context ([`ContextSchema`] roots + the
//! [`StructSpec`] tree its bodies follow). This is the schema-scaling
//! principle (`rust/docs/SCHEMA-SCALING.md`) taken to the file level: adding
//! a game concept is adding one file and one line to the registry below.
//!
//! The [`Entity`] trait is a *shape contract*, not polymorphism — it forces
//! every domain to declare the same two surfaces so none can be added and
//! left unwired. Shared structural fragments (cost, duration, …) live in
//! [`common`]; the `_*.info`-derived specs live with their owning concept.
//!
//! [`ContextSchema`]: pdxl_analysis::context::ContextSchema
//! [`StructSpec`]: pdxl_analysis::context::StructSpec

use pdxl_analysis::KindSpec;
use pdxl_analysis::context::ClauseKind;

pub(crate) mod common;

pub(crate) mod artifact;
mod building;
mod casus_belli;
mod character;
mod character_interaction;
mod character_template;
pub(crate) mod create_character;
mod culture;
mod culture_era;
mod culture_innovation;
mod culture_misc;
mod culture_pillar;
mod culture_shared;
mod culture_tradition;
mod custom_loc;
mod death_reason;
mod decision;
mod dynasty;
mod effect_localization;
mod event;
mod event_background;
mod event_theme;
mod faith;
mod game_concept;
mod gui;
mod law;
mod loc;
mod modifier;
mod name_list;
mod namespace;
mod nickname;
mod on_action;
mod portrait_animation;
mod scheme;
mod scripted;
mod scripted_gui;
mod secret_type;
mod situation;
mod title;
mod traits;

/// The uniform surface every game concept declares. Both consts default to
/// empty, so a concept contributes only the facets it has (a pure structural
/// helper has no `KINDS`; a symbol with no body has no `ROOTS`).
pub(crate) trait Entity {
    /// Schema rows this concept contributes (usually one; `scripted` has two).
    const KINDS: &'static [KindSpec] = &[];
    /// Directory prefix → root structural context for this concept's bodies.
    const ROOTS: &'static [(&'static str, ClauseKind)] = &[];
}

/// Assembles the registered entities into the flat rows the engine consumes.
/// Registration order is preserved (it fixes `KindSpec` order and — for
/// mutually-exclusive prefixes — is otherwise behaviorally irrelevant).
macro_rules! registry {
    ($($e:ty),+ $(,)?) => {
        /// Every concept's schema rows, in registration order.
        pub(crate) fn kinds() -> Vec<KindSpec> {
            let mut v = Vec::new();
            $( v.extend_from_slice(<$e as Entity>::KINDS); )+
            v
        }
        /// Every concept's structural-context roots.
        pub(crate) fn roots() -> Vec<(&'static str, ClauseKind)> {
            let mut v = Vec::new();
            $( v.extend_from_slice(<$e as Entity>::ROOTS); )+
            v
        }
    };
}

registry!(
    scripted::Scripted,
    traits::Traits,
    decision::Decision,
    on_action::OnAction,
    event::Event,
    character::Character,
    title::Title,
    culture::Culture,
    faith::Faith,
    law::Law,
    scheme::Scheme,
    event_background::EventBackground,
    event_theme::EventTheme,
    modifier::Modifier,
    portrait_animation::PortraitAnimation,
    character_template::CharacterTemplate,
    character_interaction::CharacterInteraction,
    namespace::Namespace,
    secret_type::SecretType,
    artifact::Artifact,
    casus_belli::CasusBelli,
    dynasty::Dynasty,
    nickname::Nickname,
    death_reason::DeathReason,
    custom_loc::CustomLoc,
    building::Building,
    effect_localization::EffectLocalization,
    scripted_gui::ScriptedGui,
    gui::Gui,
    culture_pillar::CulturePillar,
    culture_tradition::CultureTradition,
    culture_era::CultureEra,
    culture_innovation::CultureInnovation,
    name_list::NameList,
    culture_misc::CultureMisc,
    game_concept::GameConcept,
    situation::Situation,
    loc::Loc,
);
