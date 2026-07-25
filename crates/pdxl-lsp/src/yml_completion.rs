//! Completion for Paradox localization values.

use lsp_types::{CompletionItem, CompletionItemKind, CompletionTextEdit, Range, TextEdit};
use pdxl_analysis::LOC_KEY;
use pdxl_project::Project;

use crate::position::offset_to_position;

pub(crate) fn items(project: &Project, src: &[u8], off: u32) -> Vec<CompletionItem> {
    if let Some((start, prefix)) = open_loc_reference(src, off as usize) {
        let range = Range::new(
            offset_to_position(src, start as u32),
            offset_to_position(src, off),
        );
        return project
            .table()
            .iter()
            .filter(|symbol| symbol.kind == LOC_KEY && symbol.name.starts_with(prefix))
            .map(|symbol| CompletionItem {
                label: symbol.name.clone(),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: Some("localization key".to_string()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: symbol.name.clone(),
                })),
                ..CompletionItem::default()
            })
            .collect();
    }

    // The datafunction expression syntax is shared with `.gui`; reuse its
    // chain-aware global/member completion and dumped-table documentation.
    if inside_open_bracket(src, off as usize) {
        return crate::gui_completion::datafn_completion(src, off);
    }
    Vec::new()
}

fn open_loc_reference(src: &[u8], off: usize) -> Option<(usize, &str)> {
    let line_start = src[..off]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let dollar = src[line_start..off].iter().rposition(|&b| b == b'$')? + line_start;
    // An even number of delimiters means the last `$` closed a reference.
    if src[line_start..dollar]
        .iter()
        .filter(|&&b| b == b'$')
        .count()
        % 2
        == 1
    {
        return None;
    }
    let start = dollar + 1;
    let prefix = std::str::from_utf8(&src[start..off]).ok()?;
    prefix
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        .then_some((start, prefix))
}

fn inside_open_bracket(src: &[u8], off: usize) -> bool {
    let line_start = src[..off]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let Some(open) = src[line_start..off].iter().rposition(|&b| b == b'[') else {
        return false;
    };
    !src[line_start + open..off].contains(&b']')
}
