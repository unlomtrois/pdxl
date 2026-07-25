//! Agent-facing semantic queries over generated CK3 documentation.

use std::collections::HashSet;

use pdxl_game::tables::{EFFECTS, MODIFIERS, SCOPE_LINKS, TRIGGERS};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 50;

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptItemKind {
    Effect,
    Trigger,
    Modifier,
    ScopeLink,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchScriptItemsParams {
    /// Words or a script key to find in names and game-provided documentation.
    pub query: String,
    /// Restrict results to these item kinds. Omit to search every kind.
    #[serde(default)]
    pub kinds: Vec<ScriptItemKind>,
    /// Restrict effects, triggers, and scope links to this input scope.
    pub input_scope: Option<String>,
    /// Maximum number of results. Defaults to 10 and is capped at 50.
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct ScriptItemMatch {
    pub kind: ScriptItemKind,
    pub name: String,
    pub description: String,
    pub input_scopes: Vec<String>,
    pub output_scopes: Vec<String>,
    pub score: u32,
    pub source: String,
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct SearchScriptItemsResult {
    pub query: String,
    pub input_scope: Option<String>,
    pub matches: Vec<ScriptItemMatch>,
}

/// Search the generated CK3 documentation with deterministic lexical ranking.
pub fn search_script_items(params: SearchScriptItemsParams) -> SearchScriptItemsResult {
    let query = params.query.trim().to_lowercase();
    let tokens: Vec<&str> = query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|token| !token.is_empty())
        .collect();
    let kinds: HashSet<_> = params.kinds.iter().copied().collect();
    let accepts_kind = |kind| kinds.is_empty() || kinds.contains(&kind);
    let scope = params.input_scope.as_deref();
    let mut matches = Vec::new();

    if accepts_kind(ScriptItemKind::Effect) {
        for row in EFFECTS {
            if scope_compatible(scope, row.scopes)
                && let Some(score) = lexical_score(&query, &tokens, row.name, row.description)
            {
                matches.push(ScriptItemMatch {
                    kind: ScriptItemKind::Effect,
                    name: row.name.to_string(),
                    description: row.description.to_string(),
                    input_scopes: strings(row.scopes),
                    output_scopes: strings(row.targets),
                    score,
                    source: "effects.log".to_string(),
                });
            }
        }
    }
    if accepts_kind(ScriptItemKind::Trigger) {
        for row in TRIGGERS {
            if scope_compatible(scope, row.scopes)
                && let Some(score) = lexical_score(&query, &tokens, row.name, row.description)
            {
                matches.push(ScriptItemMatch {
                    kind: ScriptItemKind::Trigger,
                    name: row.name.to_string(),
                    description: row.description.to_string(),
                    input_scopes: strings(row.scopes),
                    output_scopes: strings(row.targets),
                    score,
                    source: "triggers.log".to_string(),
                });
            }
        }
    }
    if accepts_kind(ScriptItemKind::Modifier) {
        for row in MODIFIERS {
            let description = format!("Modifier use areas: {}", row.use_areas.join(", "));
            if let Some(score) = lexical_score(&query, &tokens, row.tag, &description) {
                matches.push(ScriptItemMatch {
                    kind: ScriptItemKind::Modifier,
                    name: row.tag.to_string(),
                    description,
                    input_scopes: Vec::new(),
                    output_scopes: Vec::new(),
                    score,
                    source: "modifiers.log".to_string(),
                });
            }
        }
    }
    if accepts_kind(ScriptItemKind::ScopeLink) {
        for row in SCOPE_LINKS {
            if (scope.is_none() || row.global_link || scope_compatible(scope, row.input_scopes))
                && let Some(score) = lexical_score(&query, &tokens, row.name, "scope transition")
            {
                matches.push(ScriptItemMatch {
                    kind: ScriptItemKind::ScopeLink,
                    name: row.name.to_string(),
                    description: if row.requires_data {
                        "Scope transition requiring a :data argument".to_string()
                    } else {
                        "Scope transition".to_string()
                    },
                    input_scopes: strings(row.input_scopes),
                    output_scopes: strings(row.output_scopes),
                    score,
                    source: "event_targets.log".to_string(),
                });
            }
        }
    }

    matches.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| kind_rank(a.kind).cmp(&kind_rank(b.kind)))
    });
    matches.truncate(params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT));
    SearchScriptItemsResult {
        query: params.query,
        input_scope: params.input_scope,
        matches,
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn scope_compatible(requested: Option<&str>, supported: &[&str]) -> bool {
    requested.is_none()
        || supported.contains(&"none")
        || requested.is_some_and(|scope| supported.contains(&scope))
}

fn lexical_score(query: &str, tokens: &[&str], name: &str, description: &str) -> Option<u32> {
    if query.is_empty() {
        return None;
    }
    let name = name.to_lowercase();
    let description = description.to_lowercase();
    let mut score = if name == query {
        10_000
    } else if name.starts_with(query) {
        5_000
    } else if name.contains(query) {
        2_500
    } else {
        0
    };
    for token in tokens {
        if name == *token {
            score += 800;
        } else if name.contains(token) {
            score += 400;
        }
        if description.contains(token) {
            score += 100;
        }
    }
    (score > 0).then_some(score)
}

const fn kind_rank(kind: ScriptItemKind) -> u8 {
    match kind {
        ScriptItemKind::Effect => 0,
        ScriptItemKind::Trigger => 1,
        ScriptItemKind::Modifier => 2,
        ScriptItemKind::ScopeLink => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_names_rank_first() {
        let result = search_script_items(SearchScriptItemsParams {
            query: "add_gold".into(),
            kinds: vec![ScriptItemKind::Effect],
            input_scope: Some("character".into()),
            limit: None,
        });
        assert_eq!(result.matches[0].name, "add_gold");
        assert_eq!(result.matches[0].kind, ScriptItemKind::Effect);
    }

    #[test]
    fn scope_filter_excludes_incompatible_items() {
        let result = search_script_items(SearchScriptItemsParams {
            query: "title color".into(),
            kinds: vec![ScriptItemKind::Effect],
            input_scope: Some("character".into()),
            limit: Some(50),
        });
        assert!(result.matches.iter().all(|item| {
            item.input_scopes
                .iter()
                .any(|scope| scope == "none" || scope == "character")
        }));
        assert!(
            !result
                .matches
                .iter()
                .any(|item| item.name == "set_title_color")
        );
    }

    #[test]
    fn searches_scope_links_from_an_input_scope() {
        let result = search_script_items(SearchScriptItemsParams {
            query: "holder".into(),
            kinds: vec![ScriptItemKind::ScopeLink],
            input_scope: Some("landed_title".into()),
            limit: None,
        });
        assert!(
            result
                .matches
                .iter()
                .any(|item| { item.name == "holder" && item.output_scopes == ["character"] })
        );
    }

    #[test]
    fn clamps_result_limit() {
        let result = search_script_items(SearchScriptItemsParams {
            query: "has".into(),
            kinds: vec![],
            input_scope: None,
            limit: Some(usize::MAX),
        });
        assert_eq!(result.matches.len(), MAX_LIMIT);
    }
}
