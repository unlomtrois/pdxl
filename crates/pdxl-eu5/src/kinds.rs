//! EU5's symbol-kind vocabulary (starter set — the universal script kinds
//! shared by every Jomini game; EU5-specific concepts grow from here the
//! same way CK3's did, one corpus-validated entity at a time).

use pdxl_analysis::KindId;

pub use pdxl_analysis::LOC_KEY;

pub const SCRIPTED_TRIGGER: KindId = KindId::new("scripted_trigger");
pub const SCRIPTED_EFFECT: KindId = KindId::new("scripted_effect");
pub const SCRIPT_VALUE: KindId = KindId::new("script_value");
pub const EVENT: KindId = KindId::new("event");
pub const NAMESPACE: KindId = KindId::new("namespace");
pub const COUNTRY: KindId = KindId::new("country");
pub const FORMABLE_COUNTRY: KindId = KindId::new("formable_country");
pub const CULTURE: KindId = KindId::new("culture");
pub const RELIGION: KindId = KindId::new("religion");
pub const RELIGION_GROUP: KindId = KindId::new("religion_group");
pub const RELIGIOUS_ASPECT: KindId = KindId::new("religious_aspect");
pub const RELIGIOUS_FACTION: KindId = KindId::new("religious_faction");
pub const RELIGIOUS_FIGURE: KindId = KindId::new("religious_figure");
pub const RELIGIOUS_FOCUS: KindId = KindId::new("religious_focus");
pub const RELIGIOUS_SCHOOL: KindId = KindId::new("religious_school");
pub const NAMED_COLOR: KindId = KindId::new("named_color");
pub const CUSTOM_LOC: KindId = KindId::new("custom_loc");
pub const DEFINE: KindId = KindId::new("define");
pub const COUNTRY_DESCRIPTION_CATEGORY: KindId = KindId::new("country_description_category");
pub const START_COUNTRY: KindId = KindId::new("start_country");
pub const DYNAMIC_COUNTRY: KindId = KindId::new("dynamic_country");
pub const COAT_OF_ARMS: KindId = KindId::new("coat_of_arms");
pub const ESTATE: KindId = KindId::new("estate");
pub const SUBJECT_TYPE: KindId = KindId::new("subject_type");
pub const SITUATION: KindId = KindId::new("situation");
pub const UNIT_ABILITY: KindId = KindId::new("unit_ability");
pub const CHARACTER_INTERACTION: KindId = KindId::new("character_interaction");
pub const COUNTRY_INTERACTION: KindId = KindId::new("country_interaction");
pub const RELATION_TYPE: KindId = KindId::new("relation_type");
pub const LEVY: KindId = KindId::new("levy");
pub const GOVERNMENT_REFORM: KindId = KindId::new("government_reform");
pub const CASUS_BELLI: KindId = KindId::new("casus_belli");
pub const PRODUCTION_METHOD: KindId = KindId::new("production_method");
pub const GOVERNMENT_TYPE: KindId = KindId::new("government_type");
pub const ADVANCE: KindId = KindId::new("advance");
pub const AGE: KindId = KindId::new("age");
pub const BIAS: KindId = KindId::new("bias");
pub const BUILDING: KindId = KindId::new("building");
pub const UNIT: KindId = KindId::new("unit_type");
pub const LAW: KindId = KindId::new("law");
pub const INTERNATIONAL_ORGANIZATION: KindId = KindId::new("international_organization");
pub const IO_VARIABLE: KindId = KindId::new("io_variable");
pub const IO_SPECIAL_STATUS: KindId = KindId::new("special_status");
pub const IO_PAYMENT: KindId = KindId::new("io_payment");
pub const IO_LAND_OWNERSHIP_RULE: KindId = KindId::new("land_ownership_rule");
pub const PARLIAMENT_TYPE: KindId = KindId::new("parliament_type");
pub const CULTURE_GROUP: KindId = KindId::new("culture_group");
pub const LANGUAGE: KindId = KindId::new("language");
pub const LANGUAGE_FAMILY: KindId = KindId::new("language_family");
pub const GAME_CONCEPT: KindId = KindId::new("game_concept");
pub const INSTITUTION: KindId = KindId::new("institution");
pub const LOCATION: KindId = KindId::new("location");
pub const PROVINCE: KindId = KindId::new("province");
pub const AREA: KindId = KindId::new("area");
pub const REGION: KindId = KindId::new("region");
pub const SUB_CONTINENT: KindId = KindId::new("sub_continent");
pub const CONTINENT: KindId = KindId::new("continent");
