//! Schema-coverage survey over a synthetic game tree.

use pdxl_project::coverage::{Coverage, survey};
use pdxl_testutil::TempTree;

#[test]
fn survey_classifies_covered_and_uncovered_dirs() {
    let t = TempTree::new();
    // Covered: the schema harvests common/traits/.
    t.write("common/traits/00.txt", "brave = { }\ncraven = { }\n");
    // Uncovered, documented: an .info marks it as a modelling target.
    t.write(
        "common/opinion_modifiers/00.txt",
        "a = { opinion = 5 }\nb = { opinion = 1 }\n",
    );
    t.write("common/opinion_modifiers/_opinions.info", "docs");
    // Uncovered, no defs (loose values only) — still reported, low score.
    t.write("common/whatever/00.txt", "# just a comment\n");

    let schema = pdxl_ck3::schema();
    let context_roots: Vec<&str> = pdxl_ck3::contexts::context_schema()
        .roots
        .iter()
        .map(|(prefix, _)| *prefix)
        .collect();
    let reports = survey(
        &t.path,
        &schema,
        pdxl_ck3::coverage::SURVEY_ROOTS,
        &context_roots,
    )
    .unwrap();
    let by_dir = |d: &str| {
        reports
            .iter()
            .find(|r| r.rel_dir == d)
            .unwrap_or_else(|| panic!("missing {d}: {reports:?}"))
    };

    let traits = by_dir("common/traits/");
    assert_eq!(traits.coverage, Coverage::Defs);
    assert_eq!(traits.defs, 2);

    let opinions = by_dir("common/opinion_modifiers/");
    assert_eq!(opinions.coverage, Coverage::None);
    assert_eq!(opinions.defs, 2);
    assert_eq!(opinions.info_files, vec!["_opinions.info"]);
    // The .info boost makes the documented dir outrank an equal undocumented one.
    assert!(opinions.score() > traits.defs as u64);

    let empty = by_dir("common/whatever/");
    assert_eq!(empty.defs, 0);
}
