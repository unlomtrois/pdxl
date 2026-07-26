//! CK3 facts-extraction regression tests — golden snapshots.
//!
//! The harness (fixture discovery, dump format, diffing) lives in
//! `pdxl_testutil::facts_golden`; this file supplies only what is CK3-specific.
//! Fixtures come from `testdata/ck3/` — never another game's, since the
//! personas below are CK3 directory paths and the schema is the CK3 one.
//!
//! To accept an intentional behavior change, regenerate with:
//! `UPDATE_GOLDENS=1 cargo test -p pdxl-ck3 --test facts`
//! and review the golden diff like any other code change.

use pdxl_testutil::facts_golden::FactsGoldens;

/// Directory personas: one per CK3 def rule (incl. the nested landed-titles
/// rule), one gated (on_action), one that matches nothing.
const PERSONAS: &[&str] = &[
    "common/scripted_triggers/f.txt",
    "common/scripted_effects/f.txt",
    "common/traits/f.txt",
    "common/decisions/f.txt",
    "common/on_action/f.txt",
    "common/landed_titles/f.txt",
    "events/f.txt",
    "history/characters/f.txt",
    "gfx/f.txt",
];

#[test]
fn facts_match_goldens() {
    FactsGoldens {
        manifest_dir: env!("CARGO_MANIFEST_DIR"),
        game: "ck3",
        // Syntax stress fixtures are game-neutral script; the CK3 schema reads
        // them as harmlessly as any other, and they exercise odd shapes.
        extra_dirs: &["crates/pdxl-lexer/testdata", "crates/pdxl-ck3/testdata"],
        personas: PERSONAS,
        schema: &pdxl_ck3::schema(),
        crate_name: "pdxl-ck3",
    }
    .run();
}
