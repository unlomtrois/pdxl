//! Structural-context tests over the CK3 specs: `context_at` must classify
//! every position the design doc calls out (`rust/docs/STRUCTURAL-CONTEXTS.md`),
//! including the option inline-effect fallback, the `limit` duality, and
//! per-context key meaning.

use pdxl_analysis::context::{ClauseKind, context_at};
use pdxl_ast::{NodeId, SyntaxTree};

/// Parses `src` as `rel_path` and returns the context of the `index`-th node
/// whose text equals `needle` (0-based; keys and scalar values both match).
fn ctx_of_nth(src: &str, rel_path: &str, needle: &str, index: usize) -> ClauseKind {
    let parsed = pdxl_parser::parse(rel_path.to_string(), src.as_bytes().to_vec());
    let tree = parsed.tree();
    let node = find_nth(tree, needle.as_bytes(), index)
        .unwrap_or_else(|| panic!("needle {needle:?} (#{index}) not found"));
    context_at(tree, node, rel_path, pdxl_ck3::contexts::context_schema())
}

fn ctx_of(src: &str, rel_path: &str, needle: &str) -> ClauseKind {
    ctx_of_nth(src, rel_path, needle, 0)
}

fn find_nth(tree: &SyntaxTree, needle: &[u8], mut index: usize) -> Option<NodeId> {
    let mut stack = vec![tree.root()];
    let mut ordered = Vec::new();
    while let Some(id) = stack.pop() {
        ordered.push(id);
        for child in tree.children(id) {
            stack.push(child);
        }
    }
    // DFS with a stack visits siblings in reverse; sort by source position.
    ordered.sort_by_key(|&id| tree.node(id).range.start);
    for id in ordered {
        if tree.node_text(id) == needle {
            if index == 0 {
                return Some(id);
            }
            index -= 1;
        }
    }
    None
}

// ── directory roots ─────────────────────────────────────────────────────────

#[test]
fn directory_roots_set_definition_body_context() {
    let eff = "e = {\n\tadd_gold = 50\n}\n";
    assert_eq!(
        ctx_of(eff, "common/scripted_effects/e.txt", "add_gold"),
        ClauseKind::Effect
    );
    let trg = "t = {\n\thas_trait = brave\n}\n";
    assert_eq!(
        ctx_of(trg, "common/scripted_triggers/t.txt", "has_trait"),
        ClauseKind::Trigger
    );
    let sv = "v = {\n\tvalue = 5\n}\n";
    assert_eq!(
        ctx_of(sv, "common/script_values/v.txt", "value"),
        ClauseKind::ScriptValue
    );
    // Unknown directories claim nothing.
    assert_eq!(
        ctx_of(eff, "gfx/whatever.txt", "add_gold"),
        ClauseKind::Unknown
    );
}

#[test]
fn top_level_scalars_are_config() {
    let src = "namespace = test\ntest.1 = { }\n";
    assert_eq!(ctx_of(src, "events/e.txt", "test"), ClauseKind::Config);
}

// ── events ──────────────────────────────────────────────────────────────────

const EVENT_SRC: &str = r#"namespace = test
test.1 = {
	type = character_event
	title = test.1.t
	desc = {
		first_valid = {
			triggered_desc = {
				trigger = { has_trait = brave }
				desc = test.1.brave
			}
			desc = test.1.fallback
		}
	}
	trigger = { is_adult = yes }
	immediate = {
		if = {
			limit = { gold >= 50 }
			remove_short_term_gold = 50
		}
		save_scope_as = payer
	}
	left_portrait = {
		character = scope:payer
		trigger = { is_alive = yes }
	}
	option = {
		name = test.1.a
		trigger = { is_landed = yes }
		ai_chance = {
			base = 10
			modifier = {
				add = 5
				has_trait = greedy
			}
		}
		ai_will_select = {
			base = 10
			if = {
				limit = { has_trait = brave }
				add = 5
			}
		}
		add_dread = 5
		stress_impact = {
			brave = minor_stress_impact_loss
		}
	}
	after = { add_prestige = 10 }
}
"#;

#[test]
fn event_effect_and_trigger_blocks() {
    let f = |needle| ctx_of(EVENT_SRC, "events/e.txt", needle);
    assert_eq!(f("save_scope_as"), ClauseKind::Effect); // immediate
    assert_eq!(f("add_prestige"), ClauseKind::Effect); // after
    assert_eq!(f("is_adult"), ClauseKind::Trigger); // trigger
    // Event config keys live in the event struct context.
    assert_eq!(f("character_event"), ClauseKind::Config);
}

#[test]
fn limit_inside_effect_flips_to_trigger() {
    let f = |needle| ctx_of(EVENT_SRC, "events/e.txt", needle);
    assert_eq!(f("gold"), ClauseKind::Trigger); // if = { limit = { … } }
    assert_eq!(f("remove_short_term_gold"), ClauseKind::Effect); // if body
}

#[test]
fn option_unknown_keys_are_inline_effects() {
    let f = |needle| ctx_of(EVENT_SRC, "events/e.txt", needle);
    // A key reports its container: option keys complete from the option
    // struct (whose fallback is Effect — the inline-effects rule)…
    assert_eq!(f("add_dread"), ClauseKind::Struct(pick_spec("option")));
    // …and an unknown key's VALUE gets effect context, at any depth.
    assert_eq!(
        ctx_of_nth(EVENT_SRC, "events/e.txt", "5", 2),
        ClauseKind::Effect
    ); // add_dread = 5
    assert_eq!(f("minor_stress_impact_loss"), ClauseKind::Effect); // inside stress_impact block
    // …while known structural fields keep their own contexts.
    assert_eq!(f("is_landed"), ClauseKind::Trigger); // option trigger
}

#[test]
fn option_ai_blocks() {
    let f = |needle| ctx_of(EVENT_SRC, "events/e.txt", needle);
    // ai_chance modifier blocks hold trigger conditions.
    assert_eq!(f("greedy"), ClauseKind::Trigger);
    // ai_will_select is a script value; its if-limit is a trigger.
    assert_eq!(
        ctx_of_nth(EVENT_SRC, "events/e.txt", "brave", 1),
        ClauseKind::Trigger
    );
    // …and the stress_impact key inside the option body is an inline effect.
    assert_eq!(
        ctx_of_nth(EVENT_SRC, "events/e.txt", "brave", 2),
        ClauseKind::Effect
    );
    assert_eq!(f("add"), ClauseKind::Trigger); // add inside ai_chance modifier
}

#[test]
fn dynamic_desc_nests_and_escapes_to_trigger() {
    let f = |needle| ctx_of(EVENT_SRC, "events/e.txt", needle);
    assert_eq!(f("first_valid"), ClauseKind::DynamicDesc);
    assert_eq!(f("triggered_desc"), ClauseKind::DynamicDesc);
    // triggered_desc's trigger block is trigger context (first `brave`).
    assert_eq!(
        ctx_of_nth(EVENT_SRC, "events/e.txt", "brave", 0),
        ClauseKind::Trigger
    );
    // Scalar desc at event level is config (a loc key).
    assert_eq!(
        ctx_of(EVENT_SRC, "events/e.txt", "test.1.t"),
        ClauseKind::Config
    );
}

#[test]
fn portrait_block_form_and_its_trigger() {
    let f = |needle| ctx_of(EVENT_SRC, "events/e.txt", needle);
    assert_eq!(f("is_alive"), ClauseKind::Trigger); // portrait trigger
    assert_eq!(f("scope:payer"), ClauseKind::Config); // character = <target>
}

#[test]
fn unknown_event_key_is_rejected() {
    let src = "namespace = t\nt.1 = {\n\tnot_an_event_field = { x = y }\n}\n";
    assert_eq!(ctx_of(src, "events/e.txt", "x"), ClauseKind::Unknown);
}

// ── decisions ───────────────────────────────────────────────────────────────

#[test]
fn decision_contexts() {
    let src = "d = {\n\
        \tis_shown = { is_ruler = yes }\n\
        \teffect = { add_gold = 5 }\n\
        \tai_will_do = {\n\t\tbase = 100\n\t\tmodifier = { factor = 0 is_at_war = yes }\n\t}\n\
        \tcost = { gold = { value = 50 } }\n\
        }\n";
    let f = |needle| ctx_of(src, "common/decisions/d.txt", needle);
    assert_eq!(f("is_ruler"), ClauseKind::Trigger);
    assert_eq!(f("add_gold"), ClauseKind::Effect);
    assert_eq!(f("is_at_war"), ClauseKind::Trigger); // ai_will_do modifier
    assert_eq!(f("value"), ClauseKind::ScriptValue); // cost gold block
}

// ── on_actions ──────────────────────────────────────────────────────────────

#[test]
fn on_action_contexts() {
    let src = "my_oa = {\n\
        \ttrigger = { is_adult = yes }\n\
        \teffect = { add_gold = 1 }\n\
        \tevents = {\n\t\tt.1\n\t\tdelay = { days = { 1 30 } }\n\t\tt.2\n\t}\n\
        \trandom_events = {\n\t\tchance_to_happen = 25\n\t\t100 = t.3\n\t}\n\
        \tweight_multiplier = {\n\t\tbase = 1\n\t\tmodifier = { add = 1 is_ruler = yes }\n\t}\n\
        }\n";
    let f = |needle| ctx_of(src, "common/on_action/oa.txt", needle);
    assert_eq!(f("is_adult"), ClauseKind::Trigger);
    assert_eq!(f("add_gold"), ClauseKind::Effect);
    // A loose event-id item sits directly in the fire-list struct.
    assert_eq!(f("t.1"), ClauseKind::Struct(pick_spec("fire_list")));
    assert_eq!(f("t.3"), ClauseKind::Config); // weighted `100 = t.3` value (dynamic key)
    assert_eq!(f("is_ruler"), ClauseKind::Trigger); // weight_multiplier modifier
    // The delay's days block is a script value range.
    assert_eq!(f("delay"), ClauseKind::Struct(pick_spec("fire_list")));
}

/// Resolves a StructSpec by name through the public schema (for asserting
/// struct contexts without exporting the statics).
fn pick_spec(name: &str) -> &'static pdxl_analysis::context::StructSpec {
    fn find(
        kind: &ClauseKind,
        name: &str,
        seen: &mut Vec<&'static str>,
    ) -> Option<&'static pdxl_analysis::context::StructSpec> {
        let ClauseKind::Struct(spec) = kind else {
            return None;
        };
        if spec.name == name {
            return Some(spec);
        }
        if seen.contains(&spec.name) {
            return None;
        }
        seen.push(spec.name);
        // Follow field blocks and the struct fallback (law group → law).
        if let pdxl_analysis::context::Fallback::Struct(inner) = spec.fallback
            && let found @ Some(_) = find(&ClauseKind::Struct(inner), name, seen)
        {
            return found;
        }
        spec.fields
            .iter()
            .filter_map(|(_, f)| f.block.as_ref())
            .find_map(|k| find(k, name, seen))
    }
    pdxl_ck3::contexts::context_schema()
        .roots
        .iter()
        .find_map(|(_, k)| find(k, name, &mut Vec::new()))
        .unwrap_or_else(|| panic!("spec {name:?} not reachable"))
}

// ── laws (`_laws.info`) ─────────────────────────────────────────────────────

const LAW_SRC: &str = r#"crown_authority = {
	default = crown_authority_1
	cumulative = yes
	can_change_law_group = { always = yes }
	crown_authority_0 = {
		can_keep = { has_trait = brave }
		can_pass = { is_adult = yes }
		on_pass = { add_gold = 5 }
		pass_cost = { gold = 50 }
		modifier = { some_opinion = 10 }
		succession = { order_of_succession = inheritance }
		triggered_flag = { trigger = { is_ruler = yes } flag = x }
		ai_will_do = { value = 10 }
	}
}
"#;

#[test]
fn law_group_and_law_field_contexts() {
    let f = |needle| ctx_of(LAW_SRC, "common/laws/00_realm.txt", needle);
    // A law name (unknown group key) opens the law struct; its fields sit
    // in the law struct context.
    assert_eq!(
        f("crown_authority_0"),
        ClauseKind::Struct(pick_spec("law_group"))
    );
    assert_eq!(f("can_keep"), ClauseKind::Struct(pick_spec("law")));
    // Group attributes report the law-group context.
    assert_eq!(f("default"), ClauseKind::Struct(pick_spec("law_group")));
    // can_change_law_group is a trigger block.
    assert_eq!(f("always"), ClauseKind::Trigger);
}

#[test]
fn law_trigger_effect_value_blocks() {
    let f = |needle| ctx_of(LAW_SRC, "common/laws/00_realm.txt", needle);
    assert_eq!(f("has_trait"), ClauseKind::Trigger); // can_keep
    assert_eq!(f("is_adult"), ClauseKind::Trigger); // can_pass
    assert_eq!(f("add_gold"), ClauseKind::Effect); // on_pass
    assert_eq!(f("value"), ClauseKind::ScriptValue); // ai_will_do
    assert_eq!(f("is_ruler"), ClauseKind::Trigger); // triggered_flag.trigger
    // Nested structs: cost / succession keys report their own struct.
    assert_eq!(f("gold"), ClauseKind::Struct(pick_spec("cost")));
    assert_eq!(
        f("order_of_succession"),
        ClauseKind::Struct(pick_spec("succession"))
    );
}
