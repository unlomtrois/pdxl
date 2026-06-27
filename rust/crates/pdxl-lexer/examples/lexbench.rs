//! Minimal lexer throughput benchmark, mirroring Go's `BenchmarkLexLarge`.
//!
//! Lexes each `testdata/*.txt` fixture (and the large one repeatedly) and reports
//! ns/op and MB/s. This is a parity-era sanity check, not a tuned benchmark; a
//! Criterion suite can replace it if optimization work begins.
//!
//! Run with: `cargo run --release -p pdxl-lexer --example lexbench`

use std::path::PathBuf;
use std::time::Instant;

use pdxl_lexer::Lexer;

fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("go.mod").is_file() {
            return dir;
        }
        assert!(dir.pop(), "could not locate repo root");
    }
}

fn bench(name: &str, src: &[u8]) {
    // Warm up, then time enough iterations to get a stable per-op figure.
    let target = std::time::Duration::from_millis(400);
    let mut iters: u64 = 0;
    let start = Instant::now();
    loop {
        let mut lexer = Lexer::init(src);
        while lexer.next_token().is_some() {}
        iters += 1;
        if iters.is_multiple_of(64) && start.elapsed() >= target {
            break;
        }
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;
    let mb_per_s = (src.len() as f64) / (ns_per_op / 1e9) / (1024.0 * 1024.0);
    println!("{name:<40} {ns_per_op:>12.0} ns/op {mb_per_s:>10.2} MB/s  ({iters} iters)");
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

    println!("pdxl-lexer throughput (release):\n");
    for f in &fixtures {
        let src = std::fs::read(f).expect("read fixture");
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        bench(&name, &src);
    }

    // Stable single-file baseline on the largest fixture.
    let large = std::fs::read(testdata.join("international_organization.txt")).expect("read large");
    bench("LexLarge", &large);
}
