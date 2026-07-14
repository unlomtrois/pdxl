//! Parsers for the CK3 script-documentation dumps.
//!
//! The game writes its complete static script model into `logs/` (Paradox
//! user directory): every effect and trigger with the scopes it is legal in
//! (`effects.log`, `triggers.log`), every scope-transition link
//! (`event_targets.log` — not only for events, despite the name), the scope
//! *types* themselves (`event_scopes.log`), and the permanent stat modifiers
//! (`modifiers.log`).
//!
//! These parsers turn each dump into plain structured rows. They deliberately
//! do **not** interpret the data (e.g. what `Supported Scopes: none` means is
//! the scope engine's decision, not the parser's) — values are preserved
//! verbatim so the generated tables are a faithful, diffable transcription of
//! the game's own documentation.
//!
//! The `gen-tables` binary in this crate renders parsed rows into Rust source
//! tables under `pdxl-ck3/src/tables/`, which are committed and reviewed like
//! golden files and regenerated per game patch.

/// One effect or trigger stanza from `effects.log` / `triggers.log`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocEntry {
    pub name: String,
    /// `Supported Scopes:` values, comma-split, verbatim (may be `none`).
    pub scopes: Vec<String>,
    /// `Supported Targets:` values (iterator element scopes), comma-split.
    pub targets: Vec<String>,
}

/// One scope-transition link from `event_targets.log`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetLink {
    pub name: String,
    /// Takes a `:key` argument (`title:e_hre`, `character:1234`).
    pub requires_data: bool,
    /// Usable from any scope (no `Input Scopes:` requirement).
    pub global_link: bool,
    pub wildcard: bool,
    pub input_scopes: Vec<String>,
    pub output_scopes: Vec<String>,
}

/// One scope type from `event_scopes.log` (the scope-type registry).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeType {
    pub name: String,
    pub evaluate_triggers: bool,
    pub execute_effects: bool,
    pub change_scopes: bool,
    /// `Save Token:` value; `None` when the dump says `none`.
    pub save_token: Option<String>,
    pub stores_variables: bool,
}

/// One permanent stat modifier from `modifiers.log`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModifierDef {
    /// May be templated (`$TRAIT_TRACK$_xp_gain_mult`).
    pub tag: String,
    /// `Use areas:` values, comma-split.
    pub use_areas: Vec<String>,
}

const STANZA_SEP: &str = "--------------------";

/// Splits a doc log into stanzas on the `-----` separator lines.
fn stanzas(text: &str) -> impl Iterator<Item = &str> {
    text.split(STANZA_SEP)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Whether `name` looks like a script identifier (the dumps intersperse
/// prose stanzas — warnings, usage notes — that must be skipped).
fn is_script_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

/// Splits a comma-separated field value into trimmed, non-empty items.
fn split_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

fn field_value<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    line.strip_prefix(label).map(str::trim)
}

/// Parses `effects.log` or `triggers.log` (same stanza format): each entry is
/// `name - description` followed by `Supported Scopes:` / `Supported
/// Targets:` lines. Prose stanzas (warnings, headers) are skipped.
pub fn parse_doc_log(text: &str) -> Vec<DocEntry> {
    let mut out = Vec::new();
    for stanza in stanzas(text) {
        let mut lines = stanza.lines();
        let Some(first) = lines.next() else { continue };
        // The name may stand alone or be `name - description`.
        let name = first.split(" - ").next().unwrap_or(first).trim();
        if !is_script_name(name) {
            continue;
        }
        let mut entry = DocEntry {
            name: name.to_string(),
            scopes: Vec::new(),
            targets: Vec::new(),
        };
        for line in lines {
            if let Some(v) = field_value(line, "Supported Scopes:") {
                entry.scopes = split_values(v);
            } else if let Some(v) = field_value(line, "Supported Targets:") {
                entry.targets = split_values(v);
            }
        }
        // Every real entry documents its scopes; nameless prose that happens
        // to start with a lowercase word does not.
        if !entry.scopes.is_empty() || !entry.targets.is_empty() {
            out.push(entry);
        }
    }
    out
}

/// Parses `event_targets.log`: the scope-link stanzas plus the trailing
/// `Event Targets Saved from Code:` name list (scope names the engine saves
/// itself — `scope:actor`, `scope:recipient`, …).
pub fn parse_event_targets(text: &str) -> (Vec<TargetLink>, Vec<String>) {
    const CODE_SAVED_HEADER: &str = "Event Targets Saved from Code:";
    let (stanza_part, code_part) = match text.split_once(CODE_SAVED_HEADER) {
        Some((a, b)) => (a, b),
        None => (text, ""),
    };

    let mut links = Vec::new();
    for stanza in stanzas(stanza_part) {
        let mut lines = stanza.lines();
        let Some(first) = lines.next() else { continue };
        let name = first.split(" - ").next().unwrap_or(first).trim();
        if !is_script_name(name) {
            continue;
        }
        let mut link = TargetLink {
            name: name.to_string(),
            requires_data: false,
            global_link: false,
            wildcard: false,
            input_scopes: Vec::new(),
            output_scopes: Vec::new(),
        };
        for line in lines {
            if let Some(v) = field_value(line, "Requires Data:") {
                link.requires_data = v == "yes";
            } else if let Some(v) = field_value(line, "Global Link:") {
                link.global_link = v == "yes";
            } else if let Some(v) = field_value(line, "Wild Card:") {
                link.wildcard = v == "yes";
            } else if let Some(v) = field_value(line, "Input Scopes:") {
                link.input_scopes = split_values(v);
            } else if let Some(v) = field_value(line, "Output Scopes:") {
                link.output_scopes = split_values(v);
            }
        }
        if !link.output_scopes.is_empty() {
            links.push(link);
        }
    }

    let code_saved = code_part
        .lines()
        .map(str::trim)
        .filter(|l| is_script_name(l) && !l.is_empty())
        .map(str::to_string)
        .collect();
    (links, code_saved)
}

/// Parses `event_scopes.log`: `name:` headers each followed by
/// `Attribute: value` lines.
pub fn parse_event_scopes(text: &str) -> Vec<ScopeType> {
    let mut out: Vec<ScopeType> = Vec::new();
    for line in text.lines().map(str::trim) {
        // A scope-type header is a bare `name:` (attribute lines all contain
        // a space before their value; the file header ends in `:` too but is
        // capitalized prose).
        if let Some(name) = line.strip_suffix(':')
            && is_script_name(name)
        {
            out.push(ScopeType {
                name: name.to_string(),
                evaluate_triggers: false,
                execute_effects: false,
                change_scopes: false,
                save_token: None,
                stores_variables: false,
            });
            continue;
        }
        let Some(current) = out.last_mut() else {
            continue;
        };
        if let Some(v) = field_value(line, "Evaluate Triggers:") {
            current.evaluate_triggers = v == "yes";
        } else if let Some(v) = field_value(line, "Execute Effects:") {
            current.execute_effects = v == "yes";
        } else if let Some(v) = field_value(line, "Change Scopes:") {
            current.change_scopes = v == "yes";
        } else if let Some(v) = field_value(line, "Save Token:") {
            current.save_token = (v != "none").then(|| v.to_string());
        } else if let Some(v) = field_value(line, "Stores Variables:") {
            current.stores_variables = v == "yes";
        }
    }
    out
}

/// Parses `modifiers.log`: repeated `Tag:` / `Use areas:` pairs.
pub fn parse_modifiers(text: &str) -> Vec<ModifierDef> {
    let mut out: Vec<ModifierDef> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(tag) = field_value(line, "Tag:") {
            out.push(ModifierDef {
                tag: tag.to_string(),
                use_areas: Vec::new(),
            });
        } else if let Some(v) = field_value(line, "Use areas:")
            && let Some(current) = out.last_mut()
        {
            current.use_areas = split_values(v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_log_parses_entries_and_skips_prose() {
        let entries = parse_doc_log(
            "Effect Documentation:\n\n\
             --------------------\n\n\
             add_gold - adds gold to a character\n\
             Supported Scopes: character\n\n\
             --------------------\n\n\
             WARNING: some prose stanza\n\
             Supported Scopes: character\n\n\
             --------------------\n\n\
             any_child = { <triggers> }\n\
             a stanza whose first token is not an identifier line\n\n\
             --------------------\n\n\
             marry - character, artifact scopes for the test\n\
             Multi-line description continues\n\
             Supported Scopes: character, artifact\n\
             Supported Targets: character\n",
        );
        assert_eq!(
            entries,
            vec![
                DocEntry {
                    name: "add_gold".into(),
                    scopes: vec!["character".into()],
                    targets: vec![],
                },
                DocEntry {
                    name: "marry".into(),
                    scopes: vec!["character".into(), "artifact".into()],
                    targets: vec!["character".into()],
                },
            ]
        );
    }

    #[test]
    fn event_targets_parses_links_and_code_saved_names() {
        let (links, code_saved) = parse_event_targets(
            "Event Target Documentation:\n\n\
             --------------------\n\n\
             title - Get the title with the specified key\n\
             Requires Data: yes\n\
             Global Link: yes\n\
             Output Scopes: landed_title\n\n\
             --------------------\n\n\
             holder - the character holding the title\n\
             Input Scopes: landed_title\n\
             Output Scopes: character\n\n\
             --------------------\n\n\
             Event Targets Saved from Code:\n\n\
             actor\n\
             recipient\n",
        );
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].name, "title");
        assert!(links[0].requires_data && links[0].global_link);
        assert_eq!(links[0].output_scopes, vec!["landed_title"]);
        assert!(links[0].input_scopes.is_empty());
        assert_eq!(links[1].name, "holder");
        assert!(!links[1].global_link);
        assert_eq!(links[1].input_scopes, vec!["landed_title"]);
        assert_eq!(code_saved, vec!["actor", "recipient"]);
    }

    #[test]
    fn event_scopes_parses_type_registry() {
        let types = parse_event_scopes(
            "Scope Types:\n\n\
             none:\n\
             Evaluate Triggers: yes\n\
             Execute Effects: yes\n\
             Change Scopes: yes\n\
             Save Token: none\n\
             Stores Variables: no\n\n\
             character:\n\
             Evaluate Triggers: yes\n\
             Execute Effects: yes\n\
             Change Scopes: yes\n\
             Save Token: char\n\
             Stores Variables: yes\n",
        );
        assert_eq!(types.len(), 2);
        assert_eq!(types[0].name, "none");
        assert_eq!(types[0].save_token, None);
        assert_eq!(types[1].name, "character");
        assert_eq!(types[1].save_token.as_deref(), Some("char"));
        assert!(types[1].stores_variables);
    }

    #[test]
    fn modifiers_parses_tags_including_templated() {
        let mods = parse_modifiers(
            "Printing Modifier Definitions:\n\
             Tag: dynasty_opinion\n\
             Use areas: character\n\n\
             Tag: $MEN_AT_ARMS_TYPE$_pursuit_add\n\
             Use areas: character\n",
        );
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].tag, "dynasty_opinion");
        assert_eq!(mods[0].use_areas, vec!["character"]);
        assert_eq!(mods[1].tag, "$MEN_AT_ARMS_TYPE$_pursuit_add");
    }
}
