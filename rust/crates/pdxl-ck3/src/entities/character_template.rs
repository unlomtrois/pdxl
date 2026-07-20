//! Scripted character templates (`common/scripted_character_templates/`) —
//! top-level `NAME = { … }` definitions used to spawn a character with
//! `create_character = { template = X … }`.
//!
//! The `template` key is overloaded: under `create_artifact`/`reforge_artifact`
//! it names an *artifact* template. Corpus-validated, every character-template
//! reference sits inside a `create_character` block, so the rule is gated to
//! that key (KeyBlockField), which excludes the artifact uses cleanly.

use crate::kinds;
use pdxl_analysis::context::ClauseKind;
use pdxl_analysis::{DefShape, DefSource, IconHint, KindSpec, RefPattern, RefRule};

use super::Entity;

pub(crate) struct CharacterTemplate;

impl Entity for CharacterTemplate {
    const KINDS: &'static [KindSpec] = &[KindSpec {
        kind: kinds::SCRIPTED_CHARACTER_TEMPLATE,
        icon: IconHint::Object,
        defs: Some(DefSource {
            dir_prefix: "common/scripted_character_templates/",
            shape: DefShape::TopLevel,
        }),
        refs: &[RefRule {
            pattern: RefPattern::KeyBlockField("create_character", "template"),
            gate: None,
        }],
        aliases: &[],
    }];

    // A template body is a bundle of `create_character` parameters, so it reads
    // as the same documented structure — its fields complete and hover.
    const ROOTS: &'static [(&'static str, ClauseKind)] = &[(
        "common/scripted_character_templates/",
        ClauseKind::Struct(&super::create_character::CREATE_CHARACTER),
    )];
}
