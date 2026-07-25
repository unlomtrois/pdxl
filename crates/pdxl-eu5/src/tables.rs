//! EU5 script-documentation tables, generated from the game's own dumps
//! (same pipeline as CK3; row types shared via `pdxl-gamedocs`).
//!
//! Regenerate after a game patch and review the diff like a golden file:
//!
//! ```sh
//! cargo run -p pdxl-gamedocs --bin gen-tables -- \
//!   --logs "<EU5 user dir>/docs" \
//!   --data-types "<EU5 user dir>/logs/data_types" \
//!   --out crates/pdxl-eu5/src/tables
//! ```
//!
//! EU5's dumps are the Markdown dialect (auto-detected); the game writes no
//! `event_scopes.log`, so `SCOPE_TYPES` is empty.

pub use pdxl_gamedocs::rows::{DocRow, LinkRow, ModifierRow, ScopeTypeRow};
pub use pdxl_gui::datafn::{DataFnKind, DataFnRow};

pub mod data_types;
pub mod effects;
pub mod modifiers;
pub mod scope_links;
pub mod scope_types;
pub mod triggers;

pub use data_types::DATA_FNS;
pub use effects::EFFECTS;
pub use modifiers::MODIFIERS;
pub use scope_links::{CODE_SAVED_SCOPES, SCOPE_LINKS};
pub use scope_types::SCOPE_TYPES;
pub use triggers::TRIGGERS;
