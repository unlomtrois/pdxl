//! `pdxl check` — index definitions and resolve references across game+mod,
//! mirroring Go's `cmd/pdxl/check.go`: per-kind counts (`%-18s %6d`),
//! duplicate list, unresolved list, exit 1 on unresolved. With a file
//! argument, reports only that file's unresolved references.
//!
//! Deviation from Go, measurement-backed (see `docs/BASELINE.md`): `check`
//! does **not** use the AST cache. At real CK3 scale the warm AST-cache run
//! is no faster than a cold parse (both must read + hash every file; decoding
//! 212 MB of tree entries costs as much as parsing) — Go's fast warm path was
//! its FactStore (tiny facts entries), which stays unported. `--no-cache` is
//! accepted for interface compatibility and has no effect. The `[scan]`
//! ignore defaults match Go's `config.Default()`.

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use pdxl_analysis::{KindId, RefDiag};
use pdxl_fileset::{FileKind, FileSet};

/// `file:line:col` for a diagnostic, derived on demand (the value pdxl used to
/// precompute and store on every reference). Sources are read once and cached;
/// there are few unresolved refs, so this stays cheap.
fn diag_loc(cache: &mut HashMap<String, Option<Vec<u8>>>, d: &RefDiag) -> String {
    let src = cache
        .entry(d.file.to_string())
        .or_insert_with(|| std::fs::read(d.file.as_ref()).ok());
    let (line, col) = src
        .as_deref()
        .map_or((0, 0), |s| pdxl_src::line_col(s, d.start));
    format!("{}:{}:{}", d.file, line, col)
}

/// Go `config.Default()` scan ignores.
const IGNORE_DIRS: &[&str] = &["licenses"];
const IGNORE_FILES: &[&str] = &[
    "credits.txt",
    "checksum_manifest.txt",
    "guids.txt",
    "license.txt",
    "ofl.txt",
];
pub fn run(
    file: Option<&str>,
    game: Option<&str>,
    mod_arg: Option<&str>,
    _no_cache: bool, // accepted for Go interface compatibility; no effect
) -> io::Result<ExitCode> {
    let fs = build_project_fileset(game, mod_arg)?;
    let schema = pdxl_ck3::schema();
    let (table, diags) = pdxl_project::analyze(&fs, &schema)?;

    // Interface scripts: datafunction chain typing against the DumpDataTypes
    // registry. Reported like unresolved references (few; genuine errors).
    let gui_errors = check_gui_datafns(&fs)?;

    match file {
        Some(target) => report_file(&fs, &diags, target),
        None => report_project(&table, &diags, &gui_errors, schema.kinds()),
    }
}

/// Validates every `.gui` file's datafunction chains, returning
/// `file:line:col: message` strings.
fn check_gui_datafns(fs: &FileSet) -> io::Result<Vec<String>> {
    let registry = pdxl_ck3::datafn_registry();
    let mut out = Vec::new();
    for entry in fs.iter() {
        if !entry.rel_path.ends_with(".gui") {
            continue;
        }
        let src = std::fs::read(&entry.full_path)?;
        let full = entry.full_path.to_string_lossy().into_owned();
        let (tree, _) = pdxl_gui::parse(full.clone(), src.clone()).into_parts();
        for err in pdxl_gui::datafn::validate_datafns(&tree, registry) {
            let (line, col) = pdxl_src::line_col(&src, err.start);
            out.push(format!("{full}:{line}:{col}: {}", err.msg));
        }
    }
    Ok(out)
}

/// Mirrors Go `buildProjectFileSet`: ignores from config defaults, replace
/// paths from the .mod descriptor, vanilla then mod in load order.
fn build_project_fileset(game: Option<&str>, mod_arg: Option<&str>) -> io::Result<FileSet> {
    if game.is_none() && mod_arg.is_none() {
        return Err(io::Error::other(
            "provide --game and/or --mod (or set game_path/mod_path in pdxl.toml)",
        ));
    }

    // Resolve mod: .mod file or plain directory.
    let (mod_dir, replace_paths) = match mod_arg {
        None => (None, Vec::new()),
        Some(arg) => {
            let meta = std::fs::metadata(arg)
                .map_err(|e| io::Error::new(e.kind(), format!("mod: {e}")))?;
            if meta.is_dir() || !arg.to_lowercase().ends_with(".mod") {
                (Some(arg.to_string()), Vec::new())
            } else {
                let m = pdxl_moddesc::parse_mod(arg)
                    .map_err(|e| io::Error::new(e.kind(), format!("parsing .mod file: {e}")))?;
                (Some(m.path.to_string_lossy().into_owned()), m.replace_paths)
            }
        }
    };

    let mut fs = FileSet::new();
    fs.set_ignore(IGNORE_DIRS, IGNORE_FILES);
    fs.set_localization_language(pdxl_project::DEFAULT_LOC_LANGUAGE);
    fs.set_include_gui(true);
    if !replace_paths.is_empty() {
        fs.set_replace_paths(&replace_paths);
    }
    if let Some(game_dir) = game {
        fs.add(game_dir, FileKind::Vanilla)
            .map_err(|e| io::Error::new(e.kind(), format!("scanning game dir: {e}")))?;
    }
    if let Some(dir) = &mod_dir {
        fs.add(dir, FileKind::Mod)
            .map_err(|e| io::Error::new(e.kind(), format!("scanning mod dir: {e}")))?;
    }
    Ok(fs)
}

/// Mirrors Go `reportProject`.
fn report_project(
    table: &pdxl_analysis::SymbolTable,
    diags: &[pdxl_analysis::RefDiag],
    gui_errors: &[String],
    kinds: &[KindId],
) -> io::Result<ExitCode> {
    let stdout = io::stdout();
    let mut w = io::BufWriter::new(stdout.lock());
    for kind in kinds {
        writeln!(w, "{:<18} {:>6}", kind.name(), table.count(*kind))?;
    }
    writeln!(w, "{:<18} {:>6}", "total", table.total())?;

    if !table.duplicates.is_empty() {
        writeln!(w, "\n{} duplicate definitions:", table.duplicates.len())?;
        for d in &table.duplicates {
            writeln!(
                w,
                "  {} {:?} redefined in {} (first in {})",
                d.kind.name(),
                d.name,
                d.file,
                d.first.file
            )?;
        }
    }

    if !gui_errors.is_empty() {
        writeln!(w, "\n{} gui datafunction errors:", gui_errors.len())?;
        for e in gui_errors {
            writeln!(w, "  {e}")?;
        }
    }

    if !diags.is_empty() {
        writeln!(w, "\n{} unresolved references:", diags.len())?;
        let mut cache = HashMap::new();
        for d in diags {
            writeln!(w, "  {}: {}", diag_loc(&mut cache, d), d.msg)?;
        }
        w.flush()?;
        return Ok(ExitCode::FAILURE);
    }
    w.flush()?;
    if gui_errors.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}

/// Mirrors Go `reportFile`: only `target`'s unresolved references.
fn report_file(
    fs: &FileSet,
    diags: &[pdxl_analysis::RefDiag],
    target: &str,
) -> io::Result<ExitCode> {
    let Some(full_path) = project_path_of(fs, target) else {
        return Err(io::Error::other(format!(
            "{target} is not part of the scanned game/mod project"
        )));
    };
    let mut cache = HashMap::new();
    let mut n = 0;
    for d in diags {
        if d.file.as_ref() == full_path {
            println!("{}: {}", diag_loc(&mut cache, d), d.msg);
            n += 1;
        }
    }
    Ok(if n > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Mirrors Go `projectPathOf`: match `target` to a FileSet entry by cleaned
/// absolute path, returning the FullPath used in diagnostics.
fn project_path_of(fs: &FileSet, target: &str) -> Option<String> {
    let abs = abs_clean(Path::new(target))?;
    let mut found = None;
    for e in fs.iter() {
        if abs_clean(&e.full_path).as_deref() == Some(abs.as_str()) {
            found = Some(e.full_path.to_string_lossy().into_owned());
        }
    }
    found
}

fn abs_clean(path: &Path) -> Option<String> {
    let abs = std::path::absolute(path).ok()?;
    Some(pdxl_path::clean(&abs.to_string_lossy()))
}
