//! Animations selectable via an event portrait's `animation = X`. Two
//! directories define them and both are referenced identically, so they share
//! one resolvable kind:
//! - `gfx/portraits/portrait_animations/` — portrait animations proper;
//! - `common/scripted_animations/` — scripted animations (`bow_closed`, …).
//!
//! The `animation` key is overloaded: in an event portrait it names an
//! animation, but in `common/tutorial_lessons/` and `gfx/court_scene/` it holds
//! a camera *position* (`center`, `left`, …). Corpus-validated, every real
//! animation reference lives under `events/`, so the rule is gated there — which
//! excludes both overloaded uses cleanly.

use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule, SymbolKind};

pub(crate) struct PortraitAnimation;

impl super::Entity for PortraitAnimation {
    const KINDS: &'static [KindSpec] = &[
        // Portrait animations — carries the `animation = X` reference rule.
        KindSpec {
            kind: SymbolKind::PortraitAnimation,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: "gfx/portraits/portrait_animations/",
                shape: DefShape::TopLevel,
            }),
            refs: &[RefRule {
                pattern: RefPattern::KeyValue("animation"),
                gate: Some("events/"),
            }],
            aliases: &[],
        },
        // Scripted animations — same kind, so `animation = X` resolves against
        // them too (defs only; the reference rule above is shared).
        KindSpec {
            kind: SymbolKind::PortraitAnimation,
            icon: IconHint::Object,
            defs: Some(DefSource {
                dir_prefix: "common/scripted_animations/",
                shape: DefShape::TopLevel,
            }),
            refs: &[],
            aliases: &[],
        },
    ];

    const ROOTS: &'static [(&'static str, ClauseKind)] = &[];
}
