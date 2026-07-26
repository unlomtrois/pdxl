//! EU5 facts-extraction regression tests — golden snapshots.
//!
//! The mirror of `pdxl-ck3/tests/facts.rs`: same shared harness
//! (`pdxl_testutil::facts_golden`), same dump format, EU5 fixtures and schema.
//! These fixtures used to be read by the CK3 suite under CK3 personas, which
//! produced facts that meant nothing; they are pinned properly here instead.
//!
//! To accept an intentional behavior change, regenerate with:
//! `UPDATE_GOLDENS=1 cargo test -p pdxl-eu5 --test facts`
//! and review the golden diff like any other code change.

use pdxl_testutil::facts_golden::FactsGoldens;

/// Directory personas: one per def rule the fixtures exercise, plus one that
/// matches nothing. EU5 module roots are `in_game/` and `main_menu/`.
const PERSONAS: &[&str] = &[
    "in_game/common/advances/f.txt",
    "in_game/common/government_reforms/f.txt",
    "in_game/common/international_organizations/f.txt",
    "in_game/common/international_organization_special_statuses/f.txt",
    "in_game/common/parliament_types/f.txt",
    "in_game/common/subject_types/f.txt",
    "in_game/common/situations/f.txt",
    "in_game/events/f.txt",
    "main_menu/common/named_colors/f.txt",
    "gfx/f.txt",
];

#[test]
fn facts_match_goldens() {
    FactsGoldens {
        manifest_dir: env!("CARGO_MANIFEST_DIR"),
        game: "eu5",
        extra_dirs: &[],
        personas: PERSONAS,
        schema: &pdxl_eu5::schema(),
        crate_name: "pdxl-eu5",
    }
    .run();
}
