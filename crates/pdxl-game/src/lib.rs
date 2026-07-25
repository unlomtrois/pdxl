//! Compile-time game selection: consumers (`pdxl-cli`, `pdxl-lsp`,
//! `pdxl-mcp`) reach the rules crate through this facade, and a cargo
//! feature decides which game a binary serves — one game per binary, no
//! runtime abstraction, no dead schema in the artifact.
//!
//! ```sh
//! cargo build -p pdxl-cli --features ck3   # the CK3 pdxl
//! cargo build -p pdxl-cli --features eu5   # the EU5 pdxl
//! ```
//!
//! Every rules crate exposes the same surface (`schema()`,
//! `datafn_registry()`, `kinds`, `contexts::context_schema()`,
//! `tables::{TRIGGERS, EFFECTS, MODIFIERS, SCOPE_LINKS, …}`), so the
//! re-export swaps cleanly.

#[cfg(all(feature = "ck3", feature = "eu5"))]
compile_error!("pdxl-game: enable exactly one game feature, not both (ck3 XOR eu5)");

#[cfg(not(any(feature = "ck3", feature = "eu5")))]
compile_error!("pdxl-game: enable a game feature: --features ck3 (or eu5)");

#[cfg(feature = "ck3")]
pub use pdxl_ck3::*;

#[cfg(all(feature = "eu5", not(feature = "ck3")))]
pub use pdxl_eu5::*;

/// The compiled-in game's short name (shown in logs/reports).
#[cfg(feature = "ck3")]
pub const GAME: &str = "ck3";
#[cfg(all(feature = "eu5", not(feature = "ck3")))]
pub const GAME: &str = "eu5";
