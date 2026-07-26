//! `textDocument/documentColor` / `colorPresentation` — the inline color
//! swatch + picker. Standard LSP (3.6+), rendered natively by VS Code.
//!
//! Color positions come from the structural-context layer: a block whose
//! field resolves to [`ClauseKind::Color`] is a color literal. Supported
//! forms (all corpus-real):
//!
//! - `{ 220 45 120 }` — implicit RGB; components > 1 mean the 0–255 scale,
//!   otherwise 0–1 floats.
//! - `rgb { 225 35 40 }` — 0–255 (floats ≤ 1 kept as-is).
//! - `hsv { 0.1 0.5 0.8 }` — 0–1 floats.
//! - `hsv360 { 21 74 45 }` — hue 0–360, saturation/value 0–100.
//! - An optional 4th component is alpha.
//!
//! Presentations preserve the author's original form: picking a new color in
//! an `hsv { … }` block writes `hsv { … }` back.

use lsp_types::Color;
use pdxl_analysis::context::{ClauseKind, ContextSchema, resolve_key};
use pdxl_ast::{NodeId, NodeKind, SyntaxTree};

/// A color literal found in a file: its byte span (tag/brace to closing
/// brace, inclusive) and the decoded RGBA value.
pub(crate) struct ColorSpan {
    pub start: u32,
    pub end: u32,
    pub color: Color,
}

/// Every color literal in a parsed file, located via the context schema.
pub(crate) fn document_colors(
    tree: &SyntaxTree,
    src: &[u8],
    rel_path: &str,
    schema: &ContextSchema,
) -> Vec<ColorSpan> {
    let Some(&(_, body_kind)) = schema
        .roots
        .iter()
        .find(|(prefix, _)| rel_path.starts_with(prefix))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // Top-level `NAME = { body }` opens the directory's body kind
    // (mirroring `context_at`); everything below threads `resolve_key`.
    for item in tree.children(tree.root()) {
        let node = tree.node(item);
        if node.kind != NodeKind::Field {
            continue;
        }
        let kids = tree.child_ids(item);
        if kids.len() == 2
            && matches!(
                tree.node(kids[1]).kind,
                NodeKind::Block | NodeKind::TaggedBlock
            )
        {
            walk(tree, kids[1], body_kind, src, &mut out);
        }
    }
    out
}

fn walk(tree: &SyntaxTree, node_id: NodeId, ctx: ClauseKind, src: &[u8], out: &mut Vec<ColorSpan>) {
    for item in tree.children(node_id) {
        let node = tree.node(item);
        if node.kind != NodeKind::Field {
            continue;
        }
        let kids = tree.child_ids(item);
        if kids.len() != 2 {
            continue;
        }
        let value_id = kids[1];
        let is_block = matches!(
            tree.node(value_id).kind,
            NodeKind::Block | NodeKind::TaggedBlock
        );
        let Ok(key) = std::str::from_utf8(tree.node_text(kids[0])) else {
            continue;
        };
        let next = resolve_key(ctx, key, is_block);
        if !is_block {
            continue;
        }
        if next == ClauseKind::Color {
            if let Some(span) = parse_color(tree, value_id, src) {
                out.push(span);
            }
        } else {
            walk(tree, value_id, next, src, out);
        }
    }
}

/// Decodes one color block node into a [`ColorSpan`], or `None` when the
/// block isn't a plain numeric color literal (macros, wrong arity, unknown
/// tag, empty block).
fn parse_color(tree: &SyntaxTree, value_id: NodeId, src: &[u8]) -> Option<ColorSpan> {
    let node = tree.node(value_id);
    let mut comps: Vec<f32> = Vec::new();
    let mut first_start: Option<u32> = None;
    let mut last_end: u32 = 0;
    for child in tree.children(value_id) {
        let c = tree.node(child);
        if c.kind != NodeKind::Scalar {
            return None;
        }
        comps.push(
            std::str::from_utf8(tree.node_text(child))
                .ok()?
                .parse()
                .ok()?,
        );
        first_start.get_or_insert(c.range.start);
        last_end = c.range.end;
    }
    if comps.len() != 3 && comps.len() != 4 {
        return None;
    }

    let tag = (node.kind == NodeKind::TaggedBlock).then(|| tree.node_text(value_id));
    let [a, b, c] = [comps[0], comps[1], comps[2]];
    let (red, green, blue) = match tag {
        None | Some(b"rgb") => {
            // Components > 1 mean the 0–255 integer scale.
            if comps[..3].iter().any(|&v| v > 1.0) {
                (a / 255.0, b / 255.0, c / 255.0)
            } else {
                (a, b, c)
            }
        }
        Some(b"hsv") => hsv_to_rgb(a, b, c),
        Some(b"hsv360") => hsv_to_rgb(a / 360.0, b / 100.0, c / 100.0),
        Some(_) => return None,
    };
    let alpha = match comps.get(3) {
        Some(&v) if v > 1.0 => v / 255.0,
        Some(&v) => v,
        None => 1.0,
    };

    // The replaceable span: from the tag (tagged form) or the opening brace
    // (plain form, found just left of the first component) to the closing
    // brace right of the last component.
    let start = match tag {
        Some(_) => node.range.start,
        None => scan_left_to(src, first_start?, b'{')?,
    };
    let end = scan_right_to(src, last_end, b'}')? + 1;
    Some(ColorSpan {
        start,
        end,
        color: Color {
            red: red.clamp(0.0, 1.0),
            green: green.clamp(0.0, 1.0),
            blue: blue.clamp(0.0, 1.0),
            alpha: alpha.clamp(0.0, 1.0),
        },
    })
}

/// Renders `color` in the same textual form as the original literal at
/// `original` (the text the presentation's edit will replace).
pub(crate) fn present(original: &[u8], color: &Color) -> String {
    let text = String::from_utf8_lossy(original);
    let trimmed = text.trim_start();
    let had_alpha = trimmed
        .trim_start_matches(|c: char| c.is_ascii_alphanumeric())
        .trim_matches(['{', '}', ' ', '\t'].as_ref())
        .split_whitespace()
        .count()
        == 4;
    let (h, s, v) = rgb_to_hsv(color.red, color.green, color.blue);

    if trimmed.starts_with("hsv360") {
        let mut out = format!(
            "hsv360 {{ {:.0} {:.0} {:.0}",
            h * 360.0,
            s * 100.0,
            v * 100.0
        );
        if had_alpha {
            out.push_str(&format!(" {}", fmt_f(color.alpha)));
        }
        out.push_str(" }");
        out
    } else if trimmed.starts_with("hsv") {
        let mut out = format!("hsv {{ {} {} {}", fmt_f(h), fmt_f(s), fmt_f(v));
        if had_alpha {
            out.push_str(&format!(" {}", fmt_f(color.alpha)));
        }
        out.push_str(" }");
        out
    } else {
        let ints = trimmed.starts_with("rgb") || !trimmed.contains('.');
        let one = |x: f32| {
            if ints {
                format!("{:.0}", x * 255.0)
            } else {
                fmt_f(x)
            }
        };
        let prefix = if trimmed.starts_with("rgb") {
            "rgb "
        } else {
            ""
        };
        let mut out = format!(
            "{prefix}{{ {} {} {}",
            one(color.red),
            one(color.green),
            one(color.blue)
        );
        if had_alpha {
            out.push_str(&format!(" {}", fmt_f(color.alpha)));
        }
        out.push_str(" }");
        out
    }
}

/// A float with up to 3 decimals, trailing zeros trimmed (`0.5`, not `0.500`).
fn fmt_f(x: f32) -> String {
    let s = format!("{x:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = (h.rem_euclid(1.0)) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as u32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        (((g - b) / d).rem_euclid(6.0)) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

/// Scans left from `from` over whitespace to the byte `target`; its offset.
fn scan_left_to(src: &[u8], from: u32, target: u8) -> Option<u32> {
    let mut i = from as usize;
    while i > 0 {
        i -= 1;
        match src[i] {
            b if b == target => return Some(i as u32),
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            _ => return None,
        }
    }
    None
}

/// Scans right from `from` over whitespace to the byte `target`; its offset.
fn scan_right_to(src: &[u8], from: u32, target: u8) -> Option<u32> {
    let mut i = from as usize;
    while i < src.len() {
        match src[i] {
            b if b == target => return Some(i as u32),
            b' ' | b'\t' | b'\r' | b'\n' => {
                i += 1;
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors_in(src: &str, rel: &str) -> Vec<(String, Color)> {
        let (tree, _) =
            pdxl_parser::parse("test".to_string(), src.as_bytes().to_vec()).into_parts();
        document_colors(
            &tree,
            src.as_bytes(),
            rel,
            pdxl_game::contexts::context_schema(),
        )
        .into_iter()
        .map(|s| (src[s.start as usize..s.end as usize].to_string(), s.color))
        .collect()
    }

    #[test]
    fn finds_all_three_literal_forms() {
        let src = "hills = {\n\
                   \tcolor = hsv { 0.0 1.0 1.0 }\n\
                   \ttravel_danger_color = { 255 0 0 }\n\
                   }\n";
        let found = colors_in(src, "common/terrain_types/00.txt");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, "hsv { 0.0 1.0 1.0 }");
        assert_eq!(found[1].0, "{ 255 0 0 }");
        // Both are pure red.
        for (_, c) in &found {
            assert!((c.red - 1.0).abs() < 1e-4 && c.green < 1e-4 && c.blue < 1e-4);
        }
    }

    #[test]
    fn named_colors_container_and_rgb_tag() {
        let src = "colors = {\n\
                   \twhite = rgb { 255 255 255 }\n\
                   \thalf = { 0.5 0.5 0.5 }\n\
                   }\n";
        let found = colors_in(src, "common/named_colors/x.txt");
        assert_eq!(found.len(), 2);
        assert!((found[0].1.red - 1.0).abs() < 1e-4);
        assert!((found[1].1.green - 0.5).abs() < 1e-4);
    }

    #[test]
    fn non_color_blocks_and_macros_are_skipped() {
        let src = "hills = {\n\
                   \tprovince_modifier = { supply_limit_mult = 1 }\n\
                   \tcolor = hsv { 0.1 $S$ 0.8 }\n\
                   }\n";
        assert!(colors_in(src, "common/terrain_types/00.txt").is_empty());
    }

    #[test]
    fn presentation_preserves_original_form() {
        let red = Color {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        };
        assert_eq!(present(b"hsv { 0.5 0.2 0.3 }", &red), "hsv { 0 1 1 }");
        assert_eq!(
            present(b"hsv360 { 100 50 50 }", &red),
            "hsv360 { 0 100 100 }"
        );
        assert_eq!(present(b"rgb { 10 20 30 }", &red), "rgb { 255 0 0 }");
        assert_eq!(present(b"{ 10 20 30 }", &red), "{ 255 0 0 }");
        assert_eq!(present(b"{ 0.1 0.2 0.3 }", &red), "{ 1 0 0 }");
        // Alpha kept when the original had one.
        assert_eq!(present(b"hsv { 0.0 0.0 0.1 0 }", &red), "hsv { 0 1 1 1 }");
    }

    #[test]
    #[ignore = "needs PDXL_EU5_GAME; performance regression benchmark"]
    fn eu5_named_colors_benchmark() {
        let game = std::env::var("PDXL_EU5_GAME").expect("PDXL_EU5_GAME");
        let path = std::path::Path::new(&game).join("main_menu/common/named_colors/02_map.txt");
        let src = std::fs::read(&path).expect("read 02_map.txt");
        let started = std::time::Instant::now();
        let (tree, _) = pdxl_parser::parse(path.display().to_string(), src.clone()).into_parts();
        let parsed = started.elapsed();
        let started = std::time::Instant::now();
        let colors = document_colors(
            &tree,
            &src,
            "main_menu/common/named_colors/02_map.txt",
            pdxl_game::contexts::context_schema(),
        );
        let scanned = started.elapsed();
        eprintln!(
            "{} colors: parse={parsed:?}, scan={scanned:?}",
            colors.len()
        );
        assert!(
            colors.len() > 500,
            "fixture must exceed VS Code's default editor.colorDecoratorsLimit"
        );
    }

    #[test]
    fn hsv_round_trip() {
        for (r, g, b) in [(0.8, 0.2, 0.2), (0.0, 0.0, 0.1), (0.3, 0.6, 0.6)] {
            let (h, s, v) = rgb_to_hsv(r, g, b);
            let (r2, g2, b2) = hsv_to_rgb(h, s, v);
            assert!((r - r2).abs() < 1e-4 && (g - g2).abs() < 1e-4 && (b - b2).abs() < 1e-4);
        }
    }
}
