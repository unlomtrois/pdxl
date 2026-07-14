//! Fact-extraction unit tests over the CK3 schema.
//!
//! Ports the extraction-level assertions from `internal/validate`'s
//! `validate_test.go` (definition collection, namespace skip, characters,
//! dotted IDs, macro params, unknown dirs) at the `FileFacts` level — the
//! SymbolTable half of those tests belongs to Milestone 6 — plus focused
//! coverage of every reference shape and skip rule.

use pdxl_analysis::{FileFacts, SymbolKind, extract_facts};

/// Extracts facts from `src` as if it lived at `rel_path`.
fn extract(src: &str, rel_path: &str) -> FileFacts {
    let parsed = pdxl_parser::parse(rel_path.to_string(), src.as_bytes().to_vec());
    extract_facts(parsed.tree(), rel_path, rel_path, &pdxl_ck3::schema())
}

fn def_names(f: &FileFacts) -> Vec<&str> {
    f.defs.iter().map(|s| s.name.as_str()).collect()
}

fn ref_names(f: &FileFacts) -> Vec<&str> {
    f.refs.iter().map(|r| r.name.as_str()).collect()
}

// ── definitions (ported from validate_test.go) ──────────────────────────────

#[test]
fn collects_definitions_and_skips_namespace() {
    let f = extract(
        "namespace = test\ntest.0001 = { type = character_event }\n",
        "events/test_events.txt",
    );
    // `namespace = test` has a scalar value, not a block — not a definition.
    assert_eq!(def_names(&f), vec!["test.0001"]);
    assert_eq!(f.defs[0].kind, SymbolKind::Event);
}

#[test]
fn collects_multiple_triggers() {
    let f = extract(
        "alpha_trigger = { always = yes }\nbeta_trigger = { always = no }\n",
        "common/scripted_triggers/00_t.txt",
    );
    assert_eq!(def_names(&f), vec!["alpha_trigger", "beta_trigger"]);
    assert!(f.defs.iter().all(|s| s.kind == SymbolKind::ScriptedTrigger));
}

#[test]
fn collects_characters_including_dotted_ids() {
    let f = extract(
        "145665 = { name = \"Foo\" }\nbohemia.1 = { name = \"Bar\" }\n",
        "history/characters/afar.txt",
    );
    assert_eq!(def_names(&f), vec!["145665", "bohemia.1"]);
    assert!(f.defs.iter().all(|s| s.kind == SymbolKind::Character));
}

#[test]
fn captures_macro_params_sorted_and_deduped() {
    let f = extract(
        "my_trigger = {\n\tx = $OPERATOR$\n\tcount >= $COUNT$\n\tagain = $OPERATOR$\n}\n",
        "common/scripted_triggers/00_t.txt",
    );
    assert_eq!(f.defs[0].params, vec!["COUNT", "OPERATOR"]);
}

#[test]
fn unknown_dirs_yield_no_defs() {
    let f = extract("some_block = { x = 1 }\n", "gfx/whatever.txt");
    assert!(f.defs.is_empty());
    assert!(f.aliases.is_empty());
}

#[test]
fn def_offsets_point_at_name() {
    let src = "# leading comment\nbrave = { }\n";
    let f = extract(src, "common/traits/00.txt");
    let d = &f.defs[0];
    assert_eq!(&src[d.offset as usize..d.end_offset as usize], "brave");
}

// ── aliases ──────────────────────────────────────────────────────────────────

#[test]
fn trait_groups_become_aliases() {
    let f = extract(
        "brave = { group = personality }\n\
         craven = { group = personality group_equivalence = fearful }\n",
        "common/traits/00.txt",
    );
    let names: Vec<&str> = f.aliases.iter().map(|a| a.name.as_str()).collect();
    // One alias per matching key per def — duplicates preserved (merge dedups).
    assert_eq!(names, vec!["personality", "personality", "fearful"]);
    assert!(f.aliases.iter().all(|a| a.kind == SymbolKind::Trait));
    // Go parity quirk: alias end_offset equals the def's start offset.
    assert_eq!(f.aliases[0].end_offset, f.aliases[0].offset);
}

#[test]
fn non_trait_kinds_get_no_aliases() {
    let f = extract(
        "my_effect = { group = whatever }\n",
        "common/scripted_effects/00.txt",
    );
    assert!(f.aliases.is_empty());
}

// ── references: shapes ───────────────────────────────────────────────────────

#[test]
fn scalar_refs_with_quote_stripping() {
    let f = extract(
        "e = { add_trait = brave remove_trait = \"craven\" has_trait = kind }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["brave", "craven", "kind"]);
    assert!(f.refs.iter().all(|r| r.kind == SymbolKind::Trait));
    // The byte range still covers the QUOTED source text.
    let r = &f.refs[1];
    assert_eq!(r.end - r.start, "\"craven\"".len() as u32);
}

#[test]
fn trigger_event_scalar_and_block_id() {
    let f = extract(
        "e = { trigger_event = ns.1 trigger_event = { id = ns.2 days = 5 } }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["ns.1", "ns.2"]);
    assert!(f.refs.iter().all(|r| r.kind == SymbolKind::Event));
}

#[test]
fn list_and_weighted_only_in_on_action() {
    let src = "events = { a.1 }\nrandom_events = { 50 = b.1 }\n";
    let gated = extract(src, "common/on_action/x.txt");
    assert_eq!(ref_names(&gated), vec!["a.1", "b.1"]);
    // The same keys outside on_action are ambiguous and yield nothing.
    let ungated = extract(src, "events/x.txt");
    assert!(ungated.refs.is_empty());
}

#[test]
fn weighted_skips_config_and_no_event() {
    let f = extract(
        "random_events = {\n\t100 = 0\n\t50 = ns.foo\n\tchance_to_happen = 25\n}\n",
        "common/on_action/x.txt",
    );
    // 100 = 0: numeric value means "no event"; chance_to_happen: word key.
    assert_eq!(ref_names(&f), vec!["ns.foo"]);
}

// ── references: skip rules ───────────────────────────────────────────────────

#[test]
fn skips_unresolvable_values() {
    let f = extract(
        "e = {\n\
         \thas_trait = prev\n\
         \thas_trait = root\n\
         \thas_trait = scope:target\n\
         \thas_trait = $TRAIT$\n\
         \tadd_trait = education_$EDUCATION$_5\n\
         \tadd_trait = real_one\n}\n",
        "events/x.txt",
    );
    // Scope keywords, chains (:), macro params ($), and macro-concatenation
    // prefixes are all unresolvable without deeper analysis.
    assert_eq!(ref_names(&f), vec!["real_one"]);
}

#[test]
fn loc_is_file_line_col() {
    let f = extract("x = 1\ne = { add_trait = brave }\n", "events/dir/x.txt");
    assert_eq!(f.refs[0].loc, "events/dir/x.txt:2:19");
}

// ── malformed input ──────────────────────────────────────────────────────────

#[test]
fn partial_trees_still_extract() {
    // Unclosed block: the parser recovers; extraction sees the partial tree.
    let f = extract(
        "brave = { group = personality\n", // missing '}'
        "common/traits/00.txt",
    );
    assert_eq!(def_names(&f), vec!["brave"]);
    assert_eq!(f.aliases[0].name, "personality");
}

// ── landed titles (ANALYSIS_VERSION 2; first post-parity schema) ─────────────

const TITLE_TREE: &str = "@var = 1\n\
e_empire = {\n\
\tcolor = { 1 2 3 }\n\
\tcapital = c_shore\n\
\tcultural_names = { name_list_x = k_decoy }\n\
\tai_primary_priority = { if = { limit = { always = yes } } }\n\
\tk_kingdom = {\n\
\t\td_duchy = {\n\
\t\t\tc_shore = { b_port = { province = 1 } }\n\
\t\t}\n\
\t}\n\
}\n\
k_titular = { color = { 4 5 6 } }\n\
h_hegemony = { }\n";

#[test]
fn title_tree_harvests_all_tiers_recursively() {
    let f = extract(TITLE_TREE, "common/landed_titles/00.txt");
    assert_eq!(
        def_names(&f),
        vec![
            "e_empire",
            "k_kingdom",
            "d_duchy",
            "c_shore",
            "b_port",
            "k_titular",
            "h_hegemony"
        ],
        "definition order = tree pre-order"
    );
    assert!(f.defs.iter().all(|d| d.kind == SymbolKind::Title));
    assert!(f.defs.iter().all(|d| d.params.is_empty()));
}

#[test]
fn title_tree_skips_attribute_keys_and_loc_decoys() {
    let f = extract(TITLE_TREE, "common/landed_titles/00.txt");
    let names = def_names(&f);
    for decoy in [
        "color",
        "capital",
        "cultural_names",
        "ai_primary_priority",
        "k_decoy",
        "@var",
    ] {
        assert!(!names.contains(&decoy), "{decoy} must not be a definition");
    }
    // `capital = c_shore` (scalar value) is not a def; the real c_shore block is.
    assert_eq!(names.iter().filter(|n| **n == "c_shore").count(), 1);
}

#[test]
fn title_defs_only_in_landed_titles_dir() {
    let f = extract(TITLE_TREE, "common/scripted_effects/x.txt");
    // Outside landed_titles the tier keys are ordinary top-level defs of the
    // dir's own kind (scripted_effect), not titles.
    assert!(f.defs.iter().all(|d| d.kind == SymbolKind::ScriptedEffect));
}

#[test]
fn title_scope_refs_in_all_positions() {
    let f = extract(
        "e = {\n\
         \thas_title = title:e_empire\n\
         \tis_at_war_with = title:e_empire.holder\n\
         \ttitle:k_titular = { set_flag = x }\n\
         \tOR = { title:h_hegemony.holder title:c_shore }\n\
         }\n",
        "common/scripted_effects/e.txt",
    );
    let titles: Vec<(&str, &str)> = f
        .refs
        .iter()
        .filter(|r| r.kind == SymbolKind::Title)
        .map(|r| (r.name.as_str(), r.loc.rsplit(':').nth(1).unwrap()))
        .collect();
    let names: Vec<&str> = titles.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        names,
        vec!["e_empire", "e_empire", "k_titular", "h_hegemony", "c_shore"],
        "value, chained value, key, and loose list items must all extract"
    );
}

#[test]
fn title_ref_range_covers_only_the_name() {
    let src = "x = title:e_empire.holder\n";
    //         0123456789...
    let f = extract(src, "common/scripted_effects/e.txt");
    let r = f.refs.iter().find(|r| r.kind == SymbolKind::Title).unwrap();
    assert_eq!(&src[r.start as usize..r.end as usize], "e_empire");
    assert!(
        r.loc.ends_with(":1:11"),
        "loc points at the name: {}",
        r.loc
    );
}

#[test]
fn title_refs_skip_macros_and_lookalikes() {
    let f = extract(
        "e = {\n\
         \thas_title = title:$TITLE$\n\
         \tx = subtitle:e_fake\n\
         \ty = title_something\n\
         }\n",
        "common/scripted_effects/e.txt",
    );
    assert!(
        f.refs.iter().all(|r| r.kind != SymbolKind::Title),
        "macros and non-title: prefixes must not extract: {:?}",
        f.refs
    );
}

#[test]
fn title_refs_resolve_against_tree_defs() {
    // End-to-end through merge_and_resolve: tree def + title: ref → resolved;
    // missing one → diagnostic naming the title kind.
    use pdxl_analysis::merge_and_resolve;
    use std::collections::HashMap;

    let defs = extract(TITLE_TREE, "common/landed_titles/00.txt");
    let refs = extract(
        "e = { has_title = title:d_duchy has_title = title:d_gone }\n",
        "common/scripted_effects/e.txt",
    );
    let mut facts = HashMap::new();
    facts.insert("common/landed_titles/00.txt".to_string(), defs);
    facts.insert("common/scripted_effects/e.txt".to_string(), refs);
    let order = [
        "common/landed_titles/00.txt",
        "common/scripted_effects/e.txt",
    ];
    let (table, diags) = merge_and_resolve(&order, &facts);

    assert_eq!(table.count(SymbolKind::Title), 7);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert!(
        diags[0].msg.contains("unknown title \"d_gone\""),
        "{}",
        diags[0].msg
    );
}
