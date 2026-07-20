//! CK3's symbol-kind vocabulary. One `const` per concept — the game owns its
//! kinds, so the engine (`pdxl-analysis`) never names them. Localization keys
//! are the engine's one well-known kind, re-exported here for convenience.

use pdxl_analysis::KindId;

pub use pdxl_analysis::LOC_KEY;

pub const SCRIPTED_TRIGGER: KindId = KindId::new("scripted_trigger");
pub const SCRIPTED_EFFECT: KindId = KindId::new("scripted_effect");
pub const TRAIT: KindId = KindId::new("trait");
pub const EVENT: KindId = KindId::new("event");
pub const DECISION: KindId = KindId::new("decision");
pub const ON_ACTION: KindId = KindId::new("on_action");
pub const CHARACTER: KindId = KindId::new("character");
pub const TITLE: KindId = KindId::new("title");
pub const CULTURE: KindId = KindId::new("culture");
pub const FAITH: KindId = KindId::new("faith");
pub const LAW: KindId = KindId::new("law");
pub const SCHEME: KindId = KindId::new("scheme");
pub const EVENT_BACKGROUND: KindId = KindId::new("event_background");
pub const EVENT_THEME: KindId = KindId::new("event_theme");
pub const MODIFIER: KindId = KindId::new("modifier");
pub const SCRIPT_VALUE: KindId = KindId::new("script_value");
pub const PORTRAIT_ANIMATION: KindId = KindId::new("portrait_animation");
pub const SCRIPTED_CHARACTER_TEMPLATE: KindId = KindId::new("scripted_character_template");
pub const NAMESPACE: KindId = KindId::new("namespace");
pub const SECRET_TYPE: KindId = KindId::new("secret_type");
pub const CHARACTER_INTERACTION: KindId = KindId::new("character_interaction");
pub const INTERACTION_CATEGORY: KindId = KindId::new("interaction_category");
