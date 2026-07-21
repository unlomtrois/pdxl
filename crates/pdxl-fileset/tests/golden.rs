//! FileSet / descriptor regression tests — golden snapshots.
//!
//! Historical note: these were byte-differential tests against the Go oracle
//! (`go run ./tools/filesetdump`), verified byte-identical before the Go
//! implementation was removed. Each scenario builds a temp tree, dumps the
//! scan/descriptor in-process, normalizes the temp roots to `<rootN>`, and
//! compares against a golden. To accept an intentional change, regenerate
//! with `UPDATE_GOLDENS=1 cargo test -p pdxl-fileset --test golden`
//! and review the diff like any other code change.

use std::path::{Path, PathBuf};

use pdxl_fileset::{FileKind, FileSet, validate_fileset};
use pdxl_moddesc::{ModDescriptor, parse_mod};
use pdxl_path::is_windows_absolute;
use pdxl_testutil::TempTree;

/// Dump schema version. Bump on any format change.
const FILESET_DUMP_VERSION: u32 = 1;

/// Canonical scan dump: entries (winner order), stats, resolutions. JSON with
/// one entry / resolution per line; entries stay in exact `iter` (winner) order.
fn dump_scan(fs: &FileSet, queries: &[String]) -> String {
    let mut out = String::new();
    out.push_str("{\n\"version\":");
    out.push_str(&FILESET_DUMP_VERSION.to_string());
    out.push_str(",\n\"entries\":[");

    let entries: Vec<_> = fs.iter().collect();
    if !entries.is_empty() {
        out.push('\n');
        for (i, e) in entries.iter().enumerate() {
            out.push_str("{\"rel_path\":\"");
            push_escaped(&mut out, &e.rel_path);
            out.push_str("\",\"full_path\":\"");
            push_escaped(&mut out, &e.full_path.to_string_lossy());
            out.push_str("\",\"kind\":\"");
            out.push_str(e.kind.as_str());
            out.push_str("\"}");
            if i + 1 < entries.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("],\n");

    let st = fs.stats();
    out.push_str("\"stats\":{\"vanilla\":");
    out.push_str(&st.vanilla.to_string());
    out.push_str(",\"mod\":");
    out.push_str(&st.mod_files.to_string());
    out.push_str(",\"total\":");
    out.push_str(&st.total.to_string());
    out.push_str(",\"shadowed\":");
    out.push_str(&st.shadowed.to_string());
    out.push_str(",\"replaced\":");
    out.push_str(&st.replaced.to_string());
    out.push_str("},\n");

    out.push_str("\"resolutions\":[");
    if !queries.is_empty() {
        out.push('\n');
        for (i, q) in queries.iter().enumerate() {
            out.push_str("{\"query\":\"");
            push_escaped(&mut out, q);
            match fs.resolve(q) {
                Some(e) => {
                    out.push_str("\",\"found\":true,\"rel_path\":\"");
                    push_escaped(&mut out, &e.rel_path);
                    out.push_str("\",\"kind\":\"");
                    out.push_str(e.kind.as_str());
                    out.push_str("\"}");
                }
                None => {
                    out.push_str("\",\"found\":false,\"rel_path\":null,\"kind\":null}");
                }
            }
            if i + 1 < queries.len() {
                out.push(',');
            }
            out.push('\n');
        }
    }
    out.push_str("]\n}\n");
    out
}

/// Canonical descriptor dump.
fn dump_descriptor(m: &ModDescriptor) -> String {
    let path_str = m.path.to_string_lossy();
    let mut out = String::new();
    out.push_str("{\n\"version\":");
    out.push_str(&FILESET_DUMP_VERSION.to_string());
    out.push_str(",\n\"name\":\"");
    push_escaped(&mut out, &m.name);
    out.push_str("\",\n\"path\":\"");
    push_escaped(&mut out, &path_str);
    out.push_str("\",\n\"replace_paths\":[");
    for (i, rp) in m.replace_paths.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        push_escaped(&mut out, rp);
        out.push('"');
    }
    out.push_str("],\n\"is_windows_absolute\":");
    out.push_str(if is_windows_absolute(&path_str) {
        "true"
    } else {
        "false"
    });
    out.push_str("\n}\n");
    out
}

/// Appends `s` with minimal JSON string escaping.
fn push_escaped(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
}

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

/// A scan scenario: ordered roots, ignore/replace config, resolve queries.
struct Scan {
    roots: Vec<(PathBuf, FileKind)>,
    ignore_dirs: Vec<String>,
    ignore_files: Vec<String>,
    replace: Vec<String>,
    queries: Vec<String>,
}

impl Scan {
    fn new() -> Self {
        Scan {
            roots: Vec::new(),
            ignore_dirs: Vec::new(),
            ignore_files: Vec::new(),
            replace: Vec::new(),
            queries: Vec::new(),
        }
    }
    fn root(mut self, p: &Path, k: FileKind) -> Self {
        self.roots.push((p.to_path_buf(), k));
        self
    }
    fn ignore_dir(mut self, d: &str) -> Self {
        self.ignore_dirs.push(d.into());
        self
    }
    fn ignore_file(mut self, f: &str) -> Self {
        self.ignore_files.push(f.into());
        self
    }
    fn replace(mut self, p: &str) -> Self {
        self.replace.push(p.into());
        self
    }
    fn query(mut self, q: &str) -> Self {
        self.queries.push(q.into());
        self
    }

    /// Builds the dump in-process, with temp roots normalized to `<rootN>`.
    fn dump(&self) -> String {
        let mut fs = FileSet::new();
        fs.set_ignore(&self.ignore_dirs, &self.ignore_files);
        fs.set_replace_paths(&self.replace);
        for (root, kind) in &self.roots {
            fs.add(root, *kind).expect("add root");
        }
        validate_fileset(&fs).expect("fileset invariants");
        let mut dump = dump_scan(&fs, &self.queries);
        for (i, (root, _)) in self.roots.iter().enumerate() {
            dump = dump.replace(&root.to_string_lossy().into_owned(), &format!("<root{i}>"));
        }
        dump
    }
}

fn check_golden(name: &str, dump: &str) {
    let goldens_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/goldens/fileset");
    let golden_path = goldens_dir.join(format!("{name}.golden"));
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(&goldens_dir).unwrap();
        std::fs::write(&golden_path, dump).unwrap();
        return;
    }
    let golden = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|_| panic!("missing golden {golden_path:?} — run with UPDATE_GOLDENS=1"));
    assert_eq!(
        dump, golden,
        "fileset dump changed for '{name}'; if intentional, regenerate with \
         UPDATE_GOLDENS=1 cargo test -p pdxl-fileset --test golden"
    );
}

#[test]
fn fileset_scan_scenarios() {
    // --- basic scan: nested, uppercase .TXT, non-script, empty dir ---
    let basic = TempTree::new();
    basic.write("common/traits/a.txt", "");
    basic.write("common/EVENTS.TXT", "");
    basic.write("readme.md", "");
    basic.write("notes.log", "");
    std::fs::create_dir_all(basic.path.join("empty/dir")).unwrap();
    check_golden(
        "basic",
        &Scan::new()
            .root(&basic.path, FileKind::Mod)
            .query("COMMON/EVENTS.TXT")
            .query("missing.txt")
            .dump(),
    );

    // --- overlay: vanilla/dlc/dependency/mod, stable winner slots ---
    let van = TempTree::new();
    let dlc = TempTree::new();
    let dep = TempTree::new();
    let md = TempTree::new();
    van.write("a.txt", "");
    van.write("b.txt", "");
    van.write("shared.txt", "");
    dlc.write("d.txt", "");
    dlc.write("shared.txt", "");
    dep.write("dep.txt", "");
    md.write("b.txt", ""); // shadows vanilla b in its slot
    md.write("c.txt", "");
    md.write("shared.txt", ""); // shadows in shared's slot
    check_golden(
        "overlay",
        &Scan::new()
            .root(&van.path, FileKind::Vanilla)
            .root(&dlc.path, FileKind::Dlc)
            .root(&dep.path, FileKind::Dependency)
            .root(&md.path, FileKind::Mod)
            .query("b.txt")
            .query("shared.txt")
            .dump(),
    );

    // --- replacement: exact, descendant, similar non-match, kinds, count ---
    let rvan = TempTree::new();
    let rmod = TempTree::new();
    rvan.write("common/landed_titles/base.txt", "");
    rvan.write("common/landed_titles_extra/x.txt", "");
    rvan.write("common/traits/n.txt", "");
    rmod.write("common/landed_titles/custom.txt", "");
    check_golden(
        "replacement",
        &Scan::new()
            .replace("common/landed_titles")
            .root(&rvan.path, FileKind::Vanilla)
            .root(&rmod.path, FileKind::Mod)
            .query("common/landed_titles/base.txt")
            .query("common/landed_titles/custom.txt")
            .query("common/landed_titles_extra/x.txt")
            .dump(),
    );

    // --- ignore: nested dirs/files, case-insensitive, dot dirs ---
    let ig = TempTree::new();
    ig.write("keep.txt", "");
    ig.write("licenses/a.txt", "");
    ig.write("deep/licenses/b.txt", "");
    ig.write("deep/Open_Sans/LICENSE.txt", "");
    ig.write(".git/c.txt", "");
    ig.write("sub/.hidden/d.txt", "");
    check_golden(
        "ignore",
        &Scan::new()
            .ignore_dir("licenses")
            .ignore_file("license.txt")
            .root(&ig.path, FileKind::Mod)
            .dump(),
    );

    // --- normalization: mixed case, nested, non-ASCII, case collision ---
    let nm = TempTree::new();
    nm.write("Common/Traits/Noble.txt", "");
    nm.write("café/Ω.txt", ""); // accented Latin + Greek (simple==full lower)
    nm.write("A.txt", "first");
    nm.write("a.txt", "second"); // collides with A.txt after lowercase
    check_golden(
        "normalization",
        &Scan::new()
            .root(&nm.path, FileKind::Mod)
            .query("COMMON/TRAITS/NOBLE.TXT")
            .query("CAFÉ/Ω.TXT")
            .dump(),
    );
}

#[test]
fn descriptor_scenarios() {
    let repo = repo_root();
    // (name, .mod path, temp roots to normalize out of the dump)
    let mut cases: Vec<(String, PathBuf, Vec<PathBuf>)> = vec![(
        "t4n".into(),
        repo.join("testdata/T4N.mod"),
        vec![repo.clone()],
    )];

    // A relative-path descriptor.
    let rel = TempTree::new();
    let rel_file = rel.child("mymod.mod");
    std::fs::write(
        &rel_file,
        "name=\"My Mod\"\npath=\"mods/mymod\"\nreplace_path=\"common/x\"\nreplace_path=\"common/x\"\n",
    )
    .unwrap();
    cases.push(("relative".into(), rel_file, vec![rel.path.clone()]));

    // Windows forward-slash and backslash descriptors.
    let win = TempTree::new();
    let winf = win.child("win.mod");
    std::fs::write(&winf, "name=\"Win\"\npath=\"C:/users/steamuser/mod/X\"\n").unwrap();
    cases.push(("win_fwd".into(), winf, vec![win.path.clone()]));
    let bs = win.child("bs.mod");
    std::fs::write(&bs, "path=\"C:\\users\\steamuser\\mod\\Y\"\n").unwrap();
    cases.push(("win_back".into(), bs, vec![win.path.clone()]));

    // Repeated name/path, unknown + missing fields, malformed-but-readable.
    let misc = TempTree::new();
    let dupf = misc.child("dup.mod");
    std::fs::write(
        &dupf,
        "name=\"First\"\nname=\"Second\"\npath=\"a/one\"\npath=\"a/two\"\nbogus=1\n",
    )
    .unwrap();
    cases.push(("dup_fields".into(), dupf, vec![misc.path.clone()]));
    let broken = misc.child("broken.mod");
    std::fs::write(
        &broken,
        "name=\"Broken\"\nreplace_path=\"common/y\"\ntags={ \"oops\"\n",
    )
    .unwrap();
    cases.push(("broken".into(), broken, vec![misc.path.clone()]));

    // Unix-absolute path: kept verbatim after the lockstep ParseMod fix (M7) —
    // the Linux launcher writes absolute Unix paths into real descriptors.
    let unix_abs = misc.child("abs.mod");
    std::fs::write(&unix_abs, "name=\"Abs\"\npath=\"/opt/mods/AbsMod\"\n").unwrap();
    cases.push(("unix_abs".into(), unix_abs, vec![misc.path.clone()]));

    for (name, mf, roots) in &cases {
        let m = parse_mod(mf).expect("parse_mod");
        let mut dump = dump_descriptor(&m);
        for (i, root) in roots.iter().enumerate() {
            dump = dump.replace(&root.to_string_lossy().into_owned(), &format!("<root{i}>"));
        }
        check_golden(&format!("descriptor_{name}"), &dump);
    }
}
