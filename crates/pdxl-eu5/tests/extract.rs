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
            pdxl_eu5::kinds::START_COUNTRY,
            pdxl_eu5::kinds::DYNAMIC_COUNTRY
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

#[test]
fn advance_domain_defs_and_refs() {
    let f = extract(
        "central_bank_advance = {\n\
         \tage = age_5_absolutism\n\
         \tunlock_building = central_bank\n\
         \tmax_bonds = 5\n\
         \trequires = manufactories_advance\n\
         \tallow = { has_embraced_institution = institution:banking }\n\
         \tai_weight = { add = 100 }\n\
         }\n",
        "in_game/common/advances/0_age_of_absolutism.txt",
    );
    assert_eq!(f.defs[0].kind, pdxl_eu5::kinds::ADVANCE);
    let kind_of = |n: &str| f.refs.iter().find(|r| r.name == n).expect(n).kind;
    assert_eq!(kind_of("age_5_absolutism"), pdxl_eu5::kinds::AGE);
    assert_eq!(kind_of("manufactories_advance"), pdxl_eu5::kinds::ADVANCE);
    assert_eq!(kind_of("central_bank"), pdxl_eu5::kinds::BUILDING);

    // has_advance resolves anywhere; `requires` means nothing elsewhere.
    let t = extract(
        "e = { has_advance = sanitation_advance requires = whatever }\n",
        "in_game/common/scripted_triggers/x.txt",
    );
    assert_eq!(t.refs.len(), 1, "{:?}", t.refs);
    assert_eq!(t.refs[0].kind, pdxl_eu5::kinds::ADVANCE);

    // `unlock_unit = yes` is a toggle, not a unit reference.
    let y = extract(
        "x = { unlock_unit = yes }\n",
        "in_game/common/building_types/00.txt",
    );
    assert!(y.refs.iter().all(|r| r.kind != pdxl_eu5::kinds::UNIT));

    // Loose advance body keys are modifier tags (context check).
    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let ctx = context_of_chain(
        [b"central_bank_advance".as_slice()],
        "in_game/common/advances/0.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    let modifier_tag = pdxl_analysis::context::resolve_key(ctx, "global_life_expectancy", false);
    assert_eq!(modifier_tag, ClauseKind::StaticModifier); // Modifier fallback
    let allow = pdxl_analysis::context::resolve_key(ctx, "allow", true);
    assert_eq!(allow, ClauseKind::Trigger);
}

#[test]
fn dynamic_tags_and_bare_tag_refs() {
    use pdxl_analysis::merge_and_resolve;
    use std::collections::HashMap;

    // `define_unique_country_tag = X` inside an event effect CREATES the tag.
    let creator = extract(
        "namespace = flavor_gen\n\
         flavor_gen.1 = {\n\
         \timmediate = { define_unique_country_tag = SAGEO }\n\
         }\n",
        "in_game/events/DHE/flavor_GEN.txt",
    );
    let dynamic = creator
        .defs
        .iter()
        .find(|d| d.name == "SAGEO")
        .expect("dynamic tag def");
    assert_eq!(dynamic.kind, pdxl_eu5::kinds::DYNAMIC_COUNTRY);

    // `has_or_had_tag = SAGEO` resolves against it through the alt chain;
    // `has_or_had_tag = ROOT` is a scope keyword, not a reference.
    let user = extract(
        "flavor_gen.2 = { trigger = { has_or_had_tag = SAGEO has_or_had_tag = ROOT } }\n",
        "in_game/events/DHE/flavor_GEN2.txt",
    );
    assert_eq!(
        user.refs
            .iter()
            .filter(|r| r.kind == pdxl_eu5::kinds::COUNTRY)
            .count(),
        1
    );
    let mut facts = HashMap::new();
    facts.insert("in_game/events/DHE/flavor_GEN.txt".to_string(), creator);
    facts.insert("in_game/events/DHE/flavor_GEN2.txt".to_string(), user);
    let order = [
        "in_game/events/DHE/flavor_GEN.txt",
        "in_game/events/DHE/flavor_GEN2.txt",
    ];
    let (_, diags) = merge_and_resolve(&order, &facts);
    assert!(diags.is_empty(), "{diags:?}");

    // Bare `tag = X` refs in events; coa refs in flag definitions.
    let t = extract("e = { tag = GEN }\n", "in_game/events/x.txt");
    assert_eq!(t.refs[0].kind, pdxl_eu5::kinds::COUNTRY);
    let c = extract(
        "GEN = { flag_definition = { coa = GEN_republic priority = 2 } }\n",
        "main_menu/common/flag_definitions/00.txt",
    );
    assert_eq!(c.refs[0].kind, pdxl_eu5::kinds::COAT_OF_ARMS);
    assert_eq!(c.refs[0].name, "GEN_republic");
}

#[test]
fn age_body_and_estate_refs() {
    let f = extract(
        "age_5_absolutism = {\n\
         \tyear = 1637\n\
         \thegemons_allowed = yes\n\
         \tunique = { revoke_privilege_cost_modifier = -0.33 }\n\
         \tmax_ai_privilege_per_estate = {\n\
         \t\tnobles_estate = 4\n\
         \t\tclergy_estate = 4\n\
         \t}\n\
         }\n",
        "in_game/common/age/00_default.txt",
    );
    assert_eq!(f.defs[0].kind, pdxl_eu5::kinds::AGE);
    let estates: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_eu5::kinds::ESTATE)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(estates, vec!["nobles_estate", "clergy_estate"]);

    // estate_type: literals and bare estate = X resolve anywhere; chain
    // forms are skipped.
    let e = extract(
        "e = {\n\
         \tx = estate_type:burghers_estate\n\
         \testate = tribes_estate\n\
         \testate = scope:target_estate\n\
         }\n",
        "in_game/common/scripted_effects/x.txt",
    );
    let names: Vec<&str> = e
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_eu5::kinds::ESTATE)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, vec!["burghers_estate", "tribes_estate"]);

    // Age body context: unique = { … } is a static-modifier clause.
    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let ctx = context_of_chain(
        [b"age_5_absolutism".as_slice(), b"unique".as_slice()],
        "in_game/common/age/00_default.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    assert_eq!(ctx, ClauseKind::StaticModifier);
}
