//! Curated entity documentation embedded per game feature.
//!
//! Unlike the generated tables behind `search_script_items`, these are
//! hand-written knowledge bases (`docs/<game>/*.md`): what a game system is,
//! how its pieces reference each other, where the game's own `_*.info` docs
//! and the shipped files disagree, and the conventions the corpus enforces
//! that are written down nowhere. Served both as MCP resources
//! (`docs://<entity>`) and through the `get_entity_docs` tool.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One embedded knowledge base.
pub struct EntityDoc {
    /// Stable lookup name (`docs://<name>`).
    pub name: &'static str,
    /// Human-readable title.
    pub title: &'static str,
    /// One-line summary shown in listings.
    pub summary: &'static str,
    /// The full markdown document.
    pub markdown: &'static str,
}

#[cfg(feature = "ck3")]
pub const ENTITY_DOCS: &[EntityDoc] = &[EntityDoc {
    name: "activities",
    title: "CK3 activities (common/activities/)",
    summary: "The six activity databases: types, phases, intents, pulse actions, \
              locales, invite rules, group types — cross-reference wiring, \
              corpus-vs-info gaps, unwritten conventions, pitfalls, a skeleton.",
    markdown: include_str!("../docs/ck3/activities.md"),
}];

#[cfg(not(feature = "ck3"))]
pub const ENTITY_DOCS: &[EntityDoc] = &[];

/// Looks up a doc by name, case-insensitively.
pub fn find(name: &str) -> Option<&'static EntityDoc> {
    ENTITY_DOCS
        .iter()
        .find(|doc| doc.name.eq_ignore_ascii_case(name.trim()))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEntityDocsParams {
    /// The entity to document (e.g. "activities"). Omit to list what exists.
    pub entity: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct EntityDocSummary {
    pub name: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct GetEntityDocsResult {
    /// The game compiled into this server.
    pub game: String,
    /// Every available doc, always listed (it is small).
    pub available: Vec<EntityDocSummary>,
    /// The requested entity, echoed back when one was asked for.
    pub entity: Option<String>,
    /// The full document, when the requested entity exists.
    pub markdown: Option<String>,
}

/// Serves the doc index, or one document plus the index.
pub fn get_entity_docs(params: GetEntityDocsParams) -> GetEntityDocsResult {
    let available = ENTITY_DOCS
        .iter()
        .map(|doc| EntityDocSummary {
            name: doc.name.to_string(),
            title: doc.title.to_string(),
            summary: doc.summary.to_string(),
        })
        .collect();
    let markdown = params
        .entity
        .as_deref()
        .and_then(find)
        .map(|doc| doc.markdown.to_string());
    GetEntityDocsResult {
        game: pdxl_game::GAME.to_string(),
        available,
        entity: params.entity,
        markdown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_stable_lookup_keys() {
        for doc in ENTITY_DOCS {
            assert!(
                doc.name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "doc name {:?} is not a stable lookup key",
                doc.name
            );
            assert!(!doc.markdown.is_empty());
            assert!(!doc.summary.is_empty());
        }
    }

    #[cfg(feature = "ck3")]
    #[test]
    fn ck3_serves_the_activities_doc() {
        let result = get_entity_docs(GetEntityDocsParams {
            entity: Some("Activities".into()),
        });
        let markdown = result.markdown.expect("activities doc is embedded");
        assert!(markdown.contains("common/activities/"));
        assert!(result.available.iter().any(|d| d.name == "activities"));
    }

    #[test]
    fn unknown_entity_still_lists_available_docs() {
        let result = get_entity_docs(GetEntityDocsParams {
            entity: Some("no_such_entity".into()),
        });
        assert_eq!(result.markdown, None);
        assert_eq!(result.available.len(), ENTITY_DOCS.len());
    }
}
