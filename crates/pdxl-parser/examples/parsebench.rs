//! Minimal parser throughput benchmark, mirroring Go's `BenchmarkParseLarge`.
//!
//! Parses each `testdata/*.txt` fixture (and the large one repeatedly) and
//! reports ns/op, MB/s, and the produced node/child/diagnostic counts. This is a
//! parity-era sanity check, not a tuned benchmark.
//!
//! Run with: `cargo run --release -p pdxl-syntax --example parsebench`

use std::path::PathBuf;
use std::time::Instant;

use pdxl_parser::parse;

fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("crates").is_dir() {
            return dir;
        }
        assert!(dir.pop(), "could not locate repo root");
    }
}

fn bench(name: &str, src: &[u8]) {
    // Probe once for the structural counts (identical every iteration).
    let probe = parse("bench", src.to_vec());
    let nodes = probe.tree().len();
    let children = probe.tree().child_index().len();
    let diags = probe.diagnostics().len();
    drop(probe);

    let target = std::time::Duration::from_millis(400);
    let mut iters: u64 = 0;
    let start = Instant::now();
    loop {
        let parsed = parse("bench", src.to_vec());
        std::hint::black_box(&parsed);
        iters += 1;
        if iters.is_multiple_of(32) && start.elapsed() >= target {
            break;
        }
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;
    let mb_per_s = (src.len() as f64) / (ns_per_op / 1e9) / (1024.0 * 1024.0);
    println!(
        "{name:<40} {ns_per_op:>12.0} ns/op {mb_per_s:>8.2} MB/s  nodes={nodes} children={children} diags={diags}"
    );
}

fn main() {
    let testdata = repo_root().join("testdata");
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(&testdata)
        .expect("read testdata")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("txt"))
        .collect();
    fixtures.sort();

    println!("pdxl-syntax parser throughput (release):\n");
    for f in &fixtures {
        let src = std::fs::read(f).expect("read fixture");
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        bench(&name, &src);
    }
    let large = std::fs::read(testdata.join("international_organization.txt")).expect("read large");
    bench("ParseLarge", &large);
}
