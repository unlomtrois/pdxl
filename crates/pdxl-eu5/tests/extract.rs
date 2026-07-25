//! EU5 schema extraction: the country domain (facts-level, corpus-shaped).

use pdxl_analysis::{FileFacts, extract_facts};

fn extract(src: &str, rel_path: &str) -> FileFacts {
    let schema = pdxl_eu5::schema();
    let (tree, _) = pdxl_parser::parse("test".to_string(), src.as_bytes().to_vec()).into_parts();
    extract_facts(&tree, rel_path, "test", &schema, None)
}

#[test]
fn country_defs_and_body_refs() {
    let f = extract(
        "AHI = {\n\
         \tcolor = map_ahiler\n\
         \tcolor2 = rgb { 241 198 188 }\n\
         \tculture_definition = turkish_culture\n\
         \treligion_definition = shia\n\
         \tdescription_category = administrative\n\
         }\n",
        "in_game/setup/countries/anatolia.txt",
    );
    assert_eq!(f.defs.len(), 1);
    assert_eq!(f.defs[0].name, "AHI");
    assert_eq!(f.defs[0].kind, pdxl_eu5::kinds::COUNTRY);
    let kind_of = |n: &str| f.refs.iter().find(|r| r.name == n).expect(n).kind;
    assert_eq!(kind_of("map_ahiler"), pdxl_eu5::kinds::NAMED_COLOR);
    assert_eq!(kind_of("turkish_culture"), pdxl_eu5::kinds::CULTURE);
    assert_eq!(kind_of("shia"), pdxl_eu5::kinds::RELIGION);
    assert_eq!(
        kind_of("administrative"),
        pdxl_eu5::kinds::COUNTRY_DESCRIPTION_CATEGORY
    );
    // The rgb literal is not a named-color ref.
    assert!(f.refs.iter().all(|r| r.name != "rgb"));
}

#[test]
fn formable_tag_alias_resolves_c_literal() {
    use pdxl_analysis::merge_and_resolve;
    use std::collections::HashMap;

    let formable = extract(
        "RUS_f = {\n\tlevel = 3\n\ttag = RUS\n}\n",
        "in_game/common/formable_countries/00.txt",
    );
    assert_eq!(formable.defs[0].kind, pdxl_eu5::kinds::FORMABLE_COUNTRY);
    assert_eq!(formable.aliases.len(), 1);
    assert_eq!(formable.aliases[0].name, "RUS");

    let user = extract(
        "e = { every_country = { limit = { this = c:RUS } } }\n",
        "in_game/common/scripted_effects/x.txt",
    );
    let r = user.refs.iter().find(|r| r.name == "RUS").expect("c: ref");
    assert_eq!(r.kind, pdxl_eu5::kinds::COUNTRY);
    assert_eq!(
        r.alt,
        &[
            pdxl_eu5::kinds::FORMABLE_COUNTRY,
            pdxl_eu5::kinds::START_COUNTRY
        ]
    );

    // End-to-end: the alias satisfies the alt chain.
    let mut facts = HashMap::new();
    facts.insert(
        "in_game/common/formable_countries/00.txt".to_string(),
        formable,
    );
    facts.insert("in_game/common/scripted_effects/x.txt".to_string(), user);
    let order = [
        "in_game/common/formable_countries/00.txt",
        "in_game/common/scripted_effects/x.txt",
    ];
    let (_, diags) = merge_and_resolve(&order, &facts);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn named_color_defs_and_color_context() {
    let f = extract(
        "colors = {\n\twhite = hsv360 { 0 0 92 }\n}\n",
        "main_menu/common/named_colors/01_coa.txt",
    );
    assert_eq!(f.defs[0].name, "white");
    assert_eq!(f.defs[0].kind, pdxl_eu5::kinds::NAMED_COLOR);

    // The country body's color fields open Color context (LSP swatches).
    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let ctx = context_of_chain(
        [b"AHI".as_slice(), b"color2".as_slice()],
        "in_game/setup/countries/x.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    assert_eq!(ctx, ClauseKind::Color);
}

#[test]
fn start_scenario_countries_nested_container() {
    use pdxl_analysis::merge_and_resolve;
    use std::collections::HashMap;

    // The start file nests the container: countries = { countries = { … } }.
    let start = extract(
        "current_age = age_1\n\
         countries = {\n\
         \tcountries = {\n\
         \t\tGEN = {\n\t\t\town_control_core = { genova }\n\t\t}\n\
         \t}\n\
         }\n",
        "main_menu/setup/start/10_countries.txt",
    );
    assert_eq!(start.defs.len(), 1, "{:?}", start.defs);
    assert_eq!(start.defs[0].name, "GEN");
    assert_eq!(start.defs[0].kind, pdxl_eu5::kinds::START_COUNTRY);

    // `c:GEN` resolves through the alt chain even with no data entry.
    let user = extract(
        "e = { x = c:GEN }\n",
        "in_game/common/scripted_effects/x.txt",
    );
    let mut facts = HashMap::new();
    facts.insert("main_menu/setup/start/10_countries.txt".to_string(), start);
    facts.insert("in_game/common/scripted_effects/x.txt".to_string(), user);
    let order = [
        "main_menu/setup/start/10_countries.txt",
        "in_game/common/scripted_effects/x.txt",
    ];
    let (_, diags) = merge_and_resolve(&order, &facts);
    assert!(diags.is_empty(), "{diags:?}");
}
