//! FileSet / descriptor differential dump tool (Rust side).
//!
//! Mirrors `tools/filesetdump/main.go`. Driven entirely by CLI args (no JSON
//! input) so neither implementation needs a JSON parser; the dump *output* is the
//! canonical JSON compared by the parity test.
//!
//! Scan mode:
//!   filesetdump scan \
//!     --root <path>:<kind> [--root ...] \
//!     [--ignore-dir <name>]... [--ignore-file <name>]... \
//!     [--replace <prefix>]... [--query <relpath>]...
//!   (roots are added in the given order; kind ∈ vanilla|dlc|dependency|mod)
//!
//! Descriptor mode:
//!   filesetdump descriptor <modfile>

use std::process::ExitCode;

use pdxl_fileset::{FileKind, FileSet};
use pdxl_moddesc::parse_mod;
use pdxl_parity::{dump_descriptor, dump_scan};

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
    match args.first().map(String::as_str) {
        Some("scan") => run_scan(&args[1..]),
        Some("descriptor") => run_descriptor(&args[1..]),
        _ => {
            eprintln!("usage: filesetdump <scan|descriptor> ...");
            ExitCode::from(2)
        }
    }
}

fn run_scan(args: &[String]) -> ExitCode {
    let mut roots: Vec<(String, FileKind)> = Vec::new();
    let mut ignore_dirs: Vec<String> = Vec::new();
    let mut ignore_files: Vec<String> = Vec::new();
    let mut replace: Vec<String> = Vec::new();
    let mut queries: Vec<String> = Vec::new();

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
            "--query" => queries.push(value.clone()),
            other => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
        }
        i += 2;
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

    print!("{}", dump_scan(&fs, &queries));
    ExitCode::SUCCESS
}

fn run_descriptor(args: &[String]) -> ExitCode {
    let Some(mod_file) = args.first() else {
        eprintln!("usage: filesetdump descriptor <modfile>");
        return ExitCode::from(2);
    };
    match parse_mod(mod_file) {
        Ok(m) => {
            print!("{}", dump_descriptor(&m));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("parsing {mod_file}: {e}");
            ExitCode::from(1)
        }
    }
}
