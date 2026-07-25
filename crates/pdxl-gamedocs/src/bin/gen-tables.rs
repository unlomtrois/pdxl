//! Renders the game's script-documentation dumps into Rust source tables.
//!
//! Usage:
//!   cargo run -p pdxl-gamedocs --bin gen-tables -- \
//!     --logs "<paradox user dir>/Crusader Kings III/logs" \
//!     --out  crates/pdxl-ck3/src/tables
//!
//! The generated files are committed and reviewed like golden files; rerun
//! after a game patch and review the diff. Output is deterministic (rows
//! sorted by name, exact duplicates removed) so diffs stay minimal.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pdxl_gamedocs::{
    DataFnKind, is_markdown_doc, parse_data_types, parse_doc_log, parse_doc_log_md,
    parse_event_scopes, parse_event_targets, parse_event_targets_md, parse_modifiers,
    parse_modifiers_eu5,
};

fn main() -> ExitCode {
    let mut logs: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut data_types: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            eprintln!("missing value for {flag}");
            return ExitCode::from(2);
        };
        match flag.as_str() {
            "--logs" => logs = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            // EU5 splits the dumps: doc logs live in Documents/…/docs while
            // DumpDataTypes writes to logs/data_types. Defaults to
            // <logs>/data_types (the CK3 layout).
            "--data-types" => data_types = Some(PathBuf::from(value)),
            other => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(logs), Some(out)) = (logs, out) else {
        eprintln!(
            "usage: gen-tables --logs <doc logs dir> --out <tables dir> [--data-types <dir>]"
        );
        return ExitCode::from(2);
    };
    let data_types = data_types.unwrap_or_else(|| logs.join("data_types"));

    match generate(&logs, &data_types, &out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gen-tables: {e}");
            ExitCode::from(1)
        }
    }
}

fn generate(logs: &Path, data_types_dir: &Path, out: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(out)?;

    // The dumps are almost pure ASCII, but descriptions occasionally carry a
    // stray Windows-1252 byte (an em-dash in effects.log). Names never do, so
    // lossy decoding is safe for table generation.
    let read = |name: &str| -> std::io::Result<String> {
        let bytes = std::fs::read(logs.join(name))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    };

    // Dialect is detected per file: CK3-era plaintext stanzas vs EU5-era
    // Markdown (`# Title` header). Same for modifiers (`Use areas:` lines vs
    // one-line `Tag: X, Categories: …`).
    let doc = |text: &str| {
        if is_markdown_doc(text) {
            parse_doc_log_md(text)
        } else {
            parse_doc_log(text)
        }
    };
    let mut effects = doc(&read("effects.log")?);
    let mut triggers = doc(&read("triggers.log")?);
    let targets_text = read("event_targets.log")?;
    let (mut links, mut code_saved) = if is_markdown_doc(&targets_text) {
        parse_event_targets_md(&targets_text)
    } else {
        parse_event_targets(&targets_text)
    };
    // EU5 has no event_scopes.log; the scope-type table stays empty there.
    let scope_types = match read("event_scopes.log") {
        Ok(text) => parse_event_scopes(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    let modifiers_text = read("modifiers.log")?;
    let mut modifiers = if modifiers_text.lines().any(|l| l.contains(", Categories:")) {
        parse_modifiers_eu5(&modifiers_text)
    } else {
        parse_modifiers(&modifiers_text)
    };

    // The `DumpDataTypes` console dump writes several files into a
    // `data_types/` subdirectory; merge them all. Optional — older dumps may
    // not have run the command.
    let mut data_fns = Vec::new();
    if let Ok(entries) = std::fs::read_dir(data_types_dir) {
        let mut names: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        names.sort();
        for path in names {
            if path.extension().is_some_and(|e| e == "txt") {
                let bytes = std::fs::read(&path)?;
                data_fns.extend(parse_data_types(&String::from_utf8_lossy(&bytes)));
            }
        }
    }

    sort_dedup(&mut effects, |e| e.name.clone());
    sort_dedup(&mut triggers, |e| e.name.clone());
    sort_dedup(&mut links, |l| l.name.clone());
    sort_dedup(&mut modifiers, |m| m.tag.clone());
    sort_dedup(&mut data_fns, |d| {
        (d.owner.clone(), d.name.clone(), d.kind as u8)
    });
    code_saved.sort();
    code_saved.dedup();
    // Scope types keep dump order: it is a registry, not an alphabet.

    write_doc_table(&out.join("effects.rs"), "effects.log", "EFFECTS", &effects)?;
    write_doc_table(
        &out.join("triggers.rs"),
        "triggers.log",
        "TRIGGERS",
        &triggers,
    )?;
    write_links_table(&out.join("scope_links.rs"), &links, &code_saved)?;
    write_scope_types_table(&out.join("scope_types.rs"), &scope_types)?;
    write_modifiers_table(&out.join("modifiers.rs"), &modifiers)?;
    if !data_fns.is_empty() {
        write_data_fns_table(&out.join("data_types.rs"), &data_fns)?;
    }

    eprintln!(
        "generated: {} effects, {} triggers, {} scope links (+{} code-saved names), {} scope types, {} modifiers",
        effects.len(),
        triggers.len(),
        links.len(),
        code_saved.len(),
        scope_types.len(),
        modifiers.len()
    );
    eprintln!("generated: {} data-type entries", data_fns.len());
    Ok(())
}

fn write_data_fns_table(path: &Path, rows: &[pdxl_gamedocs::DataFnEntry]) -> std::io::Result<()> {
    let mut s = header("data_types/*.txt (DumpDataTypes console command)");
    s.push_str(
        "use super::{DataFnKind, DataFnRow};

#[rustfmt::skip]
",
    );
    let _ = writeln!(s, "pub const DATA_FNS: &[DataFnRow] = &[");
    for row in rows {
        let kind = match row.kind {
            DataFnKind::Type => "Type",
            DataFnKind::GlobalPromote => "GlobalPromote",
            DataFnKind::GlobalFunction => "GlobalFunction",
            DataFnKind::GlobalMacro => "GlobalMacro",
            DataFnKind::Promote => "Promote",
            DataFnKind::Function => "Function",
        };
        let _ = writeln!(
            s,
            "    DataFnRow {{ owner: {:?}, name: {:?}, kind: DataFnKind::{kind}, args: {}, ret: {:?}, desc: {:?} }},",
            row.owner, row.name, row.args, row.ret, row.description,
        );
    }
    s.push_str(
        "];
",
    );
    std::fs::write(path, s)
}

fn sort_dedup<T: PartialEq, K: Ord>(rows: &mut Vec<T>, key: impl Fn(&T) -> K) {
    rows.sort_by_key(&key);
    rows.dedup();
}

fn header(source: &str) -> String {
    format!(
        "//! GENERATED from the game's `{source}` dump — do not edit.\n\
         //! Regenerate: `cargo run -p pdxl-gamedocs --bin gen-tables -- --logs <logs dir> --out crates/pdxl-ck3/src/tables`\n\
         //! then review the diff like any other code change.\n\n"
    )
}

fn str_slice(values: &[String]) -> String {
    let items: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
    format!("&[{}]", items.join(", "))
}

fn write_doc_table(
    path: &Path,
    source: &str,
    const_name: &str,
    rows: &[pdxl_gamedocs::DocEntry],
) -> std::io::Result<()> {
    let mut s = header(source);
    s.push_str("use super::DocRow;\n\n#[rustfmt::skip]\n");
    let _ = writeln!(s, "pub const {const_name}: &[DocRow] = &[");
    for row in rows {
        let _ = writeln!(
            s,
            "    DocRow {{ name: {:?}, description: {:?}, scopes: {}, targets: {} }},",
            row.name,
            row.description,
            str_slice(&row.scopes),
            str_slice(&row.targets)
        );
    }
    s.push_str("];\n");
    std::fs::write(path, s)
}

fn write_links_table(
    path: &Path,
    links: &[pdxl_gamedocs::TargetLink],
    code_saved: &[String],
) -> std::io::Result<()> {
    let mut s = header("event_targets.log");
    s.push_str("use super::LinkRow;\n\n#[rustfmt::skip]\n");
    s.push_str("pub const SCOPE_LINKS: &[LinkRow] = &[\n");
    for l in links {
        let _ = writeln!(
            s,
            "    LinkRow {{ name: {:?}, requires_data: {}, global_link: {}, wildcard: {}, input_scopes: {}, output_scopes: {} }},",
            l.name,
            l.requires_data,
            l.global_link,
            l.wildcard,
            str_slice(&l.input_scopes),
            str_slice(&l.output_scopes)
        );
    }
    s.push_str("];\n\n");
    s.push_str(
        "/// Scope names the game engine itself saves (`scope:actor`, …).\n#[rustfmt::skip]\n",
    );
    let _ = writeln!(
        s,
        "pub const CODE_SAVED_SCOPES: &[&str] = {};",
        str_slice(code_saved)
    );
    std::fs::write(path, s)
}

fn write_scope_types_table(path: &Path, types: &[pdxl_gamedocs::ScopeType]) -> std::io::Result<()> {
    let mut s = header("event_scopes.log");
    s.push_str("use super::ScopeTypeRow;\n\n#[rustfmt::skip]\n");
    s.push_str("pub const SCOPE_TYPES: &[ScopeTypeRow] = &[\n");
    for t in types {
        let save = match &t.save_token {
            Some(tok) => format!("Some({tok:?})"),
            None => "None".to_string(),
        };
        let _ = writeln!(
            s,
            "    ScopeTypeRow {{ name: {:?}, evaluate_triggers: {}, execute_effects: {}, change_scopes: {}, save_token: {}, stores_variables: {} }},",
            t.name,
            t.evaluate_triggers,
            t.execute_effects,
            t.change_scopes,
            save,
            t.stores_variables
        );
    }
    s.push_str("];\n");
    std::fs::write(path, s)
}

fn write_modifiers_table(path: &Path, mods: &[pdxl_gamedocs::ModifierDef]) -> std::io::Result<()> {
    let mut s = header("modifiers.log");
    s.push_str("use super::ModifierRow;\n\n#[rustfmt::skip]\n");
    s.push_str("pub const MODIFIERS: &[ModifierRow] = &[\n");
    for m in mods {
        let _ = writeln!(
            s,
            "    ModifierRow {{ tag: {:?}, use_areas: {} }},",
            m.tag,
            str_slice(&m.use_areas)
        );
    }
    s.push_str("];\n");
    std::fs::write(path, s)
}
