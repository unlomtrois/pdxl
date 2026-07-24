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
fn game_concept_alias_list_becomes_aliases() {
    let f = extract(
        "vassal = {\n\
         \talias = { vassals vassalize vassalage }\n\
         \ttexture = \"x.dds\"\n\
         }\n",
        "common/game_concepts/00.txt",
    );
    // The def itself.
    assert_eq!(def_names(&f), vec!["vassal"]);
    assert!(f.defs[0].kind == pdxl_ck3::kinds::GAME_CONCEPT);
    // Every list item is a resolvable alias name for the concept.
    let names: Vec<&str> = f.aliases.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, vec!["vassals", "vassalize", "vassalage"]);
    assert!(
        f.aliases
            .iter()
            .all(|a| a.kind == pdxl_ck3::kinds::GAME_CONCEPT)
    );
    // Same Go-parity offset quirk as scalar aliases.
    assert_eq!(f.aliases[0].end_offset, f.aliases[0].offset);
}

#[test]
fn game_concept_parent_ref_is_gated() {
    let f = extract(
        "direct_vassal = {\n\
         \tparent = vassal\n\
         }\nvassal = { }\n",
        "common/game_concepts/00.txt",
    );
    assert_eq!(ref_names(&f), vec!["vassal"]);
    assert!(f.refs[0].kind == pdxl_ck3::kinds::GAME_CONCEPT);

    // `parent` is a common key elsewhere (gui, culture eras) — not a concept
    // ref outside the game_concepts directory.
    let elsewhere = extract("x = { parent = vassal }\n", "common/culture/eras/00.txt");
    assert!(ref_names(&elsewhere).is_empty());
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
fn death_reason_defs_and_refs() {
    let d = extract(
        "death_murder = { icon = \"death_murder.dds\" }\n\
         death_duel = { public_knowledge = yes use_equipped_artifact_in_slot = weapon }\n",
        "common/deathreasons/00.txt",
    );
    assert_eq!(def_names(&d), vec!["death_murder", "death_duel"]);
    assert!(
        d.defs
            .iter()
            .all(|s| s.kind == pdxl_ck3::kinds::DEATH_REASON)
    );
    // … and the slot key inside a death reason references an artifact slot.
    assert_eq!(ref_names(&d), vec!["weapon"]);
    assert_eq!(d.refs[0].kind, pdxl_ck3::kinds::ARTIFACT_SLOT);

    // `death_reason` resolves anywhere: the death effect and history blocks.
    let f = extract(
        "e = { death = { death_reason = death_murder killer = scope:killer } }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["death_murder"]);
    assert_eq!(f.refs[0].kind, pdxl_ck3::kinds::DEATH_REASON);

    // In history, `killer` is a character reference too.
    let h = extract(
        "1 = { 1089.1.1 = { death = { death_reason = death_murder killer = 20816 } } }\n",
        "history/characters/x.txt",
    );
    let kinds: Vec<(&str, pdxl_analysis::KindId)> =
        h.refs.iter().map(|r| (r.name.as_str(), r.kind)).collect();
    assert!(kinds.contains(&("death_murder", pdxl_ck3::kinds::DEATH_REASON)));
    assert!(kinds.contains(&("20816", pdxl_ck3::kinds::CHARACTER)));
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

// ── culture domain (ANALYSIS_VERSION 30) ────────────────────────────────────

#[test]
fn culture_pillar_defs_and_refs() {
    // Defs in common/culture/pillars/.
    let d = extract(
        "ethos_bellicose = { type = ethos }\nlanguage_norse = { type = language }\n",
        "common/culture/pillars/00.txt",
    );
    assert_eq!(def_names(&d), vec!["ethos_bellicose", "language_norse"]);
    assert!(
        d.defs
            .iter()
            .all(|s| s.kind == pdxl_ck3::kinds::CULTURE_PILLAR)
    );

    // The five pillar slots of a culture body reference pillars (depth 1).
    let f = extract(
        "norse = {\n\
         \tethos = ethos_bellicose\n\
         \theritage = heritage_north_germanic\n\
         \tlanguage = language_norse\n\
         \tmartial_custom = martial_custom_male_only\n\
         \thead_determination = head_determination_domain\n\
         }\n",
        "common/culture/cultures/00.txt",
    );
    let pillars: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::CULTURE_PILLAR)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(
        pillars,
        vec![
            "ethos_bellicose",
            "heritage_north_germanic",
            "language_norse",
            "martial_custom_male_only",
            "head_determination_domain"
        ]
    );

    // `has_cultural_pillar` (trigger) and `culture_pillar:` literals resolve
    // anywhere.
    let t = extract(
        "e = {\n\
         \thas_cultural_pillar = heritage_north_germanic\n\
         \tsave_temporary_scope_value_as = { name = x value = culture_pillar:language_norse }\n\
         }\n",
        "events/x.txt",
    );
    let names: Vec<&str> = t
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::CULTURE_PILLAR)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, vec!["heritage_north_germanic", "language_norse"]);

    // `ethos =` outside common/culture/cultures/ means nothing …
    let elsewhere = extract("e = { ethos = ethos_bellicose }\n", "events/x.txt");
    assert!(elsewhere.refs.is_empty());
    // … and a nested (non-depth-1) `language =` is not a ref (KeyValueTop).
    let nested = extract(
        "norse = { dlc = { language = language_norse } }\n",
        "common/culture/cultures/00.txt",
    );
    assert!(
        nested
            .refs
            .iter()
            .all(|r| r.kind != pdxl_ck3::kinds::CULTURE_PILLAR),
        "{:?}",
        ref_names(&nested)
    );
}

#[test]
fn culture_tradition_defs_and_refs() {
    let d = extract(
        "tradition_seafaring = { category = realm }\n",
        "common/culture/traditions/00.txt",
    );
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::CULTURE_TRADITION);

    // The culture body's tradition list and dlc_tradition trait/fallback.
    let f = extract(
        "norse = {\n\
         \ttraditions = { tradition_seafaring tradition_runestones }\n\
         \tdlc_tradition = {\n\
         \t\ttrait = tradition_northern_stories\n\
         \t\trequires_dlc_flag = the_northern_lords\n\
         \t\tfallback = tradition_poetry\n\
         \t}\n\
         }\n",
        "common/culture/cultures/00.txt",
    );
    let names: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::CULTURE_TRADITION)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "tradition_seafaring",
            "tradition_runestones",
            "tradition_northern_stories",
            "tradition_poetry"
        ]
    );

    // Trigger, effects and scope literal resolve anywhere; the scope-object
    // comparison form (`has_cultural_tradition = prev`) is skipped.
    let t = extract(
        "e = {\n\
         \thas_cultural_tradition = tradition_seafaring\n\
         \thas_cultural_tradition = prev\n\
         \tadd_culture_tradition = tradition_runestones\n\
         \tremove_culture_tradition = tradition_poetry\n\
         \texists = culture_tradition:tradition_seafaring\n\
         }\n",
        "events/x.txt",
    );
    let names: Vec<&str> = t
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::CULTURE_TRADITION)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "tradition_seafaring",
            "tradition_runestones",
            "tradition_poetry",
            "tradition_seafaring"
        ]
    );

    // `traditions = { … }` outside common/culture/cultures/ is not a ref.
    let elsewhere = extract(
        "e = { traditions = { tradition_seafaring } }\n",
        "events/x.txt",
    );
    assert!(elsewhere.refs.is_empty());
}

#[test]
fn culture_era_defs_and_gated_refs() {
    let d = extract(
        "culture_era_tribal = { year = 500 }\n",
        "common/culture/eras/00.txt",
    );
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::CULTURE_ERA);

    // An innovation names its era, its unlocked CB, and its unlocked law —
    // all gated to common/culture/innovations/.
    let f = extract(
        "innovation_motte = {\n\
         \tculture_era = culture_era_tribal\n\
         \tunlock_casus_belli = claim_cb\n\
         \tunlock_law = crown_authority_1\n\
         }\n",
        "common/culture/innovations/00.txt",
    );
    let by_kind: Vec<(&str, KindId)> = f.refs.iter().map(|r| (r.name.as_str(), r.kind)).collect();
    assert_eq!(
        by_kind,
        vec![
            ("culture_era_tribal", pdxl_ck3::kinds::CULTURE_ERA),
            ("claim_cb", pdxl_ck3::kinds::CASUS_BELLI),
            ("crown_authority_1", pdxl_ck3::kinds::LAW),
        ]
    );

    // The same keys outside innovations/ mean nothing (eras/ included:
    // corpus-validated zero occurrences there).
    let elsewhere = extract(
        "x = {\n\
         \tculture_era = culture_era_tribal\n\
         \tunlock_casus_belli = claim_cb\n\
         \tunlock_law = crown_authority_1\n\
         }\n",
        "common/culture/eras/00.txt",
    );
    assert!(elsewhere.refs.is_empty(), "{:?}", ref_names(&elsewhere));
}

#[test]
fn culture_innovation_defs_and_refs() {
    let d = extract(
        "innovation_motte = { culture_era = culture_era_tribal group = culture_group_military }\n",
        "common/culture/innovations/00.txt",
    );
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::CULTURE_INNOVATION);

    // `has_innovation` (trigger) and `culture_innovation:` resolve anywhere.
    let f = extract(
        "e = {\n\
         \thas_innovation = innovation_motte\n\
         \texists = culture_innovation:innovation_longboats\n\
         }\n",
        "events/x.txt",
    );
    let names: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::CULTURE_INNOVATION)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, vec!["innovation_motte", "innovation_longboats"]);
}

#[test]
fn name_list_defs_and_refs() {
    let d = extract(
        "name_list_norse = { always_use_patronym = yes }\n",
        "common/culture/name_lists/00.txt",
    );
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::NAME_LIST);

    // `name_list = X` is ungated (corpus-validated as never overloaded): it
    // fires in culture bodies and aesthetics bundles alike.
    let c = extract(
        "norse = { name_list = name_list_norse }\n",
        "common/culture/cultures/00.txt",
    );
    let names: Vec<&str> = c
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::NAME_LIST)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, vec!["name_list_norse"]);
    let b = extract(
        "aesthetics_norwegian = { name_list = name_list_norse }\n",
        "common/culture/aesthetics_bundles/00.txt",
    );
    assert_eq!(ref_names(&b), vec!["name_list_norse"]);
    assert_eq!(b.refs[0].kind, pdxl_ck3::kinds::NAME_LIST);
}

#[test]
fn culture_parents_list_references_cultures() {
    let f = extract(
        "norwegian = { parents = { norse west_germanic } }\n",
        "common/culture/cultures/00.txt",
    );
    let names: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::CULTURE)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, vec!["norse", "west_germanic"]);

    // `parents = { … }` outside common/culture/cultures/ is not a ref.
    let elsewhere = extract("e = { parents = { norse } }\n", "events/x.txt");
    assert!(elsewhere.refs.is_empty());
}

#[test]
fn culture_misc_def_only_kinds() {
    // Aesthetics bundles, creation names, and name equivalencies are
    // definitions only — nothing in script names them by key.
    let b = extract(
        "aesthetics_norwegian = { is_shown = { } }\n",
        "common/culture/aesthetics_bundles/00.txt",
    );
    assert_eq!(b.defs[0].kind, pdxl_ck3::kinds::AESTHETICS_BUNDLE);

    let c = extract(
        "scanian = { trigger = { always = yes } hybrid = yes }\n",
        "common/culture/creation_names/00.txt",
    );
    assert_eq!(c.defs[0].kind, pdxl_ck3::kinds::CULTURE_CREATION_NAME);

    // Equivalency bodies are loose lists of unquoted name tokens; the items
    // are not references.
    let e = extract(
        "aaron_male = { Aaron AarO_n Haroun Harun }\n",
        "common/culture/name_equivalency/00.txt",
    );
    assert_eq!(def_names(&e), vec!["aaron_male"]);
    assert_eq!(e.defs[0].kind, pdxl_ck3::kinds::NAME_EQUIVALENCY);
    assert!(e.refs.is_empty());
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
fn scope_ref_skips_macro_concatenated_name() {
    // `culture_innovation:innovation_$INNOVATION$` — the lexer splits the
    // macro, so the scalar ends right before `$`. The captured prefix is not
    // a resolvable name and must be skipped (T4N silk-road triggers).
    let f = extract(
        "t = { this = culture_innovation:innovation_$INNOVATION$ }\n",
        "common/scripted_triggers/x.txt",
    );
    assert!(
        f.refs.is_empty(),
        "macro-concatenated scope names must not extract: {:?}",
        f.refs.iter().map(|r| r.name.as_str()).collect::<Vec<_>>()
    );
    // … while a plain literal in the same shape still extracts.
    let ok = extract(
        "t = { this = culture_innovation:innovation_camels }\n",
        "common/scripted_triggers/x.txt",
    );
    assert_eq!(ref_names(&ok), vec!["innovation_camels"]);
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
    // Two diags: the missing title, plus the fixture's `province = 1` (no
    // province defs in this two-file miniature project).
    assert_eq!(diags.len(), 2, "{diags:?}");
    assert!(
        diags
            .iter()
            .any(|d| d.msg.contains("unknown title \"d_gone\""))
    );
    assert!(
        diags
            .iter()
            .any(|d| d.msg.contains("unknown province \"1\""))
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

#[test]
fn custom_loc_defs_and_parent_ref() {
    let d = extract(
        "RandomElephantName = {\n\
         \ttype = character\n\
         \trandom_valid = yes\n\
         \ttext = { localization_key = elephant_name_mahmud }\n\
         }\n\
         ElephantNameVariant = {\n\
         \tparent = RandomElephantName\n\
         \tsuffix = \"_FR_Le\"\n\
         }\n",
        "common/customizable_localization/00.txt",
    );
    assert_eq!(
        def_names(&d),
        vec!["RandomElephantName", "ElephantNameVariant"]
    );
    assert!(d.defs.iter().all(|s| s.kind == pdxl_ck3::kinds::CUSTOM_LOC));
    // The variant's `parent` references the parent custom loc …
    assert_eq!(ref_names(&d), vec!["RandomElephantName"]);
    assert_eq!(d.refs[0].kind, pdxl_ck3::kinds::CUSTOM_LOC);
    // … and `localization_key` is deliberately NOT a ref (multi-language keys).

    // `parent` nested deeper, or outside the dir, means nothing.
    let deep = extract(
        "x = { text = { trigger = { parent = RandomElephantName } } }\n",
        "common/customizable_localization/00.txt",
    );
    assert!(ref_names(&deep).is_empty());
    let elsewhere = extract("e = { parent = RandomElephantName }\n", "events/x.txt");
    assert!(ref_names(&elsewhere).is_empty());
}

#[test]
fn building_defs_and_refs() {
    let d = extract(
        "castle_01 = {\n\
         \tconstruction_time = 720\n\
         \tnext_building = castle_02\n\
         }\ncastle_02 = { }\n",
        "common/buildings/00.txt",
    );
    assert_eq!(def_names(&d), vec!["castle_01", "castle_02"]);
    assert!(d.defs.iter().all(|s| s.kind == pdxl_ck3::kinds::BUILDING));
    assert_eq!(ref_names(&d), vec!["castle_02"]);
    assert_eq!(d.refs[0].kind, pdxl_ck3::kinds::BUILDING);

    // Triggers/effects reference buildings anywhere.
    let f = extract(
        "e = {\n\
         \ttrigger = { has_building_or_higher = castle_01 }\n\
         \tadd_building = castle_02\n\
         }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["castle_01", "castle_02"]);

    // Culture innovations unlock buildings (gated to common/culture/).
    let inn = extract(
        "innovation_x = { culture_era = culture_era_tribal unlock_building = castle_01 }\n",
        "common/culture/innovations/00.txt",
    );
    assert!(
        ref_names(&inn).contains(&"castle_01"),
        "{:?}",
        ref_names(&inn)
    );

    // Province history: special buildings and the buildings list.
    let h = extract(
        "100 = {\n\
         \tspecial_building_slot = hadrians_wall_01\n\
         \tbuildings = {\n\t\tcurtain_walls_01\n\t\tmilitary_camps_01\n\t}\n\
         }\n",
        "history/provinces/x.txt",
    );
    // The top-level key is a province ref; the rest are building refs.
    assert_eq!(
        ref_names(&h),
        vec![
            "100",
            "hadrians_wall_01",
            "curtain_walls_01",
            "military_camps_01"
        ]
    );
    assert_eq!(h.refs[0].kind, pdxl_ck3::kinds::PROVINCE);
    assert!(
        h.refs[1..]
            .iter()
            .all(|r| r.kind == pdxl_ck3::kinds::BUILDING)
    );
    // … but a `buildings` list elsewhere means nothing.
    let elsewhere = extract("e = { buildings = { castle_01 } }\n", "events/x.txt");
    assert!(ref_names(&elsewhere).is_empty());
}

#[test]
fn effect_and_trigger_localization_defs_and_loc_refs() {
    let e = extract(
        "accept_activity_invite = {\n\
         \tfirst = I_ACCEPT_THE_INVITATION\n\
         \tthird_past = CHARACTER_ACCEPTED\n\
         \tglobal_neg = GLOBAL_LOST\n\
         }\n",
        "common/effect_localization/00.txt",
    );
    assert_eq!(e.defs[0].kind, pdxl_ck3::kinds::EFFECT_LOC);
    assert_eq!(
        ref_names(&e),
        vec![
            "I_ACCEPT_THE_INVITATION",
            "CHARACTER_ACCEPTED",
            "GLOBAL_LOST"
        ]
    );
    assert!(e.refs.iter().all(|r| r.kind == pdxl_analysis::LOC_KEY));

    let t = extract(
        "is_adult = { first_not = I_AM_NOT_ADULT }\n",
        "common/trigger_localization/00.txt",
    );
    assert_eq!(t.defs[0].kind, pdxl_ck3::kinds::TRIGGER_LOC);
    assert_eq!(ref_names(&t), vec!["I_AM_NOT_ADULT"]);

    // The same keys elsewhere mean nothing (`first` etc. are generic words).
    let elsewhere = extract("e = { first = X global = Y }\n", "events/x.txt");
    assert!(ref_names(&elsewhere).is_empty());
}

#[test]
fn custom_description_text_is_multi_kind() {
    let f = extract(
        "e = {\n\
         \tcustom_description = {\n\
         \t\ttext = T4N_buy_rice_can_afford\n\
         \t\tgold >= 150\n\
         \t}\n\
         }\n",
        "common/scripted_triggers/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["T4N_buy_rice_can_afford"]);
    let r = &f.refs[0];
    assert_eq!(r.kind, pdxl_ck3::kinds::TRIGGER_LOC);
    assert_eq!(
        r.alt,
        &[pdxl_ck3::kinds::EFFECT_LOC, pdxl_analysis::LOC_KEY]
    );
    // Deeper `text` (not a direct child) is untouched.
    let deep = extract(
        "e = { custom_description = { limit = { text = X } } }\n",
        "events/x.txt",
    );
    assert!(ref_names(&deep).is_empty());
}

#[test]
fn trait_opposites_and_compatibility_refs() {
    let d = extract(
        "brave = {\n\
         \tcategory = personality\n\
         \topposites = { craven }\n\
         \tcompatibility = { gluttonous = 20 drunkard = @pos_compat_low }\n\
         }\ncraven = { }\ngluttonous = { }\ndrunkard = { }\n",
        "common/traits/00.txt",
    );
    // opposites list items and compatibility block KEYS are trait refs.
    assert_eq!(ref_names(&d), vec!["craven", "gluttonous", "drunkard"]);
    assert!(d.refs.iter().all(|r| r.kind == pdxl_ck3::kinds::TRAIT));

    // The same keys outside the traits dir mean nothing.
    let elsewhere = extract(
        "e = { opposites = { craven } compatibility = { x = 1 } }\n",
        "events/x.txt",
    );
    assert!(ref_names(&elsewhere).is_empty());
}

#[test]
fn situation_scope_and_type_refs() {
    // `situation:X` scope literal and `situation_type = X` both resolve to a
    // situation_type, anywhere.
    let f = extract(
        "e = {\n\
         \tsituation_type = dynastic_cycle\n\
         \tif = { limit = { situation:silk_road_situation = { is_unique = yes } } }\n\
         }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&f), vec!["dynastic_cycle", "silk_road_situation"]);
    assert!(
        f.refs
            .iter()
            .all(|r| r.kind == pdxl_ck3::kinds::SITUATION_TYPE)
    );
}

#[test]
fn situation_group_type_ref_is_gated() {
    let f = extract(
        "dynastic_cycle = {\n\tsituation_group_type = major\n}\n",
        "common/situation/situations/00.txt",
    );
    assert_eq!(ref_names(&f), vec!["major"]);
    assert_eq!(f.refs[0].kind, pdxl_ck3::kinds::SITUATION_GROUP_TYPE);

    // `situation_group_type` outside the situations dir is not a ref.
    let elsewhere = extract("e = { situation_group_type = major }\n", "events/x.txt");
    assert!(ref_names(&elsewhere).is_empty());
}

#[test]
fn catalyst_refs_gated_and_struggle_excluded() {
    // Block keys inside a situation's future_phases catalysts map (gated dir).
    let sit = extract(
        "dynastic_cycle = {\n\
         \tphases = { p = { future_phases = { q = {\n\
         \t\tcatalysts = { catalyst_gain = 25 catalyst_loss = 30 }\n\
         \t} } } }\n\
         }\n",
        "common/situation/situations/00.txt",
    );
    assert_eq!(ref_names(&sit), vec!["catalyst_gain", "catalyst_loss"]);
    assert!(sit.refs.iter().all(|r| r.kind == pdxl_ck3::kinds::CATALYST));

    // `catalyst = X` inside a situation-catalyst effect resolves; the same
    // field inside `activate_struggle_catalyst` does NOT (separate database).
    let eff = extract(
        "e = {\n\
         \ttrigger_situation_catalyst = { catalyst = catalyst_gain character = scope:x }\n\
         \tactivate_struggle_catalyst = { catalyst = struggle_only_catalyst }\n\
         }\n",
        "events/x.txt",
    );
    assert_eq!(ref_names(&eff), vec!["catalyst_gain"]);
    assert_eq!(eff.refs[0].kind, pdxl_ck3::kinds::CATALYST);
}

#[test]
fn scheme_text_fields_are_loc_refs() {
    let f = extract(
        "murder = {\n\
         \tdesc = murder_desc\n\
         \tsuccess_desc = murder_success_desc\n\
         \tdiscovery_desc = MURDER_DISCOVERY_DESC\n\
         }\n",
        "common/schemes/scheme_types/00.txt",
    );
    assert_eq!(
        ref_names(&f),
        vec![
            "murder_desc",
            "murder_success_desc",
            "MURDER_DISCOVERY_DESC"
        ]
    );
    assert!(f.refs.iter().all(|r| r.kind == pdxl_analysis::LOC_KEY));

    // Those keys are loc refs only inside the schemes dir.
    let elsewhere = extract("x = { discovery_desc = Y }\n", "common/traits/00.txt");
    assert!(ref_names(&elsewhere).is_empty());
}

// ── provinces (ANALYSIS_VERSION 50) ──────────────────────────────────────────

#[test]
fn province_history_top_level_keys_are_refs_not_defs() {
    let f = extract(
        "@score = 100\n\
         8289 = {\n\
         \tculture = ethiopian\n\
         \tholding = castle_holding\n\
         \t1100.1.1 = { religion = coptic }\n\
         }\n\
         8290 = { holding = none }\n",
        "history/provinces/k_abyssinia.txt",
    );
    assert!(f.defs.is_empty(), "province history declares nothing");
    let provinces: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::PROVINCE)
        .map(|r| r.name.as_str())
        .collect();
    // `@score` is a script constant, not a province id; nested date blocks
    // (`1100.1.1`) are not top-level, so they don't fire either.
    assert_eq!(provinces, vec!["8289", "8290"]);

    // Body attributes cross-reference their own kinds (rules live in
    // culture.rs / faith.rs), at both the entry and the date level.
    let by_name = |n: &str| f.refs.iter().find(|r| r.name == n).expect("body ref");
    assert_eq!(by_name("ethiopian").kind, pdxl_ck3::kinds::CULTURE);
    assert_eq!(by_name("coptic").kind, pdxl_ck3::kinds::FAITH);
}

#[test]
fn province_top_level_keys_only_in_province_history_dir() {
    let f = extract("8289 = { x = y }\n", "common/scripted_effects/e.txt");
    assert!(
        f.refs.iter().all(|r| r.kind != pdxl_ck3::kinds::PROVINCE),
        "{:?}",
        f.refs
    );
}

#[test]
fn province_ref_in_landed_titles_and_scope_literal() {
    let f = extract(
        "c_shore = { b_port = { province = 1337 } }\n",
        "common/landed_titles/00.txt",
    );
    let r = f
        .refs
        .iter()
        .find(|r| r.name == "1337")
        .expect("barony province id extracted");
    assert_eq!(r.kind, pdxl_ck3::kinds::PROVINCE);

    // `province:X` works anywhere; a barony title key chains as the alt kind.
    let e = extract(
        "e = { scope:p = province:8780 loc = province:b_constantinople }\n",
        "common/scripted_effects/e.txt",
    );
    let by_name = |n: &str| e.refs.iter().find(|r| r.name == n).expect("scope ref");
    assert_eq!(by_name("8780").kind, pdxl_ck3::kinds::PROVINCE);
    assert_eq!(
        by_name("b_constantinople").alt,
        &[pdxl_ck3::kinds::TITLE],
        "title keys are the alternate resolution for province: literals"
    );
}

// ── terrain types (ANALYSIS_VERSION 52) ──────────────────────────────────────

#[test]
fn terrain_defs_and_ungated_refs() {
    let d = extract(
        "@cost = 25\n\
         hills = {\n\
         \tprovision_cost = @cost\n\
         \tprovince_modifier = { supply_limit_mult = -0.1 }\n\
         }\n",
        "common/terrain_types/00_terrains.txt",
    );
    assert_eq!(def_names(&d), vec!["hills"]);
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::TERRAIN_TYPE);

    // `terrain = X` resolves anywhere: province history, triggers (bare and
    // inside county_has_province_with_terrain), activity script.
    for (src, rel) in [
        ("100 = { terrain = hills }\n", "history/provinces/x.txt"),
        (
            "e = { county_has_province_with_terrain = { terrain = hills } }\n",
            "common/scripted_effects/e.txt",
        ),
    ] {
        let f = extract(src, rel);
        let r = f
            .refs
            .iter()
            .find(|r| r.name == "hills")
            .unwrap_or_else(|| panic!("terrain ref in {rel}"));
        assert_eq!(r.kind, pdxl_ck3::kinds::TERRAIN_TYPE);
    }

    // Macro values are skipped by the engine.
    let m = extract(
        "e = { terrain = $TERRAIN$ }\n",
        "common/scripted_effects/e.txt",
    );
    assert!(
        m.refs
            .iter()
            .all(|r| r.kind != pdxl_ck3::kinds::TERRAIN_TYPE)
    );
}

// ── file-local script constants (ANALYSIS_VERSION 53) ────────────────────────

#[test]
fn script_constants_defs_and_refs() {
    let f = extract(
        "@cost = 25\n\
         plains = {\n\
         \tprovision_cost = @cost\n\
         \ttravel_danger_score = @missing\n\
         \tmath = @[cost * 2]\n\
         }\n",
        "common/terrain_types/00.txt",
    );
    // The @def is a constant, never a directory-kind definition.
    assert_eq!(def_names(&f), vec!["plains"]);
    assert_eq!(f.constants.len(), 1);
    assert_eq!(f.constants[0].name, "@cost");
    assert_eq!(f.constants[0].kind, pdxl_analysis::SCRIPT_CONSTANT);
    // Uses are constant refs (inline math @[…] is skipped); they never leak
    // into the global ref stream.
    let names: Vec<&str> = f.constant_refs.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["@cost", "@missing"]);
    assert!(f.refs.iter().all(|r| !r.name.starts_with('@')));
}

#[test]
fn script_constants_not_script_value_defs() {
    // In common/script_values/ (TopLevelValued), @defs must not be claimed as
    // script values.
    let f = extract(
        "@base = 10\nmy_value = @base\n",
        "common/script_values/00.txt",
    );
    assert_eq!(def_names(&f), vec!["my_value"]);
    assert_eq!(f.constants[0].name, "@base");
}

#[test]
fn script_constants_resolve_per_file() {
    use pdxl_analysis::merge_and_resolve;
    use std::collections::HashMap;

    // File A defines @cost; file B uses it without defining it — the use in B
    // must NOT resolve against A (constants are file-local).
    let a = extract("@cost = 1\nx = { }\n", "common/traits/a.txt");
    let b = extract(
        "y = { potential = { gold = @cost } }\n",
        "common/traits/b.txt",
    );
    let mut facts = HashMap::new();
    facts.insert("common/traits/a.txt".to_string(), a);
    facts.insert("common/traits/b.txt".to_string(), b);
    let order = ["common/traits/a.txt", "common/traits/b.txt"];
    let (_, diags) = merge_and_resolve(&order, &facts);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert!(diags[0].msg.contains("unknown script_constant \"@cost\""));
}

// ── named colors (ANALYSIS_VERSION 55) ───────────────────────────────────────

#[test]
fn named_color_defs_and_refs() {
    let d = extract(
        "colors = {\n\
         \tenglish = { 0.8 0.2 0.2 }\n\
         \tbrown = hsv360 { 21 74 45 }\n\
         }\n",
        "common/named_colors/default_colors.txt",
    );
    assert_eq!(def_names(&d), vec!["english", "brown"]);
    assert!(
        d.defs
            .iter()
            .all(|d| d.kind == pdxl_ck3::kinds::NAMED_COLOR)
    );

    // Scalar color fields reference named colors in their gated dirs.
    let c = extract(
        "my_culture = { color = english }\n",
        "common/culture/cultures/00.txt",
    );
    let r = c.refs.iter().find(|r| r.name == "english").expect("ref");
    assert_eq!(r.kind, pdxl_ck3::kinds::NAMED_COLOR);

    // CoA slot indirection and the list selector are skips, not names.
    let coa = extract(
        "d_norm = {\n\
         \tcolor1 = english\n\
         \tcolored_emblem = { color1 = color2 color2 = list \"fp2_standard\" }\n\
         }\n",
        "common/coat_of_arms/coat_of_arms/00.txt",
    );
    let names: Vec<&str> = coa
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::NAMED_COLOR)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, vec!["english"]);

    // `color = X` outside the gated dirs (genes, modifier formats) is not a ref.
    let genes = extract("e = { color = hair }\n", "common/genes/00.txt");
    assert!(
        genes
            .refs
            .iter()
            .all(|r| r.kind != pdxl_ck3::kinds::NAMED_COLOR)
    );
}

// ── religion domain: doctrines, holy sites, families (ANALYSIS_VERSION 57) ───

#[test]
fn religion_domain_defs_and_refs() {
    // Doctrines: top-level defs; referenced by doctrine/has_doctrine anywhere.
    let d = extract(
        "doctrine_monogamy = { piety_cost = { value = 5 } }\n",
        "common/religion/doctrine_types/20_doctrines.txt",
    );
    assert_eq!(d.defs[0].kind, pdxl_ck3::kinds::DOCTRINE);
    let e = extract(
        "e = { has_doctrine = doctrine_monogamy }\n",
        "common/scripted_triggers/x.txt",
    );
    assert_eq!(e.refs[0].kind, pdxl_ck3::kinds::DOCTRINE);

    // Holy sites: defs + faith-body refs; county/barony are title refs.
    let h = extract(
        "jerusalem = { county = c_jerusalem barony = b_vaticano }\n",
        "common/religion/holy_site_types/00.txt",
    );
    assert_eq!(h.defs[0].kind, pdxl_ck3::kinds::HOLY_SITE);
    let titles: Vec<&str> = h
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_ck3::kinds::TITLE)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(titles, vec!["c_jerusalem", "b_vaticano"]);

    // Religion bodies: family/doctrine/religious_head/holy_site/virtues refs.
    let r = extract(
        "my_religion = {\n\
         \tfamily = rf_abrahamic\n\
         \tdoctrine = doctrine_monogamy\n\
         \ttraits = { virtues = { brave honest = 0.5 } sins = { craven } }\n\
         \tfaiths = {\n\
         \t\tmy_faith = { religious_head = k_papal_state holy_site = jerusalem }\n\
         \t}\n\
         }\n",
        "common/religion/religion_types/00.txt",
    );
    let kind_of = |n: &str| r.refs.iter().find(|x| x.name == n).expect(n).kind;
    assert_eq!(kind_of("rf_abrahamic"), pdxl_ck3::kinds::RELIGION_FAMILY);
    assert_eq!(kind_of("doctrine_monogamy"), pdxl_ck3::kinds::DOCTRINE);
    assert_eq!(kind_of("k_papal_state"), pdxl_ck3::kinds::TITLE);
    assert_eq!(kind_of("jerusalem"), pdxl_ck3::kinds::HOLY_SITE);
    for t in ["brave", "honest", "craven"] {
        assert_eq!(kind_of(t), pdxl_ck3::kinds::TRAIT, "{t}");
    }
    // The faith itself is still the def.
    assert_eq!(def_names(&r), vec!["my_faith"]);
}

// ── religion localization maps (ANALYSIS_VERSION 58) ─────────────────────────

#[test]
fn religion_localization_values_are_loc_refs() {
    let f = extract(
        "x = {\n\
         \tlocalization = {\n\
         \t\tHighGodName = generic_high_god_name\n\
         \t\tPantheon = { PANTHEON_A PANTHEON_B }\n\
         \t}\n\
         \tfaiths = { y = { localization = { DevilName = devil_name } } }\n\
         }\n",
        "common/religion/religion_types/00.txt",
    );
    let locs: Vec<&str> = f
        .refs
        .iter()
        .filter(|r| r.kind == pdxl_analysis::LOC_KEY)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(
        locs,
        vec![
            "generic_high_god_name",
            "PANTHEON_A",
            "PANTHEON_B",
            "devil_name"
        ]
    );
    // The dynamic keys themselves are NOT references.
    assert!(f.refs.iter().all(|r| r.name != "HighGodName"));

    // `localization` blocks outside common/religion/ mean nothing.
    let e = extract("e = { localization = { K = some_key } }\n", "events/x.txt");
    assert!(e.refs.iter().all(|r| r.kind != pdxl_analysis::LOC_KEY));
}

// ── game rules (ANALYSIS_VERSION 59) ─────────────────────────────────────────

#[test]
fn game_rule_settings_defs_and_refs() {
    let f = extract(
        "difficulty = {\n\
         \tcategories = { difficulty ai }\n\
         \tdefault = normal_difficulty\n\
         \tnormal_difficulty = { }\n\
         \thard_difficulty = {\n\
         \t\tapply_modifier = ai:hard_difficulty\n\
         \t\tflag = blocks_achievements\n\
         \t}\n\
         }\n",
        "common/game_rules/00_game_rules.txt",
    );
    // Settings are the defs; `categories` (block attribute) and the rule
    // itself are not.
    assert_eq!(def_names(&f), vec!["normal_difficulty", "hard_difficulty"]);
    assert!(
        f.defs
            .iter()
            .all(|d| d.kind == pdxl_ck3::kinds::GAME_RULE_SETTING)
    );
    // `default` references a setting; `apply_modifier`'s ai: literal
    // references a static modifier.
    let kind_of = |n: &str| f.refs.iter().find(|r| r.name == n).expect(n).kind;
    assert_eq!(
        kind_of("normal_difficulty"),
        pdxl_ck3::kinds::GAME_RULE_SETTING
    );
    assert_eq!(kind_of("hard_difficulty"), pdxl_ck3::kinds::MODIFIER);

    // The trigger resolves anywhere.
    let t = extract(
        "e = { has_game_rule = hard_difficulty }\n",
        "common/scripted_triggers/x.txt",
    );
    assert_eq!(t.refs[0].kind, pdxl_ck3::kinds::GAME_RULE_SETTING);

    // `ai:`/`player:` literals mean nothing outside the game-rules dir.
    let e = extract("e = { x = ai:whatever }\n", "common/scripted_effects/e.txt");
    assert!(e.refs.iter().all(|r| r.kind != pdxl_ck3::kinds::MODIFIER));
}
