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

use pdxl_analysis::KindId;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::{ImplicitLocPattern, KindSpec};

pub(crate) mod common;

mod activity;
pub(crate) mod artifact;
mod building;
mod casus_belli;
mod character;
pub(crate) mod character_interaction;
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
mod doctrine;
mod domicile;
mod dynasty;
mod effect_localization;
pub(crate) mod event;
mod event_background;
mod event_theme;
mod faith;
mod game_concept;
mod game_rule;
pub(crate) mod government;
mod great_project;
mod gui;
pub(crate) mod holding;
mod holy_site;
mod law;
mod loc;
mod message;
mod modifier;
mod name_list;
mod named_color;
mod namespace;
mod nickname;
mod on_action;
mod portrait_animation;
mod province;
mod religion_family;
mod scheme;
mod scripted;
mod scripted_gui;
mod secret_type;
mod situation;
mod subject_contract;
mod task_contract;
mod terrain;
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
    /// Localization keys this concept's entities claim by name alone, with no
    /// reference token in script — `<key>`, `<key>_desc` and the like. They
    /// power the hover links from an entity to its text, and the reverse
    /// edge that makes a loc key's references include the entities using it.
    const IMPLICIT_LOC: &'static [ImplicitLocPattern] = &[];
    /// Names the *engine* uses directly, so script never references them.
    ///
    /// A definition here is live content whose call site is compiled into the
    /// game — `msg_siege_won` is raised by the siege code, not by any
    /// `send_interface_toast`. Without this, such a symbol reports zero
    /// references and reads as dead, which is exactly the wrong conclusion.
    ///
    /// Verified by `strings` over the game binary; see the entity's module doc
    /// for the extraction used, so a list can be rebuilt after a patch.
    const INTRINSICS: &'static [(KindId, &'static [&'static str])] = &[];
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
        /// Every concept's implicit-localization conventions.
        pub(crate) fn implicit_loc_patterns() -> Vec<ImplicitLocPattern> {
            let mut v = Vec::new();
            $( v.extend_from_slice(<$e as Entity>::IMPLICIT_LOC); )+
            v
        }
        /// Every concept's engine-owned names.
        pub(crate) fn intrinsics() -> Vec<(KindId, &'static [&'static str])> {
            let mut v = Vec::new();
            $( v.extend_from_slice(<$e as Entity>::INTRINSICS); )+
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
    province::Province,
    terrain::Terrain,
    named_color::NamedColor,
    doctrine::Doctrine,
    holy_site::HolySite,
    religion_family::ReligionFamily,
    game_rule::GameRule,
    government::Government,
    holding::Holding,
    subject_contract::SubjectContract,
    domicile::Domicile,
    great_project::GreatProject,
    activity::Activity,
    task_contract::TaskContract,
    message::Message,
    loc::Loc,
);
