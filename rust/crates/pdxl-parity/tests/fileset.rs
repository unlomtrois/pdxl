//! Differential parity test: the Rust FileSet/descriptor vs the Go oracle.
//!
//! Each scenario builds a temp tree, then compares the Rust crate's canonical
//! dump (`dump_scan` / `dump_descriptor`, built in-process) against the Go tool
//! (`go run ./tools/filesetdump ...`) operating on the SAME tree. Entry order,
//! stats, resolutions, and descriptor facts must match byte-for-byte.
//!
//! Self-skips with a warning if `go` is unavailable.

use std::path::{Path, PathBuf};
use std::process::Command;

use pdxl_fileset::{FileKind, FileSet, validate_fileset};
use pdxl_moddesc::parse_mod;
use pdxl_parity::{dump_descriptor, dump_scan};
use pdxl_testutil::{TempTree, go_available};

fn repo_root() -> PathBuf {
    pdxl_testutil::repo_root(env!("CARGO_MANIFEST_DIR"))
}

fn kind_str(k: FileKind) -> &'static str {
    k.as_str()
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

    /// Builds the Rust dump in-process.
    fn rust_dump(&self) -> String {
        let mut fs = FileSet::new();
        fs.set_ignore(&self.ignore_dirs, &self.ignore_files);
        fs.set_replace_paths(&self.replace);
        for (root, kind) in &self.roots {
            fs.add(root, *kind).expect("add root");
        }
        validate_fileset(&fs).expect("fileset invariants");
        dump_scan(&fs, &self.queries)
    }

    /// Builds CLI args for the Go tool.
    fn go_args(&self) -> Vec<String> {
        let mut a = vec!["run".into(), "./tools/filesetdump".into(), "scan".into()];
        for (root, kind) in &self.roots {
            a.push("--root".into());
            a.push(format!("{}:{}", root.display(), kind_str(*kind)));
        }
        for d in &self.ignore_dirs {
            a.push("--ignore-dir".into());
            a.push(d.clone());
        }
        for f in &self.ignore_files {
            a.push("--ignore-file".into());
            a.push(f.clone());
        }
        for p in &self.replace {
            a.push("--replace".into());
            a.push(p.clone());
        }
        for q in &self.queries {
            a.push("--query".into());
            a.push(q.clone());
        }
        a
    }
}

fn run_go(root: &Path, args: &[String]) -> String {
    let out = Command::new("go")
        .current_dir(root)
        .args(args)
        .output()
        .expect("spawn go");
    assert!(
        out.status.success(),
        "go tool failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8")
}

fn assert_scan_parity(name: &str, repo: &Path, scan: &Scan) {
    let rust = scan.rust_dump();
    let go = run_go(repo, &scan.go_args());
    assert_eq!(rust, go, "scan dump mismatch in scenario '{name}'");
}

#[test]
fn fileset_differential() {
    let repo = repo_root();
    if !go_available() {
        eprintln!("warning: `go` not found — skipping FileSet differential parity test");
        return;
    }

    // --- basic scan: nested, uppercase .TXT, non-script, empty dir ---
    let basic = TempTree::new();
    basic.write("common/traits/a.txt", "");
    basic.write("common/EVENTS.TXT", "");
    basic.write("readme.md", "");
    basic.write("notes.log", "");
    std::fs::create_dir_all(basic.path.join("empty/dir")).unwrap();
    assert_scan_parity(
        "basic",
        &repo,
        &Scan::new()
            .root(&basic.path, FileKind::Mod)
            .query("COMMON/EVENTS.TXT")
            .query("missing.txt"),
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
    assert_scan_parity(
        "overlay",
        &repo,
        &Scan::new()
            .root(&van.path, FileKind::Vanilla)
            .root(&dlc.path, FileKind::Dlc)
            .root(&dep.path, FileKind::Dependency)
            .root(&md.path, FileKind::Mod)
            .query("b.txt")
            .query("shared.txt"),
    );

    // --- replacement: exact, descendant, similar non-match, kinds, count ---
    let rvan = TempTree::new();
    let rmod = TempTree::new();
    rvan.write("common/landed_titles/base.txt", "");
    rvan.write("common/landed_titles_extra/x.txt", "");
    rvan.write("common/traits/n.txt", "");
    rmod.write("common/landed_titles/custom.txt", "");
    assert_scan_parity(
        "replacement",
        &repo,
        &Scan::new()
            .replace("common/landed_titles")
            .root(&rvan.path, FileKind::Vanilla)
            .root(&rmod.path, FileKind::Mod)
            .query("common/landed_titles/base.txt")
            .query("common/landed_titles/custom.txt")
            .query("common/landed_titles_extra/x.txt"),
    );

    // --- ignore: nested dirs/files, case-insensitive, dot dirs ---
    let ig = TempTree::new();
    ig.write("keep.txt", "");
    ig.write("licenses/a.txt", "");
    ig.write("deep/licenses/b.txt", "");
    ig.write("deep/Open_Sans/LICENSE.txt", "");
    ig.write(".git/c.txt", "");
    ig.write("sub/.hidden/d.txt", "");
    assert_scan_parity(
        "ignore",
        &repo,
        &Scan::new()
            .ignore_dir("licenses")
            .ignore_file("license.txt")
            .root(&ig.path, FileKind::Mod),
    );

    // --- normalization: mixed case, nested, non-ASCII, case collision ---
    let nm = TempTree::new();
    nm.write("Common/Traits/Noble.txt", "");
    nm.write("café/Ω.txt", ""); // accented Latin + Greek (simple==full lower)
    nm.write("A.txt", "first");
    nm.write("a.txt", "second"); // collides with A.txt after lowercase
    assert_scan_parity(
        "normalization",
        &repo,
        &Scan::new()
            .root(&nm.path, FileKind::Mod)
            .query("COMMON/TRAITS/NOBLE.TXT")
            .query("CAFÉ/Ω.TXT"),
    );

    eprintln!("FileSet differential: 5 scenarios byte-identical to Go oracle");
}

#[test]
fn descriptor_differential() {
    let repo = repo_root();
    if !go_available() {
        eprintln!("warning: `go` not found — skipping descriptor differential parity test");
        return;
    }

    let mut mod_files: Vec<PathBuf> = vec![repo.join("testdata/T4N.mod")];

    // A relative-path descriptor.
    let rel = TempTree::new();
    let rel_file = rel.child("mymod.mod");
    std::fs::write(&rel_file, "name=\"My Mod\"\npath=\"mods/mymod\"\nreplace_path=\"common/x\"\nreplace_path=\"common/x\"\n").unwrap();
    mod_files.push(rel_file);

    // Windows forward-slash and backslash descriptors.
    let win = TempTree::new();
    let winf = win.child("win.mod");
    std::fs::write(&winf, "name=\"Win\"\npath=\"C:/users/steamuser/mod/X\"\n").unwrap();
    mod_files.push(winf);
    let bs = win.child("bs.mod");
    std::fs::write(&bs, "path=\"C:\\users\\steamuser\\mod\\Y\"\n").unwrap();
    mod_files.push(bs);

    // Repeated name/path, unknown + missing fields, malformed-but-readable.
    let misc = TempTree::new();
    let dupf = misc.child("dup.mod");
    std::fs::write(
        &dupf,
        "name=\"First\"\nname=\"Second\"\npath=\"a/one\"\npath=\"a/two\"\nbogus=1\n",
    )
    .unwrap();
    mod_files.push(dupf);
    let broken = misc.child("broken.mod");
    std::fs::write(
        &broken,
        "name=\"Broken\"\nreplace_path=\"common/y\"\ntags={ \"oops\"\n",
    )
    .unwrap();
    mod_files.push(broken);

    // Unix-absolute path: kept verbatim after the lockstep ParseMod fix (M7) —
    // the Linux launcher writes absolute Unix paths into real descriptors.
    let unix_abs = misc.child("abs.mod");
    std::fs::write(&unix_abs, "name=\"Abs\"\npath=\"/opt/mods/AbsMod\"\n").unwrap();
    mod_files.push(unix_abs);

    for mf in &mod_files {
        let m = parse_mod(mf).expect("parse_mod");
        let rust = dump_descriptor(&m);
        let go = run_go(
            &repo,
            &[
                "run".into(),
                "./tools/filesetdump".into(),
                "descriptor".into(),
                mf.to_string_lossy().into_owned(),
            ],
        );
        assert_eq!(rust, go, "descriptor dump mismatch for {}", mf.display());
    }

    eprintln!(
        "descriptor differential: {} descriptors byte-identical to Go oracle",
        mod_files.len()
    );
}
