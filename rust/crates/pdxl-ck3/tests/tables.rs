//! Sanity checks over the generated script-documentation tables.
//!
//! These do not re-verify the game's data (the diff review at regeneration
//! time does that); they pin the invariants consumers will rely on: sorted
//! unique names, plausible sizes, and a few known rows staying recognizable.

use pdxl_ck3::tables::{CODE_SAVED_SCOPES, EFFECTS, MODIFIERS, SCOPE_LINKS, SCOPE_TYPES, TRIGGERS};

fn assert_sorted_unique(names: impl Iterator<Item = &'static str>, what: &str) {
    let names: Vec<&str> = names.collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(names, sorted, "{what} names must be sorted and unique");
}

#[test]
fn tables_are_sorted_unique_and_plausibly_sized() {
    assert_sorted_unique(EFFECTS.iter().map(|r| r.name), "effect");
    assert_sorted_unique(TRIGGERS.iter().map(|r| r.name), "trigger");
    assert_sorted_unique(MODIFIERS.iter().map(|r| r.tag), "modifier");
    assert_sorted_unique(CODE_SAVED_SCOPES.iter().copied(), "code-saved scope");

    // Link names legitimately repeat: the game overloads a name across link
    // forms — data link vs chain link (`culture:norse` / `.culture`,
    // `dynasty:123` / `.dynasty`). Consumers must expect several rows per
    // name; here we only pin sortedness and that no two rows are identical.
    let link_names: Vec<&str> = SCOPE_LINKS.iter().map(|r| r.name).collect();
    assert!(link_names.is_sorted(), "scope link names must be sorted");
    let mut keys: Vec<String> = SCOPE_LINKS.iter().map(|r| format!("{r:?}")).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), SCOPE_LINKS.len(), "no identical link rows");

    // Regeneration should only ever grow these (a shrink means the parser
    // started dropping stanzas — investigate before accepting the diff).
    assert!(EFFECTS.len() >= 1900, "effects: {}", EFFECTS.len());
    assert!(TRIGGERS.len() >= 1700, "triggers: {}", TRIGGERS.len());
    assert!(SCOPE_LINKS.len() >= 270, "links: {}", SCOPE_LINKS.len());
    assert!(
        SCOPE_TYPES.len() >= 70,
        "scope types: {}",
        SCOPE_TYPES.len()
    );
    assert!(MODIFIERS.len() >= 700, "modifiers: {}", MODIFIERS.len());
}

#[test]
fn known_rows_survive_regeneration() {
    let add_gold = EFFECTS.iter().find(|r| r.name == "add_gold").unwrap();
    assert_eq!(add_gold.scopes, ["character"]);
    assert_eq!(add_gold.description, "adds gold to a character");

    let has_trait = TRIGGERS.iter().find(|r| r.name == "has_trait").unwrap();
    assert_eq!(has_trait.scopes, ["character"]);

    // The `title:` global data link — the table row our hardcoded
    // ScopePrefix("title") schema rule corresponds to.
    let title = SCOPE_LINKS.iter().find(|r| r.name == "title").unwrap();
    assert!(title.requires_data && title.global_link);
    assert_eq!(title.output_scopes, ["landed_title"]);

    // `.holder` navigation: landed_title → character.
    let holder = SCOPE_LINKS.iter().find(|r| r.name == "holder").unwrap();
    assert!(!holder.global_link);
    assert_eq!(holder.input_scopes, ["landed_title"]);
    assert_eq!(holder.output_scopes, ["character"]);

    let character = SCOPE_TYPES.iter().find(|r| r.name == "character").unwrap();
    assert!(character.evaluate_triggers && character.execute_effects);
    assert_eq!(character.save_token, Some("char"));
}
