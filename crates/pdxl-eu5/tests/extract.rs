//! EU5 schema extraction: the country domain (facts-level, corpus-shaped).

use pdxl_analysis::{FileFacts, extract_facts};

fn extract(src: &str, rel_path: &str) -> FileFacts {
    let schema = pdxl_eu5::schema();
    let (tree, _) = pdxl_parser::parse("test".to_string(), src.as_bytes().to_vec()).into_parts();
    extract_facts(&tree, rel_path, "test", &schema, None)
}

#[test]
fn situations_define_refs_localization_and_scoped_structure() {
    let facts = extract(
        "black_death = { custom_description = GetBlackDeathDesc monthly_spawn_chance = monthly_spawn_chance_unique hint_tag = hint_black_death can_start = { always = yes } visible = { always = yes } on_start = { situation = black_death } tooltip = { custom_tooltip = X } map_color = { value = red } legend_key = { desc = BLACK_DEATH color = black require_color_on_map = yes } }\n",
        "in_game/common/situations/black_death.txt",
    );
    assert!(
        facts
            .defs
            .iter()
            .any(|d| d.kind == pdxl_eu5::kinds::SITUATION && d.name == "black_death")
    );
    assert!(
        facts
            .refs
            .iter()
            .any(|r| r.kind == pdxl_eu5::kinds::SITUATION && r.name == "black_death")
    );
    assert!(
        facts
            .refs
            .iter()
            .any(|r| r.kind == pdxl_eu5::kinds::CUSTOM_LOC && r.name == "GetBlackDeathDesc")
    );
    assert!(
        facts
            .refs
            .iter()
            .any(|r| { r.kind == pdxl_eu5::kinds::NAMED_COLOR && r.name == "black" })
    );
    for name in ["hint_black_death", "BLACK_DEATH"] {
        assert!(
            facts
                .refs
                .iter()
                .any(|r| r.kind == pdxl_eu5::kinds::LOC_KEY && r.name == name),
            "{name}"
        );
    }

    let schema = pdxl_eu5::schema();
    assert_eq!(
        schema.loc_datafn_arg_kind("ShowSituationName"),
        Some(pdxl_eu5::kinds::SITUATION)
    );
    assert_eq!(
        schema
            .implicit_loc_patterns(pdxl_eu5::kinds::SITUATION)
            .iter()
            .map(|p| p.suffix)
            .collect::<Vec<_>>(),
        ["", "_desc"]
    );

    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    for (field, expected_scope) in [
        ("can_start", "situation"),
        ("visible", "country"),
        ("on_start", "situation"),
        ("tooltip", "location"),
    ] {
        let context = context_of_chain(
            [b"black_death".as_slice(), field.as_bytes()],
            "in_game/common/situations/black_death.txt",
            pdxl_eu5::contexts::context_schema(),
        );
        assert!(matches!(context, ClauseKind::Trigger | ClauseKind::Effect));
        let root = context_of_chain(
            [b"black_death".as_slice()],
            "in_game/common/situations/black_death.txt",
            pdxl_eu5::contexts::context_schema(),
        );
        let ClauseKind::Struct(spec) = root else {
            panic!()
        };
        assert_eq!(
            spec.field(field).and_then(|f| f.scope),
            Some(expected_scope)
        );
    }
}

#[test]
fn religion_families_define_and_cross_reference_entities() {
    let cases = [
        (
            "in_game/common/religions/x.txt",
            "judaism = { group = israelite_group important_country = ISR has_religious_head = yes religious_aspects = 3 religious_school = hanafi_school religious_focuses = { adopt_ometeotl } factions = { imperial_court } opinions = { shinto = negative } }",
            pdxl_eu5::kinds::RELIGION,
            "judaism",
        ),
        (
            "in_game/common/religion_groups/x.txt",
            "israelite_group = { convert_slaves_at_start = no }",
            pdxl_eu5::kinds::RELIGION_GROUP,
            "israelite_group",
        ),
        (
            "in_game/common/religious_aspects/x.txt",
            "pacifism = { religion = judaism visible = { always = yes } }",
            pdxl_eu5::kinds::RELIGIOUS_ASPECT,
            "pacifism",
        ),
        (
            "in_game/common/religious_factions/x.txt",
            "imperial_court = { visible = { always = yes } actions = { act } }",
            pdxl_eu5::kinds::RELIGIOUS_FACTION,
            "imperial_court",
        ),
        (
            "in_game/common/religious_figures/x.txt",
            "muslim_scholar = { enabled_for_religion = { always = yes } }",
            pdxl_eu5::kinds::RELIGIOUS_FIGURE,
            "muslim_scholar",
        ),
        (
            "in_game/common/religious_focuses/x.txt",
            "adopt_ometeotl = { allow = { always = yes } monthly_progress = 1 }",
            pdxl_eu5::kinds::RELIGIOUS_FOCUS,
            "adopt_ometeotl",
        ),
        (
            "in_game/common/religious_schools/x.txt",
            "hanafi_school = { enabled_for_country = { always = yes } }",
            pdxl_eu5::kinds::RELIGIOUS_SCHOOL,
            "hanafi_school",
        ),
    ];
    let mut all = Vec::new();
    for (path, source, kind, name) in cases {
        let facts = extract(source, path);
        assert!(
            facts.defs.iter().any(|d| d.kind == kind && d.name == name),
            "{name}"
        );
        all.extend(facts.refs);
    }
    for (kind, name) in [
        (pdxl_eu5::kinds::RELIGION_GROUP, "israelite_group"),
        (pdxl_eu5::kinds::RELIGIOUS_SCHOOL, "hanafi_school"),
        (pdxl_eu5::kinds::RELIGIOUS_FOCUS, "adopt_ometeotl"),
        (pdxl_eu5::kinds::RELIGIOUS_FACTION, "imperial_court"),
        (pdxl_eu5::kinds::RELIGION, "shinto"),
        (pdxl_eu5::kinds::RELIGION, "judaism"),
        (pdxl_eu5::kinds::COUNTRY, "ISR"),
    ] {
        assert!(
            all.iter().any(|r| r.kind == kind && r.name == name),
            "{name}"
        );
    }
    let body = pdxl_analysis::context::context_of_chain(
        [b"catholic".as_slice()],
        "in_game/common/religions/christian.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    let pdxl_analysis::context::ClauseKind::Struct(spec) = body else {
        panic!("religion body should be structural")
    };
    for field in [
        "enable",
        "religious_aspects",
        "has_religious_influence",
        "ai_wants_convert",
        "unique_names",
        "custom_tags",
        "has_canonization",
        "has_autocephalous_patriarchates",
        "has_patriarchs",
        "has_religious_head",
        "has_cardinals",
        "important_country",
        "needs_reform",
        "tithe",
        "use_icons",
        "goods_demand_modifier",
        "clergy_goods_demand_modifier",
    ] {
        assert!(spec.field(field).is_some(), "{field}");
    }

    let schema = pdxl_eu5::schema();
    assert_eq!(
        schema.loc_datafn_arg_kind("ShowReligiousSchoolName"),
        Some(pdxl_eu5::kinds::RELIGIOUS_SCHOOL)
    );
}

#[test]
fn named_locations_and_map_hierarchy_are_entities() {
    let locations = extract(
        "stockholm = dda910\naachen = 123456\n",
        "in_game/map_data/named_locations/00_default.txt",
    );
    assert!(
        locations
            .defs
            .iter()
            .any(|d| d.kind == pdxl_eu5::kinds::LOCATION && d.name == "stockholm")
    );

    let map = extract(
        "europe = { western_europe = { scandinavian_region = { svealand_area = { uppland_province = { stockholm } } } } }\n",
        "in_game/map_data/definitions.txt",
    );
    for (kind, name) in [
        (pdxl_eu5::kinds::CONTINENT, "europe"),
        (pdxl_eu5::kinds::SUB_CONTINENT, "western_europe"),
        (pdxl_eu5::kinds::REGION, "scandinavian_region"),
        (pdxl_eu5::kinds::AREA, "svealand_area"),
        (pdxl_eu5::kinds::PROVINCE, "uppland_province"),
    ] {
        assert!(
            map.defs.iter().any(|d| d.kind == kind && d.name == name),
            "{name}"
        );
    }
    assert!(
        map.refs
            .iter()
            .any(|r| r.kind == pdxl_eu5::kinds::LOCATION && r.name == "stockholm")
    );

    let manager = extract(
        "institution_manager = { institutions = { feudalism = { active = yes birth_place = aachen } } }\n",
        "main_menu/setup/start/02_core.txt",
    );
    assert!(
        manager
            .refs
            .iter()
            .any(|r| r.kind == pdxl_eu5::kinds::LOCATION && r.name == "aachen")
    );
}

#[test]
fn religion_manager_keys_and_relations_reference_schools() {
    let facts = extract(
        "religion_manager = { maliki_school = { relation = { hanafi_school = kindred ismaili_school = enemy } } hanbali_school = { athari_school = kindred mutazili_school = enemy } }\n",
        "main_menu/setup/start/02_core.txt",
    );
    for name in [
        "maliki_school",
        "hanafi_school",
        "ismaili_school",
        "hanbali_school",
        "athari_school",
        "mutazili_school",
    ] {
        assert!(
            facts
                .refs
                .iter()
                .any(|r| { r.kind == pdxl_eu5::kinds::RELIGIOUS_SCHOOL && r.name == name }),
            "{name}"
        );
    }

    let relation = pdxl_analysis::context::context_of_chain(
        [
            b"religion_manager".as_slice(),
            b"maliki_school".as_slice(),
            b"relation".as_slice(),
        ],
        "main_menu/setup/start/02_core.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    let pdxl_analysis::context::ClauseKind::Struct(spec) = relation else {
        panic!("religious-school relation map should be structural")
    };
    assert_eq!(spec.name, "religious school relations");
}

#[test]
fn institution_manager_keys_reference_institutions() {
    let facts = extract(
        "institution_manager = { institutions = { feudalism = { active = yes birth_place = aachen } } }\n",
        "main_menu/setup/start/02_core.txt",
    );
    assert!(
        facts
            .refs
            .iter()
            .any(|r| { r.kind == pdxl_eu5::kinds::INSTITUTION && r.name == "feudalism" })
    );

    let context = pdxl_analysis::context::context_of_chain(
        [
            b"institution_manager".as_slice(),
            b"institutions".as_slice(),
            b"feudalism".as_slice(),
        ],
        "main_menu/setup/start/02_core.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    let pdxl_analysis::context::ClauseKind::Struct(spec) = context else {
        panic!("institution setup should be structural")
    };
    assert!(spec.field("active").is_some());
    assert!(spec.field("birth_place").is_some());
}

#[test]
fn institutions_define_refs_and_location_scoped_spawn_triggers() {
    let facts = extract(
        "printing_press = { age = age_3_discovery can_spawn = { has_owner = yes } spread_from_any_export = 1 }\n",
        "in_game/common/institution/x.txt",
    );
    assert!(
        facts
            .defs
            .iter()
            .any(|d| d.kind == pdxl_eu5::kinds::INSTITUTION && d.name == "printing_press")
    );
    assert!(
        facts
            .refs
            .iter()
            .any(|r| r.kind == pdxl_eu5::kinds::AGE && r.name == "age_3_discovery")
    );

    let usage = extract(
        "x = { has_institution = institution:printing_press institution = printing_press }\n",
        "in_game/events/x.txt",
    );
    assert_eq!(
        usage
            .refs
            .iter()
            .filter(|r| r.kind == pdxl_eu5::kinds::INSTITUTION && r.name == "printing_press")
            .count(),
        2
    );

    let schema = pdxl_eu5::schema();
    assert_eq!(
        schema.loc_datafn_arg_kind("ShowInstitutionName"),
        Some(pdxl_eu5::kinds::INSTITUTION)
    );
    assert_eq!(
        schema
            .implicit_loc_patterns(pdxl_eu5::kinds::INSTITUTION)
            .iter()
            .map(|p| p.suffix)
            .collect::<Vec<_>>(),
        ["", "_desc"]
    );
}

#[test]
fn events_define_ids_refs_and_localization_fields() {
    let facts = extract(
        "namespace = test\ntest.1 = { type = country_event title = test.1.title desc = { first_valid = { triggered_desc = { trigger = { always = yes } desc = test.1.desc } } } option = { name = test.1.a add_stability = 1 } }\ncaller = { trigger_event_silently = { id = test.1 } trigger_event_non_silently = test.1 }\n",
        "in_game/events/test.txt",
    );
    assert!(
        facts
            .defs
            .iter()
            .any(|d| d.kind == pdxl_eu5::kinds::EVENT && d.name == "test.1")
    );
    assert_eq!(
        facts
            .refs
            .iter()
            .filter(|r| r.kind == pdxl_eu5::kinds::EVENT && r.name == "test.1")
            .count(),
        2
    );
    for key in ["test.1.title", "test.1.desc", "test.1.a"] {
        assert!(
            facts
                .refs
                .iter()
                .any(|r| r.kind == pdxl_analysis::LOC_KEY && r.name == key),
            "{key}"
        );
    }
}

#[test]
fn game_concepts_define_aliases_and_family_refs() {
    let facts = extract(
        "goods = { alias = { good } texture = x }\ngoods_demand = { family = good }\n",
        "main_menu/common/game_concepts/00_game_concepts.txt",
    );
    assert!(
        facts
            .defs
            .iter()
            .any(|d| d.kind == pdxl_eu5::kinds::GAME_CONCEPT && d.name == "goods")
    );
    assert!(
        facts
            .aliases
            .iter()
            .any(|d| d.kind == pdxl_eu5::kinds::GAME_CONCEPT && d.name == "good")
    );
    assert!(
        facts
            .refs
            .iter()
            .any(|r| r.kind == pdxl_eu5::kinds::GAME_CONCEPT && r.name == "good")
    );
}

#[test]
fn entity_localization_patterns_are_schema_owned() {
    let schema = pdxl_eu5::schema();
    let suffixes = |kind| {
        schema
            .implicit_loc_patterns(kind)
            .iter()
            .map(|pattern| pattern.suffix)
            .collect::<Vec<_>>()
    };
    assert_eq!(suffixes(pdxl_eu5::kinds::ADVANCE), ["", "_desc"]);
    assert_eq!(suffixes(pdxl_eu5::kinds::COUNTRY), ["", "_ADJ"]);
    let concepts = schema.implicit_loc_patterns(pdxl_eu5::kinds::GAME_CONCEPT);
    assert_eq!(
        concepts.iter().map(|p| p.suffix).collect::<Vec<_>>(),
        ["game_concept_{}", "game_concept_{}_desc",]
    );
    assert_eq!(concepts[0].loc_name("modifier"), "game_concept_modifier");
    assert_eq!(
        concepts[1].entity_name("game_concept_modifier_desc"),
        Some("modifier")
    );
    for kind in [
        pdxl_eu5::kinds::LOCATION,
        pdxl_eu5::kinds::PROVINCE,
        pdxl_eu5::kinds::AREA,
        pdxl_eu5::kinds::REGION,
        pdxl_eu5::kinds::SUB_CONTINENT,
        pdxl_eu5::kinds::CONTINENT,
    ] {
        assert_eq!(suffixes(kind), [""]);
    }
    assert_eq!(suffixes(pdxl_eu5::kinds::START_COUNTRY), ["", "_ADJ"]);
    assert_eq!(suffixes(pdxl_eu5::kinds::FORMABLE_COUNTRY), ["", "_ADJ"]);
    assert_eq!(suffixes(pdxl_eu5::kinds::SUBJECT_TYPE), ["", "_desc"]);
    assert_eq!(
        suffixes(pdxl_eu5::kinds::INTERNATIONAL_ORGANIZATION),
        ["", "_desc"]
    );
    assert_eq!(suffixes(pdxl_eu5::kinds::IO_SPECIAL_STATUS), ["", "_desc"]);
    assert_eq!(suffixes(pdxl_eu5::kinds::IO_PAYMENT), ["", "_desc"]);
    assert_eq!(suffixes(pdxl_eu5::kinds::IO_VARIABLE), [""]);
    assert_eq!(suffixes(pdxl_eu5::kinds::PARLIAMENT_TYPE), ["", "_desc"]);
}

#[test]
fn international_organization_nested_desc_values_are_loc_refs() {
    let facts = extract(
        "x = { variables = { communal_unity = { format = COMMUNAL_UNITY_DISPLAY change_format = VARIABLE_CHANGE_FORMAT monthly_change = { add = { desc = COMPACT_ORGANIZATION value = var:communal_unity } subtract = { desc = [opinion|e] value = 1 } } } } }\n",
        "in_game/common/international_organizations/x.txt",
    );
    assert!(
        facts
            .refs
            .iter()
            .any(|r| { r.kind == pdxl_analysis::LOC_KEY && r.name == "COMPACT_ORGANIZATION" })
    );
    assert!(
        facts
            .refs
            .iter()
            .any(|r| { r.kind == pdxl_analysis::LOC_KEY && r.name == "COMMUNAL_UNITY_DISPLAY" })
    );
    assert!(
        facts
            .refs
            .iter()
            .any(|r| { r.kind == pdxl_analysis::LOC_KEY && r.name == "VARIABLE_CHANGE_FORMAT" })
    );
    assert!(!facts.refs.iter().any(|r| r.name == "opinion"));
    assert!(
        facts
            .defs
            .iter()
            .any(|d| { d.kind == pdxl_eu5::kinds::IO_VARIABLE && d.name == "communal_unity" })
    );
    assert!(
        facts
            .calls
            .iter()
            .any(|r| { r.kind == pdxl_eu5::kinds::IO_VARIABLE && r.name == "communal_unity" })
    );
    assert!(!facts.refs.iter().any(|r| r.name == "communal_unity"));
}

#[test]
fn international_organization_loc_datafunctions_are_schema_owned() {
    let schema = pdxl_eu5::schema();
    for (function, kind) in [
        (
            "GetUniqueInternationalOrganization",
            pdxl_eu5::kinds::INTERNATIONAL_ORGANIZATION,
        ),
        ("ShowSpecialStatusName", pdxl_eu5::kinds::IO_SPECIAL_STATUS),
        (
            "ShowSpecialStatusNamePluralWithNoTooltip",
            pdxl_eu5::kinds::IO_SPECIAL_STATUS,
        ),
        ("ShowPaymentName", pdxl_eu5::kinds::IO_PAYMENT),
        ("ShowParliamentTypeName", pdxl_eu5::kinds::PARLIAMENT_TYPE),
    ] {
        assert_eq!(
            schema.loc_datafn_arg_kind(function),
            Some(kind),
            "{function}"
        );
    }
}

#[test]
fn qualified_define_defs_and_pipe_refs() {
    let defs = extract(
        "@helper = 2\nNMapColors = { LEADER_COLOR = { 1 2 3 } WIDTH = 4 }\n",
        "loading_screen/common/defines/graphic/colors.txt",
    );
    let names: Vec<_> = defs.defs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["NMapColors|LEADER_COLOR", "NMapColors|WIDTH"]);
    assert!(defs.defs.iter().all(|d| d.kind == pdxl_eu5::kinds::DEFINE));
    assert!(defs.defs.iter().all(|d| d.name != "@helper"));

    let usage = extract(
        "x = { map_color = define:NMapColors|LEADER_COLOR }\n",
        "in_game/common/international_organization_special_statuses/x.txt",
    );
    let reference = usage
        .refs
        .iter()
        .find(|r| r.name == "NMapColors|LEADER_COLOR")
        .expect("qualified define ref");
    assert_eq!(reference.kind, pdxl_eu5::kinds::DEFINE);
}

#[test]
fn international_organization_gold_treasury_field() {
    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let body = context_of_chain(
        [b"catholic_church".as_slice()],
        "in_game/common/international_organizations/catholic_church.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    let ClauseKind::Struct(spec) = body else {
        panic!("IO body should be structural")
    };
    let gold = spec.field("gold").expect("gold treasury field");
    assert_eq!(
        gold.scalar,
        Some(pdxl_analysis::context::ScalarKind::Setting)
    );
    assert_eq!(gold.values, Some(&["yes", "no"][..]));
}

#[test]
fn customizable_localization_defs_refs_and_body() {
    let f = extract(
        "base_name = { type = country text = { localization_key = base_name trigger = { always = yes } } }\n\
         derived_name = { parent = base_name suffix = \"_ADJ\" log_loc_errors = no if_invalid_loc = return_empty }\n",
        "in_game/common/customizable_localization/names.txt",
    );
    assert_eq!(f.defs.len(), 2);
    assert!(f.defs.iter().all(|d| d.kind == pdxl_eu5::kinds::CUSTOM_LOC));
    assert!(
        f.refs
            .iter()
            .any(|r| r.name == "base_name" && r.kind == pdxl_eu5::kinds::CUSTOM_LOC)
    );
    // localization_key is intentionally not validated against one language.
    assert!(f.refs.iter().all(|r| r.name != "base_name_ADJ"));

    let io = extract(
        "x = { custom_name = base_name }\n",
        "in_game/common/international_organizations/x.txt",
    );
    assert!(
        io.refs
            .iter()
            .any(|r| r.name == "base_name" && r.kind == pdxl_eu5::kinds::CUSTOM_LOC)
    );

    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let schema = pdxl_eu5::contexts::context_schema();
    let body = context_of_chain(
        [b"base_name".as_slice()],
        "in_game/common/customizable_localization/names.txt",
        schema,
    );
    assert!(matches!(body, ClauseKind::Struct(spec) if spec.name == "customizable localization"));
    assert!(matches!(
        pdxl_analysis::context::resolve_key(body, "text", true),
        ClauseKind::Struct(spec) if spec.name == "customizable localization text"
    ));
}

#[test]
fn parliament_type_defs_refs_and_body() {
    let f = extract(
        "assembly = {\n\
         \ttype = country\n\
         \tpotential = { always = yes }\n\
         \tallow = { always = yes }\n\
         \tlocked = { always = no }\n\
         \tmodifier = { has_a_parliamentary_system = yes }\n\
         }\n",
        "in_game/common/parliament_types/00_default.txt",
    );
    assert_eq!(f.defs.len(), 1);
    assert_eq!(f.defs[0].name, "assembly");
    assert_eq!(f.defs[0].kind, pdxl_eu5::kinds::PARLIAMENT_TYPE);

    let refs = extract(
        "e = { set_parliament_type = parliament_type:assembly }\n",
        "in_game/events/x.txt",
    );
    assert!(
        refs.refs
            .iter()
            .any(|r| r.name == "assembly" && r.kind == pdxl_eu5::kinds::PARLIAMENT_TYPE)
    );

    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let schema = pdxl_eu5::contexts::context_schema();
    let body = context_of_chain(
        [b"assembly".as_slice()],
        "in_game/common/parliament_types/00_default.txt",
        schema,
    );
    assert!(matches!(body, ClauseKind::Struct(spec) if spec.name == "parliament type"));
    let ClauseKind::Struct(country_spec) = body else {
        unreachable!()
    };
    assert_eq!(
        country_spec.field("potential").unwrap().scope,
        Some("country")
    );
    let io_body = context_of_chain(
        [b"union_royal_assembly".as_slice()],
        "in_game/common/parliament_types/01_international_organization.txt",
        schema,
    );
    let ClauseKind::Struct(io_spec) = io_body else {
        panic!("IO parliament body should be structural")
    };
    assert_eq!(
        io_spec.field("locked").unwrap().scope,
        Some("international_organization")
    );
    assert_eq!(
        pdxl_analysis::context::resolve_key(body, "potential", true),
        ClauseKind::Trigger
    );
    assert_eq!(
        pdxl_analysis::context::resolve_key(body, "modifier", true),
        ClauseKind::StaticModifier
    );
}

#[test]
fn bias_defs_refs_and_context() {
    let f = extract(
        "opinion_sabotage_reputation = { value = -50 yearly_decay = 5 min = -200 }\n",
        "in_game/common/biases/03_opinion.txt",
    );
    assert_eq!(f.defs.len(), 1);
    assert_eq!(f.defs[0].name, "opinion_sabotage_reputation");
    assert_eq!(f.defs[0].kind, pdxl_eu5::kinds::BIAS);

    let refs = extract(
        "e = { add_opinion = { target = scope:x modifier = opinion_sabotage_reputation } }\n",
        "in_game/events/x.txt",
    );
    let bias_ref = refs
        .refs
        .iter()
        .find(|r| r.name == "opinion_sabotage_reputation")
        .expect("bias ref");
    assert_eq!(bias_ref.kind, pdxl_eu5::kinds::BIAS);

    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let ctx = context_of_chain(
        [b"opinion_sabotage_reputation".as_slice()],
        "in_game/common/biases/03_opinion.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    assert!(matches!(ctx, ClauseKind::Struct(spec) if spec.name == "bias"));
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
        "colors = {\n\tmap_austrian = rgb { 220 220 220 }\n\tmap_german = hsv360 { 180 10 50 }\n}\n",
        "main_menu/common/named_colors/02_map.txt",
    );
    assert_eq!(f.defs[0].name, "map_austrian");
    assert_eq!(f.defs[0].kind, pdxl_eu5::kinds::NAMED_COLOR);
    assert!(f.defs.iter().any(|d| d.name == "map_german"));

    let religion = extract(
        "catholic = { color = map_austrian }\n",
        "in_game/common/religions/x.txt",
    );
    assert!(
        religion
            .refs
            .iter()
            .any(|r| { r.kind == pdxl_eu5::kinds::NAMED_COLOR && r.name == "map_austrian" })
    );

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

#[test]
fn subject_type_defs_refs_and_body() {
    let f = extract(
        "maona = {\n\
         \tcolor = subject_vassal\n\
         \tlevel = 1\n\
         \tjoin_defensive_wars_always = { scope:actor = { is_subject_of = scope:recipient } }\n\
         \tsubject_pays = subject_pays_vassal\n\
         }\n",
        "in_game/common/subject_types/cci_maona.txt",
    );
    assert_eq!(f.defs[0].name, "maona");
    assert_eq!(f.defs[0].kind, pdxl_eu5::kinds::SUBJECT_TYPE);
    let color = f
        .refs
        .iter()
        .find(|r| r.name == "subject_vassal")
        .expect("color ref");
    assert_eq!(color.kind, pdxl_eu5::kinds::NAMED_COLOR);

    // Both reference forms resolve anywhere; chain forms skip.
    let e = extract(
        "e = {\n\
         \tsubject_type = maona\n\
         \tx = subject_type:tributary\n\
         \tsubject_type = scope:st\n\
         }\n",
        "in_game/common/scripted_effects/x.txt",
    );
    let names: Vec<&str> = e
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_eu5::kinds::SUBJECT_TYPE)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, vec!["maona", "tributary"]);

    // Advances unlock subject types.
    let a = extract(
        "x = { age = age_3_discovery unlock_subject_type = colonial_nation }\n",
        "in_game/common/advances/0.txt",
    );
    let u = a
        .refs
        .iter()
        .find(|r| r.name == "colonial_nation")
        .expect("unlock ref");
    assert_eq!(u.kind, pdxl_eu5::kinds::SUBJECT_TYPE);

    // Body contexts: availability triggers are Trigger clauses; lifecycle
    // effects are Effect clauses.
    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let ctx = context_of_chain(
        [
            b"maona".as_slice(),
            b"join_defensive_wars_always".as_slice(),
        ],
        "in_game/common/subject_types/x.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    assert_eq!(ctx, ClauseKind::Trigger);
    let on_enable = context_of_chain(
        [b"maona".as_slice(), b"on_enable".as_slice()],
        "in_game/common/subject_types/x.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    assert_eq!(on_enable, ClauseKind::Effect);
}

#[test]
fn readme_unlock_targets_and_dual_site_production_methods() {
    use pdxl_analysis::merge_and_resolve;
    use std::collections::HashMap;

    // Production methods define in BOTH sites: their own dir and inline
    // unique_production_methods containers inside building bodies — the
    // first dual-rule directory (buildings still harvest as buildings).
    let building = extract(
        "brewery = {\n\
         \taudio_tier = 1\n\
         \tunique_production_methods = {\n\
         \t\tbavarian_brewery_maintenance = {\n\t\t\twheat = 1.3\n\t\t\tproduced = beer\n\t\t}\n\
         \t}\n\
         }\n",
        "in_game/common/building_types/production_beer.txt",
    );
    let kinds_of: Vec<(&str, &str)> = building
        .defs
        .iter()
        .map(|d| (d.name.as_str(), d.kind.name()))
        .collect();
    assert!(kinds_of.contains(&("brewery", "building")), "{kinds_of:?}");
    assert!(
        kinds_of.contains(&("bavarian_brewery_maintenance", "production_method")),
        "{kinds_of:?}"
    );

    // The advance's unlock resolves against the inline definition.
    let advance = extract(
        "beer_advance = {\n\
         \tage = age_2_renaissance\n\
         \tunlock_production_method = bavarian_brewery_maintenance\n\
         \tallow_children = yes\n\
         \tmodifier_while_progressing = {\n\
         \t\tpotential_trigger = { is_at_war = no }\n\
         \t\tscale = 0.5\n\
         \t\tglobal_tax_modifier = 0.1\n\
         \t}\n\
         }\n",
        "in_game/common/advances/1_building_unlocks.txt",
    );
    let mut facts = HashMap::new();
    facts.insert(
        "in_game/common/building_types/production_beer.txt".to_string(),
        building,
    );
    facts.insert(
        "in_game/common/advances/1_building_unlocks.txt".to_string(),
        advance,
    );
    let order = [
        "in_game/common/building_types/production_beer.txt",
        "in_game/common/advances/1_building_unlocks.txt",
    ];
    let (_, diags) = merge_and_resolve(&order, &facts);
    // age_2_renaissance is genuinely undefined in this two-file fixture.
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert!(diags[0].msg.contains("unknown age"));

    // modifier_while_progressing contexts: trigger inside, modifier fallback.
    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let ctx = context_of_chain(
        [
            b"beer_advance".as_slice(),
            b"modifier_while_progressing".as_slice(),
        ],
        "in_game/common/advances/0.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    assert_eq!(
        pdxl_analysis::context::resolve_key(ctx, "potential_trigger", true),
        ClauseKind::Trigger
    );
    assert_eq!(
        pdxl_analysis::context::resolve_key(ctx, "global_tax_modifier", false),
        ClauseKind::StaticModifier
    );
}

#[test]
fn government_reform_body_and_refs() {
    // The mod's maona charter reform: derived government_reform: literal,
    // scaled country_modifier, locked/potential triggers.
    let f = extract(
        "maona_charter = {\n\
         \tage = age_2_renaissance\n\
         \tgovernment = republic\n\
         \tpotential = { has_reform = government_reform:maona_charter }\n\
         \tcountry_modifier = {\n\
         \t\tpotential_trigger = { is_at_war = no }\n\
         \t\ttrade_efficiency = 0.1\n\
         \t}\n\
         \tyears = 0\n\
         }\n",
        "in_game/common/government_reforms/cci_maona_reform.txt",
    );
    assert_eq!(f.defs[0].kind, pdxl_eu5::kinds::GOVERNMENT_REFORM);
    let kind_of = |n: &str| f.refs.iter().find(|r| r.name == n).expect(n).kind;
    assert_eq!(kind_of("age_2_renaissance"), pdxl_eu5::kinds::AGE);
    assert_eq!(kind_of("republic"), pdxl_eu5::kinds::GOVERNMENT_TYPE);
    // The derived scope-link literal self-references the reform.
    assert_eq!(kind_of("maona_charter"), pdxl_eu5::kinds::GOVERNMENT_REFORM);

    // Body contexts: locked/potential are triggers; the scaled modifier's
    // loose keys are modifier tags.
    use pdxl_analysis::context::{ClauseKind, context_of_chain};
    let cm = context_of_chain(
        [b"maona_charter".as_slice(), b"country_modifier".as_slice()],
        "in_game/common/government_reforms/x.txt",
        pdxl_eu5::contexts::context_schema(),
    );
    assert_eq!(
        pdxl_analysis::context::resolve_key(cm, "trade_efficiency", false),
        ClauseKind::StaticModifier
    );
    assert_eq!(
        pdxl_analysis::context::resolve_key(cm, "potential_trigger", true),
        ClauseKind::Trigger
    );
}
