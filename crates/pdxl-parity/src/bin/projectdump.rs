//! Whole-project analysis dump tool (Rust side), mirroring
//! `tools/projectdump/main.go`: build a FileSet from ordered roots, analyze
//! with the CK3 schema, dump counts / duplicates / unresolved diagnostics.

use std::process::ExitCode;

use pdxl_fileset::{FileKind, FileSet};
use pdxl_parity::dump_project;

fn parse_kind(s: &str) -> Option<FileKind> {
    match s {
        "vanilla" => Some(FileKind::Vanilla),
        "dlc" => Some(FileKind::Dlc),
        "dependency" => Some(FileKind::Dependency),
        "mod" => Some(FileKind::Mod),
        _ => None,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut roots: Vec<(String, FileKind)> = Vec::new();
    let mut ignore_dirs: Vec<String> = Vec::new();
    let mut ignore_files: Vec<String> = Vec::new();
    let mut replace: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let flag = &args[i];
        let Some(value) = args.get(i + 1) else {
            eprintln!("missing value for {flag}");
            return ExitCode::from(2);
        };
        match flag.as_str() {
            "--root" => {
                let Some((path, kind_str)) = value.rsplit_once(':') else {
                    eprintln!("--root expects <path>:<kind>");
                    return ExitCode::from(2);
                };
                let Some(kind) = parse_kind(kind_str) else {
                    eprintln!("unknown kind: {kind_str}");
                    return ExitCode::from(2);
                };
                roots.push((path.to_string(), kind));
            }
            "--ignore-dir" => ignore_dirs.push(value.clone()),
            "--ignore-file" => ignore_files.push(value.clone()),
            "--replace" => replace.push(value.clone()),
            other => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
        }
        i += 2;
    }
    if roots.is_empty() {
        eprintln!("usage: projectdump --root <path>:<kind> ...");
        return ExitCode::from(2);
    }

    let mut fs = FileSet::new();
    fs.set_ignore(&ignore_dirs, &ignore_files);
    fs.set_replace_paths(&replace);
    for (root, kind) in &roots {
        if let Err(e) = fs.add(root, *kind) {
            eprintln!("scanning {root}: {e}");
            return ExitCode::from(1);
        }
    }

    let schema = pdxl_ck3::schema();
    match pdxl_project::analyze(&fs, &schema) {
        Ok((table, diags)) => {
            print!("{}", dump_project(&table, &diags, schema.kinds()));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("analyze: {e}");
            ExitCode::from(1)
        }
    }
}
