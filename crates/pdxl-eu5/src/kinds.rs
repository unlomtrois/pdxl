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
