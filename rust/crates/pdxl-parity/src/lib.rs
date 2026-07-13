//! The Go-oracle differential harness for the pdxl Rust port.
//!
//! During the port, the Go implementation is the executable specification. This
//! crate concentrates everything that exists purely to *prove parity* and would
//! otherwise clutter the production crates:
//!
//! - the canonical **dump formats** ([`dump_tokens`], [`dump_json`],
//!   [`dump_scan`], [`dump_descriptor`]) whose byte layout matches the additive
//!   Go tools under `tools/` exactly;
//! - the **dump binaries** (`lexdump`, `parsedump`, `filesetdump`), each with a
//!   same-format Go twin;
//! - the **differential tests** (`tests/`), which run both sides over shared
//!   fixtures and assert byte-identical output.
//!
//! Production crates do not depend on this crate; it depends on all of them.
//! Once the port is complete and the Go oracle retires, this crate can shrink or
//! disappear without touching anything else.

mod facts_dump;
mod fileset_dump;
mod token_dump;
mod tree_dump;

pub use facts_dump::{FACTS_DUMP_VERSION, dump_facts};
pub use fileset_dump::{FILESET_DUMP_VERSION, dump_descriptor, dump_scan};
pub use token_dump::dump_tokens;
pub use tree_dump::{DUMP_VERSION, dump_json};
