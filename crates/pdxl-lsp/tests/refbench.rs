//! Manual benchmark for the CodeLens-resolve hot path: one `references()`
//! call per definition of a file (what the editor triggers when every lens in
//! the viewport resolves). Ignored by default — needs the real game corpus.
//!
//! ```sh
//! PDXL_BENCH_GAME="$HOME/.local/share/Steam/steamapps/common/Crusader Kings III/game" \
//!   cargo test --release -p pdxl-lsp --test refbench -- --ignored --nocapture
//! ```

use std::time::Instant;

use lsp_types::Url;
use pdxl_lsp::{ServerState, build_project, offset_to_position};

#[test]
#[ignore = "needs the game corpus; run with PDXL_BENCH_GAME set"]
fn bench_references_per_def() {
    let Ok(game) = std::env::var("PDXL_BENCH_GAME") else {
        panic!("set PDXL_BENCH_GAME to the game dir");
    };

    let t = Instant::now();
    let project = build_project(Some(&game), None).expect("build project");
    eprintln!("project build: {:?}", t.elapsed());

    let (tx, _rx) = crossbeam_channel::unbounded();
    let mut state = ServerState::new(None, tx);
    state.project_ready(Ok(Box::new(project)));

    let file = format!("{game}/common/named_colors/default_colors.txt");
    let uri = Url::from_file_path(&file).expect("uri");
    let src = std::fs::read(&file).expect("read");

    // The def offsets, straight from the project facts.
    let defs: Vec<(String, u32)> = {
        let facts = state
            .project()
            .expect("project")
            .facts_at(std::path::Path::new(&file))
            .expect("facts");
        facts
            .defs
            .iter()
            .map(|d| (d.name.clone(), d.end_offset.saturating_sub(1)))
            .collect()
    };
    eprintln!("defs in file: {}", defs.len());

    let t = Instant::now();
    let mut total_locs = 0usize;
    for (_, off) in &defs {
        let pos = offset_to_position(&src, *off);
        total_locs += state.references(&uri, pos, false).len();
    }
    eprintln!(
        "resolve all {} lenses: {:?} ({} locations)",
        defs.len(),
        t.elapsed(),
        total_locs
    );
}
