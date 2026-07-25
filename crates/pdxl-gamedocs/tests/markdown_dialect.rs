//! EU5-era Markdown dump dialect: `## name` doc sections, `### name` scope
//! links, one-line `Tag: X, Categories: …` modifiers.

use pdxl_gamedocs::{
    is_markdown_doc, parse_doc_log_md, parse_event_targets_md, parse_modifiers_eu5,
};

#[test]
fn markdown_doc_log() {
    let text = "\u{feff}# Effect Documentation\n\
                ## abandon_location\n\
                Abandons the target location!\n\
                **Supported Scopes**: country  \n\
                **Supported Targets**: location  \n\
                \n\
                ## add_accepted_culture\n\
                Adds an accepted culture to a country\n\
                **Supported Scopes**: country  \n\
                **Supported Targets**: culture  \n";
    assert!(is_markdown_doc(text));
    let entries = parse_doc_log_md(text);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "abandon_location");
    assert_eq!(entries[0].description, "Abandons the target location!");
    assert_eq!(entries[0].scopes, vec!["country"]);
    assert_eq!(entries[0].targets, vec!["location"]);
}

#[test]
fn markdown_event_targets_and_code_saved() {
    let text = "# Event Target Documentation\n\
                ### area\n\
                Unknown, add something in code registration\n\
                Requires Data: yes\n\
                Output Scopes: area\n\
                \n\
                ### dominant_language\n\
                desc\n\
                Input Scopes: location, country\n\
                Output Scopes: language\n\
                \n\
                --------------------\n\
                \n\
                Event Targets Saved from Code:\n\
                \n\
                context\n\
                neighbor\n";
    let (links, code_saved) = parse_event_targets_md(text);
    assert_eq!(links.len(), 2);
    assert!(links[0].requires_data);
    assert_eq!(links[1].input_scopes, vec!["location", "country"]);
    assert_eq!(links[1].output_scopes, vec!["language"]);
    assert_eq!(code_saved, vec!["context", "neighbor"]);
}

#[test]
fn eu5_modifiers_single_line() {
    let text = "Printing Modifier Definitions:\n\
                Tag: ai_opinion_bias, Categories: Country, , All, \n\
                Tag: army_artillery_power, Categories: Unit, , All, \n";
    let mods = parse_modifiers_eu5(text);
    assert_eq!(mods.len(), 2);
    assert_eq!(mods[0].tag, "ai_opinion_bias");
    assert_eq!(mods[0].use_areas, vec!["Country", "All"]);
}
