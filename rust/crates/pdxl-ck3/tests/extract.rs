//! Fact-extraction unit tests over the CK3 schema.
//!
//! Ports the extraction-level assertions from `internal/validate`'s
//! `validate_test.go` (definition collection, namespace skip, characters,
//! dotted IDs, macro params, unknown dirs) at the `FileFacts` level — the
//! SymbolTable half of those tests belongs to Milestone 6 — plus focused
//! coverage of every reference shape and skip rule.

use std::collections::HashSet;

use pdxl_analysis::{CallKinds, CallTargets, FileFacts, KindId, extract_facts};

/// Extracts facts from `src` as if it lived at `rel_path`.
fn extract(src: &str, rel_path: &str) -> FileFacts {
    let parsed = pdxl_parser::parse(rel_path.to_string(), src.as_bytes().to_vec());
    extract_facts(parsed.tree(), rel_path, rel_path, &pdxl_ck3::schema(), None)
}

/// Extracts facts with a set of callable scripted-effect/trigger names, so
/// call-by-name references (`my_effect = yes`) are recorded.
fn extract_with_calls(src: &str, rel_path: &str, effects: &[&str], triggers: &[&str]) -> FileFacts {
    extract_with_names(src, rel_path, effects, triggers, &[])
}

fn extract_with_names(
    src: &str,
    rel_path: &str,
    effects: &[&str],
    triggers: &[&str],
    script_values: &[&str],
) -> FileFacts {
    let parsed = pdxl_parser::parse(rel_path.to_string(), src.as_bytes().to_vec());
    let effects: HashSet<String> = effects.iter().map(|s| s.to_string()).collect();
    let triggers: HashSet<String> = triggers.iter().map(|s| s.to_string()).collect();
    let script_values: HashSet<String> = script_values.iter().map(|s| s.to_string()).collect();
    let targets = CallTargets {
        kinds: CallKinds {
            effect: pdxl_ck3::kinds::SCRIPTED_EFFECT,
            trigger: pdxl_ck3::kinds::SCRIPTED_TRIGGER,
            value: pdxl_ck3::kinds::SCRIPT_VALUE,
        },
        effects: &effects,
        triggers: &triggers,
        script_values: &script_values,
    };
    extract_facts(
        parsed.tree(),
        rel_path,
        rel_path,
        &pdxl_ck3::schema(),
        Some(&targets),
    )
}

fn def_names(f: &FileFacts) -> Vec<&str> {
    f.defs.iter().map(|s| s.name.as_str()).collect()
}

fn ref_names(f: &FileFacts) -> Vec<&str> {
    f.refs.iter().map(|r| r.name.as_str()).collect()
}

fn call_names(f: &FileFacts) -> Vec<(&str, KindId)> {
    f.calls.iter().map(|r| (r.name.as_str(), r.kind)).collect()
}

// ── call-by-name references (scripted effect / trigger invocations) ──────────

#[test]
fn records_scripted_calls_nested_not_at_top_level() {
    // `my_effect` appears both as a definition (top level) and as a call
    // (nested inside an event's effect block). Only the nested one is a call.
    let src = "\
my_effect = { add_gold = 5 }
namespace = test
test.1 = {
    immediate = {
        my_effect = yes
        my_trigger = { count = 2 }
        add_gold = 10
    }
}
";
    let f = extract_with_calls(src, "events/test.txt", &["my_effect"], &["my_trigger"]);
    // The top-level `my_effect = { … }` is a definition here, not a call.
    assert_eq!(
        call_names(&f),
        vec![
            ("my_effect", pdxl_ck3::kinds::SCRIPTED_EFFECT),
            ("my_trigger", pdxl_ck3::kinds::SCRIPTED_TRIGGER),
        ],
        "only nested invocations of known scripted names are calls"
    );
    // The call range covers the invoked key, not its value.
    let call = &f.calls[0];
    assert_eq!(&src[call.start as usize..call.end as usize], "my_effect");
}

#[test]
fn inline_typed_scripted_defs_kinded_not_as_events() {
    // `scripted_effect NAME = {}` inside an events file defines a scripted
    // effect (not an event), regardless of directory.
    let f = extract(
        "scripted_effect my_eff = { add_gold = 5 }\n\
         scripted_trigger my_trig = { always = yes }\n\
         actual.1 = { type = character_event }\n",
        "events/scheme_events/x.txt",
    );
    let by_kind: Vec<(&str, KindId)> = f.defs.iter().map(|d| (d.name.as_str(), d.kind)).collect();
    assert_eq!(
        by_kind,
        vec![
            ("my_eff", pdxl_ck3::kinds::SCRIPTED_EFFECT),
            ("my_trig", pdxl_ck3::kinds::SCRIPTED_TRIGGER),
            ("actual.1", pdxl_ck3::kinds::EVENT),
        ]
    );
    // The def offset points at the NAME, not the keyword.
    let d = &f.defs[0];
    let src = "scripted_effect my_eff = { add_gold = 5 }\n";
    assert_eq!(&src[d.offset as usize..d.end_offset as usize], "my_eff");
}

#[test]
fn script_value_defs_scalar_and_block() {
    // Both the scalar and formula forms are definitions.
    let f = extract(
        "minor_stress_gain = 10\nmy_formula = { value = 3 add = 2 }\n",
        "common/script_values/00_x.txt",
    );
    let by_kind: Vec<(&str, KindId)> = f.defs.iter().map(|d| (d.name.as_str(), d.kind)).collect();
    assert_eq!(
        by_kind,
        vec![
            ("minor_stress_gain", pdxl_ck3::kinds::SCRIPT_VALUE),
            ("my_formula", pdxl_ck3::kinds::SCRIPT_VALUE),
        ]
    );
}

#[test]
fn script_value_refs_in_value_positions() {
    let src = "e = {\n\
         \tadd_stress = minor_stress_gain\n\
         \tadd_gold = { value = my_formula multiply = 2 }\n\
         \tadd_prestige = { minor_stress_gain another_value }\n\
         \tminor_stress_gain = 5\n\
         }\n";
    let f = extract_with_names(
        src,
        "events/x.txt",
        &[],
        &[],
        &["minor_stress_gain", "my_formula", "another_value"],
    );
    let names: Vec<(&str, KindId)> = f.calls.iter().map(|c| (c.name.as_str(), c.kind)).collect();
    // The scalar value, the nested `value =`, and both list items — but NOT the
    // `minor_stress_gain = 5` KEY (a key is never a script-value reference).
    assert_eq!(
        names,
        vec![
            ("minor_stress_gain", pdxl_ck3::kinds::SCRIPT_VALUE),
            ("my_formula", pdxl_ck3::kinds::SCRIPT_VALUE),
            ("minor_stress_gain", pdxl_ck3::kinds::SCRIPT_VALUE),
            ("another_value", pdxl_ck3::kinds::SCRIPT_VALUE),
        ]
    );
}

#[test]
fn unknown_keys_are_not_calls() {
    let f = extract_with_calls(
        "x = { immediate = { not_scripted = yes } }",
        "events/test.txt",
        &["my_effect"],
        &[],
    );
    assert!(call_names(&f).is_empty());
}

// ── definitions (ported from validate_test.go) ──────────────────────────────

#[test]
fn collects_definitions_including_namespace() {
    let src = "namespace = test\ntest.0001 = { type = character_event }\n";
    let f = extract(src, "events/test_events.txt");
    // `namespace = test` declares a Namespace named `test` (its value); the
    // event is a separate Event definition.
    assert_eq!(def_names(&f), vec!["test", "test.0001"]);
    assert_eq!(f.defs[0].kind, pdxl_ck3::kinds::NAMESPACE);
    assert_eq!(f.defs[1].kind, pdxl_ck3::kinds::EVENT);
    // The namespace symbol points at the value, so hover/doc land on the name.
    let d = &f.defs[0];
    assert_eq!(&src[d.offset as usize..d.end_offset as usize], "test");
}

#[test]
fn collects_multiple_triggers() {
    let f = extract(
        "alpha_trigger = { always = yes }\nbeta_trigger = { always = no }\n",
        "common/scripted_triggers/00_t.txt",
    );
    assert_eq!(def_names(&f), vec!["alpha_trigger", "beta_trigger"]);
    assert!(
        f.defs
            .iter()
            .all(|s| s.kind == pdxl_ck3::kinds::SCRIPTED_TRIGGER)
    );
}

#[test]
fn collects_faiths_nested_under_religion_type() {
    let f = extract(
        "religion_fire = { faiths = { sun_spirituality = { } fire_lord_cult = { } } }\n",
        "common/religion/religion_types/fire.txt",
    );
    assert_eq!(def_names(&f), vec!["sun_spirituality", "fire_lord_cult"]);
    assert!(f.defs.iter().all(|d| d.kind == pdxl_ck3::kinds::FAITH));
}

#[test]
fn collects_characters_including_dotted_ids() {
    let f = extract(
        "145665 = { name = \"Foo\" }\nbohemia.1 = { name = \"Bar\" }\n",
        "history/characters/afar.txt",
    );
    assert_eq!(def_names(&f), vec!["145665", "bohemia.1"]);
    assert!(f.defs.iter().all(|s| s.kind == pdxl_ck3::kinds::CHARACTER));
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
    assert!(f.aliases.iter().all(|a| a.kind == pdxl_ck3::kinds::TRAIT));
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
    assert!(f.refs.iter().all(|r| r.kind == pdxl_ck3::kinds::TRAIT));
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
    assert!(f.refs.iter().all(|r| r.kind == pdxl_ck3::kinds::EVENT));
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

#[test]
fn modifier_block_and_scalar_refs() {
    let f = extract(
        "e = {\n\
         \tadd_scheme_modifier = { type = massive_success_modifier days = 360 }\n\
         \tadd_character_modifier = { modifier = brave_modifier years = 5 }\n\
         \tadd_artifact_modifier = shiny_modifier\n\
         \tsome_weight = { modifier = ignored }\n\
         \ttype = also_ignored\n\
         }\n",
        "events/x.txt",
    );
    assert_eq!(
        ref_names(&f),
        vec![
            "massive_success_modifier",
            "brave_modifier",
            "shiny_modifier"
        ],
    );
    assert!(f.refs.iter().all(|r| r.kind == pdxl_ck3::kinds::MODIFIER));
    // A bare `modifier =` / `type =` outside an add-key block is not a ref.
}

#[test]
fn character_interaction_def_and_ref() {
    let d = extract(
        "my_interaction = { category = interaction_category_hostile }\n",
        "common/character_interactions/00.txt",
    );
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::CHARACTER_INTERACTION);

    // `interaction = X` resolves anywhere (an important-action, effect, …).
    let r = extract(
        "e = { open_interaction_window = { interaction = my_interaction } }\n",
        "common/important_actions/x.txt",
    );
    assert_eq!(ref_names(&r), vec!["my_interaction"]);
    assert_eq!(r.refs[0].kind, pdxl_ck3::kinds::CHARACTER_INTERACTION);

    // The interaction's `desc` / `notification_text` are loc-key references
    // (so go-to-definition jumps to the .yml), but only inside interactions.
    let loc = extract(
        "my_interaction = { desc = my_interaction_desc category = c }\n",
        "common/character_interactions/00.txt",
    );
    let desc = loc
        .refs
        .iter()
        .find(|r| r.name == "my_interaction_desc")
        .expect("desc loc ref");
    assert_eq!(desc.kind, pdxl_ck3::kinds::LOC_KEY);
    // `desc` outside interactions/events/decisions is not a loc ref.
    let elsewhere = extract("x = { desc = whatever }\n", "common/traits/00.txt");
    assert!(elsewhere.refs.iter().all(|r| r.name != "whatever"));
}

#[test]
fn secret_type_def_and_gated_type_ref() {
    // Def in common/secret_types/.
    let d = extract(
        "secret_deviant = { category = deviancy }\n",
        "common/secret_types/00.txt",
    );
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::SECRET_TYPE);

    // `type = X` inside a secret effect references a secret …
    let f = extract(
        "e = { add_secret = { type = secret_deviant secret_owner = root } }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["secret_deviant"]);
    assert_eq!(f.refs[0].kind, pdxl_ck3::kinds::SECRET_TYPE);

    // … but a bare `type = character_event` (event) is not a secret ref.
    let ev = extract("test.1 = { type = character_event }\n", "events/x.txt");
    assert!(ev.refs.is_empty());
}

#[test]
fn character_template_ref_gated_to_create_character() {
    // `create_character = { template = X }` references a character template …
    let f = extract(
        "e = { create_character = { template = my_char save_scope_as = x } }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["my_char"]);
    assert_eq!(f.refs[0].kind, pdxl_ck3::kinds::SCRIPTED_CHARACTER_TEMPLATE);

    // … while `create_artifact = { template = X }` is an *artifact* template.
    let art = extract(
        "e = { create_artifact = { template = regalia_template } }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&art), vec!["regalia_template"]);
    assert_eq!(art.refs[0].kind, pdxl_ck3::kinds::ARTIFACT_TEMPLATE);
}

#[test]
fn history_character_body_refs() {
    // A character body's attribute fields reference traits, culture, faith
    // (via `religion` or `faith`), dynasty/house, and parent characters —
    // all gated to history/characters/.
    let f = extract(
        "20816 = {\n\
         \tname = \"Bilal\"\n\
         \tdynasty = 101046\n\
         \tdynasty_house = house_chiny\n\
         \treligion = muwalladi\n\
         \tculture = andalusian\n\
         \ttrait = gluttonous\n\
         \tfather = 20800\n\
         \tmother = 20801\n\
         \t1039.1.1 = { birth = \"1039.1.1\" trait = brave add_spouse = 20900 }\n\
         }\n",
        "history/characters/andalusian.txt",
    );
    assert_eq!(f.defs.len(), 1);
    assert_eq!(f.defs[0].kind, pdxl_ck3::kinds::CHARACTER);
    let by_kind: Vec<(&str, pdxl_analysis::KindId)> =
        f.refs.iter().map(|r| (r.name.as_str(), r.kind)).collect();
    // Source-walk order.
    assert_eq!(
        by_kind,
        vec![
            ("101046", pdxl_ck3::kinds::DYNASTY),
            ("house_chiny", pdxl_ck3::kinds::DYNASTY_HOUSE),
            ("muwalladi", pdxl_ck3::kinds::FAITH),
            ("andalusian", pdxl_ck3::kinds::CULTURE),
            ("gluttonous", pdxl_ck3::kinds::TRAIT),
            ("20800", pdxl_ck3::kinds::CHARACTER),
            ("20801", pdxl_ck3::kinds::CHARACTER),
            ("brave", pdxl_ck3::kinds::TRAIT),
            ("20900", pdxl_ck3::kinds::CHARACTER),
        ]
    );

    // … but the same keys mean nothing outside history/characters/.
    let elsewhere = extract(
        "e = { trait = gluttonous culture = andalusian father = 20800 }\n",
        "events/x.txt",
    );
    assert!(ref_names(&elsewhere).is_empty());
}

#[test]
fn nickname_defs_and_refs() {
    let d = extract(
        "nick_the_bald = { is_bad = yes }\nnick_bluetooth = {}\n",
        "common/nicknames/00.txt",
    );
    assert_eq!(def_names(&d), vec!["nick_the_bald", "nick_bluetooth"]);
    assert!(d.defs.iter().all(|s| s.kind == pdxl_ck3::kinds::NICKNAME));

    // `give_nickname` (effect) and `has_nickname` (trigger) resolve anywhere.
    let f = extract(
        "e = {\n\
         \tgive_nickname = nick_the_bald\n\
         \ttrigger = { has_nickname = nick_bluetooth }\n\
         }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["nick_the_bald", "nick_bluetooth"]);
    assert!(f.refs.iter().all(|r| r.kind == pdxl_ck3::kinds::NICKNAME));
}

#[test]
fn dynasty_defs_and_refs() {
    let d = extract(
        "101046 = { name = \"dynn_X\" culture = \"andalusian\" }\n",
        "common/dynasties/00.txt",
    );
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::DYNASTY);
    // The dynasty's `culture` attribute is a culture ref (quotes stripped).
    assert_eq!(ref_names(&d), vec!["andalusian"]);
    assert_eq!(d.refs[0].kind, pdxl_ck3::kinds::CULTURE);

    let h = extract(
        "house_chiny = { name = \"dynn_Chiny\" dynasty = 25061 }\n",
        "common/dynasty_houses/00.txt",
    );
    assert_eq!(h.defs[0].kind, pdxl_ck3::kinds::DYNASTY_HOUSE);
    assert_eq!(ref_names(&h), vec!["25061"]);
    assert_eq!(h.refs[0].kind, pdxl_ck3::kinds::DYNASTY);
}

#[test]
fn casus_belli_defs_and_refs() {
    // Defs in both CB dirs, each with its own kind.
    let d = extract(
        "claim_cb = { group = claim }\n",
        "common/casus_belli_types/00.txt",
    );
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::CASUS_BELLI);
    // … and the `group =` field references a CB group (gated to this dir).
    assert_eq!(ref_names(&d), vec!["claim"]);
    assert_eq!(d.refs[0].kind, pdxl_ck3::kinds::CASUS_BELLI_GROUP);

    let g = extract(
        "claim = { allowed_for_character = { } }\n",
        "common/casus_belli_groups/00.txt",
    );
    assert_eq!(g.defs[0].kind, pdxl_ck3::kinds::CASUS_BELLI_GROUP);

    // `casus_belli`, `cb` (scalar and list) and `using_cb` resolve anywhere.
    let f = extract(
        "e = {\n\
         \tstart_war = { casus_belli = claim_cb target = scope:t }\n\
         \thas_cb_on = { target = scope:t cb = conquest_cb }\n\
         \tany_character_war = { using_cb = religious_war }\n\
         \tai_start_best_war = { cb = { claim_cb conquest_cb } }\n\
         }\n",
        "events/x.txt",
    );
    assert_eq!(
        ref_names(&f),
        vec![
            "claim_cb",
            "conquest_cb",
            "religious_war",
            "claim_cb",
            "conquest_cb"
        ]
    );
    assert!(
        f.refs
            .iter()
            .all(|r| r.kind == pdxl_ck3::kinds::CASUS_BELLI)
    );

    // … but `group =` outside the CB-types dir means nothing …
    let elsewhere = extract("e = { group = claim }\n", "events/x.txt");
    assert!(elsewhere.refs.is_empty());

    // … and a nested `group` (static_group_filter, inside a trigger block)
    // is not a CB-group ref even inside the CB-types dir (KeyValueTop).
    let nested = extract(
        "war = { valid_to_start = { static_group_filter = { group = other } } }\n",
        "common/casus_belli_types/00.txt",
    );
    assert!(ref_names(&nested).is_empty(), "{:?}", ref_names(&nested));
}

#[test]
fn artifact_defs_and_refs() {
    // Each artifacts subdirectory yields its own kind of def.
    for (src, dir, kind) in [
        (
            "helmet = { slot = helmet }\n",
            "common/artifacts/types/00.txt",
            pdxl_ck3::kinds::ARTIFACT_TYPE,
        ),
        (
            "general_unique_template = { unique = yes }\n",
            "common/artifacts/templates/00.txt",
            pdxl_ck3::kinds::ARTIFACT_TEMPLATE,
        ),
        (
            "spear = { icon = \"spear.dds\" }\n",
            "common/artifacts/visuals/00.txt",
            pdxl_ck3::kinds::ARTIFACT_VISUAL,
        ),
        (
            "decoration_pattern_wolf = { group = decoration_pattern }\n",
            "common/artifacts/features/00.txt",
            pdxl_ck3::kinds::ARTIFACT_FEATURE,
        ),
        (
            "decoration_pattern = {}\n",
            "common/artifacts/feature_groups/00.txt",
            pdxl_ck3::kinds::ARTIFACT_FEATURE_GROUP,
        ),
        (
            "reforge_spear = { in_type = spear }\n",
            "common/artifacts/blueprints/00.txt",
            pdxl_ck3::kinds::ARTIFACT_BLUEPRINT,
        ),
        (
            "crown = { type = \"helmet\" category = inventory }\n",
            "common/artifacts/slots/00.txt",
            pdxl_ck3::kinds::ARTIFACT_SLOT,
        ),
    ] {
        let d = extract(src, dir);
        assert_eq!(d.defs.len(), 1, "one def expected in {dir}");
        assert_eq!(d.defs[0].kind, kind, "kind mismatch in {dir}");
    }

    // create_artifact direct-child fields reference type/visuals/template;
    // the nested history `type` is a different `type` and must NOT resolve.
    let f = extract(
        "e = { create_artifact = {\n\
         \ttype = sword\n\
         \tvisuals = easteregg_radzig_sword\n\
         \ttemplate = general_unique_template\n\
         \thistory = { type = created_before_history }\n\
         } }\n",
        "common/scripted_effects/x.txt",
    );
    // (kind-registration order: type, template, visual)
    assert_eq!(
        ref_names(&f),
        vec!["sword", "general_unique_template", "easteregg_radzig_sword"]
    );
    assert_eq!(f.refs[0].kind, pdxl_ck3::kinds::ARTIFACT_TYPE);
    assert_eq!(f.refs[1].kind, pdxl_ck3::kinds::ARTIFACT_TEMPLATE);
    assert_eq!(f.refs[2].kind, pdxl_ck3::kinds::ARTIFACT_VISUAL);

    // Blueprint fields reference types/visuals, gated to the blueprints dir.
    let bp = extract(
        "reforge_spear = {\n\
         \tin_type = spear\n\
         \tout_type = wall_big\n\
         \tin_visuals = spear\n\
         \tout_visuals = spear\n\
         }\n",
        "common/artifacts/blueprints/00.txt",
    );
    assert_eq!(
        bp.refs
            .iter()
            .map(|r| r.kind)
            .collect::<std::collections::HashSet<_>>(),
        [
            pdxl_ck3::kinds::ARTIFACT_TYPE,
            pdxl_ck3::kinds::ARTIFACT_VISUAL
        ]
        .into_iter()
        .collect()
    );
    // … but outside the blueprints dir the same keys mean nothing.
    let elsewhere = extract("e = { in_type = spear }\n", "events/x.txt");
    assert!(elsewhere.refs.is_empty());

    // Feature `group` and type `required_features` items → feature groups.
    let feat = extract(
        "decoration_pattern_wolf = { group = decoration_pattern }\n",
        "common/artifacts/features/00.txt",
    );
    assert_eq!(ref_names(&feat), vec!["decoration_pattern"]);
    assert_eq!(feat.refs[0].kind, pdxl_ck3::kinds::ARTIFACT_FEATURE_GROUP);

    let ty = extract(
        "helmet = {\n\
         \tslot = helmet\n\
         \trequired_features = { crown_decoration decoration_material_wire }\n\
         \toptional_features = { decoration_material_gem }\n\
         }\n",
        "common/artifacts/types/00.txt",
    );
    assert_eq!(
        ref_names(&ty),
        vec![
            "crown_decoration",
            "decoration_material_wire",
            "decoration_material_gem"
        ]
    );
    assert!(
        ty.refs
            .iter()
            .all(|r| r.kind == pdxl_ck3::kinds::ARTIFACT_FEATURE_GROUP)
    );
}

#[test]
fn trait_xp_block_references_trait() {
    let f = extract(
        "e = {\n\
         \tadd_trait_xp = { trait = brave value = 3 }\n\
         \thas_trait_xp = { trait = craven value >= 100 }\n\
         }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["brave", "craven"]);
    assert!(f.refs.iter().all(|r| r.kind == pdxl_ck3::kinds::TRAIT));
}

#[test]
fn portrait_animation_defs_and_gated_refs() {
    // Defs from both directories share the kind.
    let pa = extract(
        "happiness = { male = { } }\n",
        "gfx/portraits/portrait_animations/a.txt",
    );
    assert_eq!(pa.defs[0].kind, pdxl_ck3::kinds::PORTRAIT_ANIMATION);
    let sa = extract("bow_closed = { }\n", "common/scripted_animations/a.txt");
    assert_eq!(sa.defs[0].kind, pdxl_ck3::kinds::PORTRAIT_ANIMATION);

    // `animation = X` is a reference under events/ …
    let ev = extract(
        "e = { right_portrait = { animation = happiness } }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&ev), vec!["happiness"]);
    assert_eq!(ev.refs[0].kind, pdxl_ck3::kinds::PORTRAIT_ANIMATION);

    // … but not elsewhere (tutorial `animation = center` is a camera position).
    let tut = extract(
        "l = { animation = center }\n",
        "common/tutorial_lessons/x.txt",
    );
    assert!(tut.refs.is_empty());
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
fn ref_carries_file_and_offset() {
    // The CLI derives `file:line:col` from these; here `brave` begins at byte
    // 24 (line 2, col 19 in the old precomputed form).
    let f = extract("x = 1\ne = { add_trait = brave }\n", "events/dir/x.txt");
    assert_eq!(&*f.refs[0].file, "events/dir/x.txt");
    assert_eq!(f.refs[0].start, 24);
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
    assert!(f.defs.iter().all(|d| d.kind == pdxl_ck3::kinds::TITLE));
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
    assert!(
        f.defs
            .iter()
            .all(|d| d.kind == pdxl_ck3::kinds::SCRIPTED_EFFECT)
    );
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
    let names: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::TITLE)
        .map(|r| r.name.as_str())
        .collect();
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
    let r = f
        .refs
        .iter()
        .find(|r| r.kind == pdxl_ck3::kinds::TITLE)
        .unwrap();
    assert_eq!(&src[r.start as usize..r.end as usize], "e_empire");
    // The range starts at the name, not the `title:` prefix (byte 10 = col 11).
    assert_eq!(r.start, 10);
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
        f.refs.iter().all(|r| r.kind != pdxl_ck3::kinds::TITLE),
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

    assert_eq!(table.count(pdxl_ck3::kinds::TITLE), 7);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert!(
        diags[0].msg.contains("unknown title \"d_gone\""),
        "{}",
        diags[0].msg
    );
}

// ── gated capital → title refs (ANALYSIS_VERSION 3) ─────────────────────────

#[test]
fn capital_in_landed_titles_is_a_title_ref() {
    let f = extract(
        "k_kingdom = {\n\tcapital = c_shore\n\tc_shore = { b_port = { province = 1 } }\n}\n",
        "common/landed_titles/00.txt",
    );
    let r = f
        .refs
        .iter()
        .find(|r| r.name == "c_shore")
        .expect("capital value extracted as a ref");
    assert_eq!(r.kind, pdxl_ck3::kinds::TITLE);
}

#[test]
fn capital_outside_landed_titles_is_not_a_ref() {
    // `capital` elsewhere (events, effects, history) means other things —
    // the rule is gated to the landed-titles directory.
    let f = extract(
        "e = {\n\tcapital = c_shore\n}\n",
        "common/scripted_effects/e.txt",
    );
    assert!(
        f.refs.iter().all(|r| r.name != "c_shore"),
        "gated rule must not fire outside common/landed_titles/: {:?}",
        ref_names(&f)
    );
}

// ── full on_action reference set (ANALYSIS_VERSION 5) ───────────────────────

#[test]
fn on_action_fire_lists_fallback_and_weighted() {
    let f = extract(
        "my_oa = {\n\
         \tfirst_valid_on_action = { oa_a oa_b }\n\
         \trandom_on_actions = { 100 = oa_c 50 = 0 chance = 10 }\n\
         \tfallback = oa_d\n\
         }\n",
        "common/on_action/oa.txt",
    );
    assert_eq!(ref_names(&f), vec!["oa_a", "oa_b", "oa_c", "oa_d"]);
    assert!(f.refs.iter().all(|r| r.kind == pdxl_ck3::kinds::ON_ACTION));
}

#[test]
fn on_action_rules_are_gated_to_on_action_files() {
    // `fallback = yes` on an event option must not become an on_action ref;
    // neither may list keys outside common/on_action/.
    let f = extract(
        "ns.1 = {\n\
         \toption = { name = a fallback = yes }\n\
         \tfirst_valid_on_action = { oa_a }\n\
         }\n",
        "events/e.txt",
    );
    assert!(
        f.refs.iter().all(|r| r.kind != pdxl_ck3::kinds::ON_ACTION),
        "gated rules fired outside on_action: {:?}",
        ref_names(&f)
    );
}

#[test]
fn trigger_event_block_can_fire_an_on_action_anywhere() {
    let f = extract(
        "e = {\n\
         \ttrigger_event = { on_action = my_oa }\n\
         \ttrigger_event = { id = ns.1 }\n\
         }\n",
        "common/scripted_effects/e.txt",
    );
    let oa = f
        .refs
        .iter()
        .find(|r| r.name == "my_oa")
        .expect("on_action ref");
    assert_eq!(oa.kind, pdxl_ck3::kinds::ON_ACTION);
    let ev = f.refs.iter().find(|r| r.name == "ns.1").expect("event ref");
    assert_eq!(ev.kind, pdxl_ck3::kinds::EVENT);
}

// ── localization-key references (ANALYSIS_VERSION 6) ────────────────────────

#[test]
fn event_text_fields_are_loc_refs() {
    let f = extract(
        "ns.1 = {\n\
         \ttitle = ns.1.t\n\
         \tdesc = {\n\t\tfirst_valid = {\n\t\t\ttriggered_desc = {\n\t\t\t\tdesc = ns.1.desc_alt\n\t\t\t}\n\t\t}\n\t}\n\
         \toption = { name = ns.1.a custom_tooltip = ns.1.a.tt }\n\
         }\n",
        "events/e.txt",
    );
    let loc: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::LOC_KEY)
        .map(|r| r.name.as_str())
        .collect();
    // Scalar forms extracted at any depth; the desc BLOCK itself is not a ref.
    assert_eq!(loc, vec!["ns.1.t", "ns.1.desc_alt", "ns.1.a", "ns.1.a.tt"]);
}

#[test]
fn loc_ref_rules_are_gated_by_directory() {
    // `desc`/`name` mean other things outside events/ and decisions/.
    let f = extract(
        "v = {\n\tdesc = some_svalue_desc\n\tname = whatever\n}\n",
        "common/script_values/v.txt",
    );
    assert!(f.refs.iter().all(|r| r.kind != pdxl_ck3::kinds::LOC_KEY));

    // …while decisions expose their own text fields.
    let f = extract(
        "d = {\n\tselection_tooltip = d.tooltip\n\tconfirm_text = d.confirm\n}\n",
        "common/decisions/d.txt",
    );
    let loc: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::LOC_KEY)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(loc, vec!["d.tooltip", "d.confirm"]);
}

// ── laws (ANALYSIS_VERSION 7) ───────────────────────────────────────────────

#[test]
fn law_group_children_are_laws_minus_attributes() {
    let f = extract(
        "@cooldown = 20\n\
         crown_authority = {\n\
         \tdefault = crown_authority_1\n\
         \tcumulative = yes\n\
         \tflag = realm_law\n\
         \tcan_change_law_group = { always = yes }\n\
         \tcrown_authority_0 = { modifier = { x = 1 } }\n\
         \tcrown_authority_1 = { }\n\
         }\n",
        "common/laws/00_realm_laws.txt",
    );
    // Laws are the block children; scalar attrs and can_change_law_group and
    // the @var are excluded. The group itself is not a symbol.
    assert_eq!(
        def_names(&f),
        vec!["crown_authority_0", "crown_authority_1"]
    );
    assert!(f.defs.iter().all(|s| s.kind == pdxl_ck3::kinds::LAW));
}

#[test]
fn realm_law_references_and_gated_default() {
    let f = extract(
        "e = {\n\
         \thas_realm_law = crown_authority_2\n\
         \tadd_realm_law = crown_authority_1\n\
         \tadd_realm_law_skip_effects = tribal_authority_0\n\
         \tremove_realm_law = x_law\n\
         \thas_realm_law_flag = uses_crown_authority\n\
         \thas_realm_law_in_group = crown_authority\n\
         }\n",
        "common/scripted_effects/e.txt",
    );
    let laws: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::LAW)
        .map(|r| r.name.as_str())
        .collect();
    // Exact-key rules: the _flag / _in_group variants are NOT law refs.
    assert_eq!(
        laws,
        vec![
            "crown_authority_2",
            "crown_authority_1",
            "tribal_authority_0",
            "x_law"
        ]
    );

    // `default = law_name` resolves only inside common/laws/…
    let f = extract(
        "grp = { default = the_law the_law = { } }\n",
        "common/laws/00_realm_laws.txt",
    );
    assert!(
        f.refs
            .iter()
            .any(|r| r.kind == pdxl_ck3::kinds::LAW && r.name == "the_law")
    );
    // …not elsewhere (`default` means other things).
    let f = extract("d = { default = something }\n", "common/decisions/d.txt");
    assert!(f.refs.iter().all(|r| r.kind != pdxl_ck3::kinds::LAW));
}
