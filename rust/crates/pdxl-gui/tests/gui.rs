//! `.gui` dialect parsing + template/type symbol extraction.

use pdxl_analysis::{GuiKinds, KindId};
use pdxl_gui::{GuiNames, gui_defs, gui_refs, parse};

const TEMPLATE: KindId = KindId::new("gui_template");
const TYPE: KindId = KindId::new("gui_type");
const SGUI: KindId = KindId::new("scripted_gui");
const KINDS: GuiKinds = GuiKinds {
    template: TEMPLATE,
    ty: TYPE,
    arg_refs: &[("GetScriptedGui", SGUI)],
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

// ── datafunctions ───────────────────────────────────────────────────────────

use pdxl_gui::datafn::{
    DataFnKind, DataFnRegistry, DataFnRow, datafn_spans, parse_chain, resolve_chain,
    validate_datafns,
};

static ROWS: &[DataFnRow] = &[
    DataFnRow {
        owner: "",
        name: "Character",
        kind: DataFnKind::Type,
        args: 0,
        ret: "Character",
        desc: "",
    },
    DataFnRow {
        owner: "",
        name: "Title",
        kind: DataFnKind::Type,
        args: 0,
        ret: "Title",
        desc: "",
    },
    DataFnRow {
        owner: "",
        name: "GetPlayer",
        kind: DataFnKind::GlobalPromote,
        args: 0,
        ret: "Character",
        desc: "The local player.",
    },
    DataFnRow {
        owner: "",
        name: "GetTitleByKey",
        kind: DataFnKind::GlobalFunction,
        args: 1,
        ret: "Title",
        desc: "",
    },
    DataFnRow {
        owner: "Character",
        name: "GetLiege",
        kind: DataFnKind::Promote,
        args: 0,
        ret: "Character",
        desc: "",
    },
    DataFnRow {
        owner: "Character",
        name: "GetUIName",
        kind: DataFnKind::Function,
        args: 0,
        ret: "CString",
        desc: "",
    },
    DataFnRow {
        owner: "Title",
        name: "GetHolder",
        kind: DataFnKind::Promote,
        args: 0,
        ret: "Character",
        desc: "",
    },
    DataFnRow {
        owner: "Character",
        name: "MakeScope",
        kind: DataFnKind::Function,
        args: 0,
        ret: "[unregistered]",
        desc: "",
    },
];

fn registry() -> DataFnRegistry {
    DataFnRegistry::from_rows(ROWS)
}

fn errors_in(src: &str) -> Vec<String> {
    let parsed = parse("gui/x.gui", src.as_bytes().to_vec());
    validate_datafns(parsed.tree(), &registry())
        .into_iter()
        .map(|e| e.msg)
        .collect()
}

#[test]
fn datafn_valid_chains_pass() {
    let src = "w = {\n\
         \tenabled = [GetPlayer.GetLiege.GetUIName]\n\
         \tdatacontext = \"[GetTitleByKey( 'k_x' ).GetHolder]\"\n\
         \ttext = \"Ruler: [Character.GetUIName] the great\"\n\
         \traw_text = \"[GetPlayer.MakeScope.ScriptValue('v')|0]\"\n\
         }\n";
    assert_eq!(errors_in(src), Vec::<String>::new());
    // MakeScope returns [unregistered]: the ScriptValue tail is accepted
    // silently, not resolved.
}

#[test]
fn datafn_unknown_root_and_member_flagged() {
    let src = "w = {\n\
         \tenabled = [GetPlyer.GetLiege]\n\
         \ttext = \"[GetPlayer.GetLeige]\"\n\
         }\n";
    let errs = errors_in(src);
    assert_eq!(errs.len(), 2, "{errs:?}");
    assert!(errs[0].contains("unknown datafunction \"GetPlyer\""));
    assert!(errs[1].contains("\"GetLeige\" is not a member of Character"));
}

#[test]
fn datafn_spans_and_segments() {
    let src = "w = { text = \"a [GetPlayer.GetUIName|U] b [[literal]\" }\n";
    let parsed = parse("gui/x.gui", src.as_bytes().to_vec());
    let spans = datafn_spans(parsed.tree());
    // `[[` escapes; only the real expression is found.
    assert_eq!(spans.len(), 1, "{spans:?}");
    let text = &src.as_bytes()[spans[0].start as usize..spans[0].end as usize];
    let segs = parse_chain(text, spans[0].start).unwrap();
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].name, "GetPlayer");
    assert_eq!(segs[1].name, "GetUIName");
    // Name spans point into the file (usable for hover/diagnostics).
    assert_eq!(
        &src[segs[1].name_start as usize..segs[1].name_end as usize],
        "GetUIName"
    );
    let (resolved, err) = resolve_chain(&segs, &registry());
    assert!(err.is_none());
    assert_eq!(resolved[1].row.unwrap().ret, "CString");
}

#[test]
fn datafn_arg_refs_extracted() {
    let src = "window = {\n\
         \tvisible = \"[GetScriptedGui('can_pledge').IsShown( GuiScope.SetRoot( GetPlayer.MakeScope ).End )]\"\n\
         \tonclick = [GetScriptedGui('can_pledge').Execute( GuiScope.End )]\n\
         }\n";
    let parsed = parse("gui/x.gui", src.as_bytes().to_vec());
    let names = GuiNames::default();
    let refs = gui_refs(parsed.tree(), "gui/x.gui", &names, KINDS);
    let got: Vec<(&str, KindId)> = refs.iter().map(|r| (r.name.as_str(), r.kind)).collect();
    assert_eq!(got, vec![("can_pledge", SGUI), ("can_pledge", SGUI)]);
    // Spans cover exactly the quoted name.
    let r = &refs[0];
    assert_eq!(&src[r.start as usize..r.end as usize], "can_pledge");
}
