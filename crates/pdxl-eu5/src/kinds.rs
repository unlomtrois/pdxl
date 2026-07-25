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
pub const NAMED_COLOR: KindId = KindId::new("named_color");
pub const COUNTRY_DESCRIPTION_CATEGORY: KindId = KindId::new("country_description_category");
pub const START_COUNTRY: KindId = KindId::new("start_country");
pub const DYNAMIC_COUNTRY: KindId = KindId::new("dynamic_country");
pub const COAT_OF_ARMS: KindId = KindId::new("coat_of_arms");
pub const ESTATE: KindId = KindId::new("estate");
pub const ADVANCE: KindId = KindId::new("advance");
pub const AGE: KindId = KindId::new("age");
pub const BUILDING: KindId = KindId::new("building");
pub const UNIT: KindId = KindId::new("unit_type");
pub const LAW: KindId = KindId::new("law");
