//! `.gui` dialect parsing + template/type symbol extraction.

use pdxl_analysis::{GuiKinds, KindId};
use pdxl_gui::{GuiNames, gui_defs, gui_refs, parse};

const TEMPLATE: KindId = KindId::new("gui_template");
const TYPE: KindId = KindId::new("gui_type");
const KINDS: GuiKinds = GuiKinds {
    template: TEMPLATE,
    ty: TYPE,
};

const SRC: &str = r#"
template MyHeader {
    size = { 100% 34 }
    text_single = {
        text = "SOME_LOC_KEY"
        align = nobaseline
    }
}

types MyTypes {
    type my_marker = widget {
        parentanchor = bottom|left
        block "marker_label" {}
    }
    type my_button = button_std {
        enabled = [ArmyWindow.CanMerge]
        onclick = "[OpenGameViewData( 'x', Decision.Self )]"
    }
}

window = {
    using = MyHeader
    my_marker = {
        blockoverride "marker_label" {}
    }
    inner = {
        using = MyHeader
        my_button = {}
    }
}
"#;

fn facts_and_names() -> (pdxl_analysis::FileFacts, GuiNames) {
    let parsed = parse("gui/test.gui", SRC.as_bytes().to_vec());
    assert!(
        parsed.diagnostics().is_empty(),
        "dialect source must parse clean: {:?}",
        parsed.diagnostics()
    );
    let facts = gui_defs(parsed.tree(), "gui/test.gui", KINDS);
    let mut names = GuiNames::default();
    names.add_facts(&facts, KINDS);
    (facts, names)
}

#[test]
fn defs_templates_and_types() {
    let (facts, _) = facts_and_names();
    let defs: Vec<(&str, KindId)> = facts
        .defs
        .iter()
        .map(|d| (d.name.as_str(), d.kind))
        .collect();
    // `types MyTypes` is a grouping, not a symbol; its member `type` defs are.
    assert_eq!(
        defs,
        vec![
            ("MyHeader", TEMPLATE),
            ("my_marker", TYPE),
            ("my_button", TYPE),
        ]
    );
    // The def range covers the name, so go-to-definition lands on it.
    let d = &facts.defs[0];
    assert_eq!(&SRC[d.offset as usize..d.end_offset as usize], "MyHeader");
}

#[test]
fn refs_using_and_instantiations_name_gated() {
    let (_, names) = facts_and_names();
    let parsed = parse("gui/test.gui", SRC.as_bytes().to_vec());
    let refs = gui_refs(parsed.tree(), "gui/test.gui", &names, KINDS);
    let got: Vec<(&str, KindId)> = refs.iter().map(|r| (r.name.as_str(), r.kind)).collect();
    // Two `using = MyHeader`, plus the `my_marker`/`my_button` instantiations.
    // The `type … = widget/button_std` bases and `window`/`inner`/`size`
    // fields are not defined names, so they are silently skipped — and the
    // definition fields themselves never self-reference.
    assert_eq!(
        got,
        vec![
            ("MyHeader", TEMPLATE),
            ("my_marker", TYPE),
            ("MyHeader", TEMPLATE),
            ("my_button", TYPE),
        ]
    );
}

#[test]
fn type_base_ref_resolves_when_defined() {
    // A `type` whose base is itself a defined type records a base reference.
    let src = "types T {\n\ttype base_one = widget {}\n\ttype derived = base_one {}\n}\n";
    let parsed = parse("gui/base.gui", src.as_bytes().to_vec());
    let facts = gui_defs(parsed.tree(), "gui/base.gui", KINDS);
    let mut names = GuiNames::default();
    names.add_facts(&facts, KINDS);
    let refs = gui_refs(parsed.tree(), "gui/base.gui", &names, KINDS);
    let got: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(got, vec!["base_one"], "only the defined base is a ref");
}

#[test]
fn unclosed_datafunction_diagnosed() {
    let parsed = parse("gui/bad.gui", b"x = { enabled = [Foo.Bar }\n".to_vec());
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|d| d.message.contains("unclosed datafunction")),
        "{:?}",
        parsed.diagnostics()
    );
}

#[test]
fn script_parser_unchanged_rejects_brackets() {
    // The script grammar stays a Go-parity target: `[` is still an error there.
    let parsed = pdxl_parser::parse("x.txt", b"a = [Foo.Bar]\n".to_vec());
    assert!(!parsed.diagnostics().is_empty());
}
