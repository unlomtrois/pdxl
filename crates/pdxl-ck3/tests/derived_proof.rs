//! Manual measurement for the table-driven reference derivation (see
//! `src/derived.rs`): compares the hand-written schema against hand + derived
//! scope-link rules over the real corpus, per kind. Ignored by default.
//!
//! Unlike the EU5 harness this reports every kind the map touches rather than
//! one, because CK3's map spans twenty scope types — the question is not "does
//! it work" but "which entries earn their noise".
//!
//! ```sh
//! PDXL_CK3_GAME="…/Crusader Kings III/game" \
//! PDXL_CK3_MOD="…/mod/T4N-CK3/T4N" \
//!   cargo test --release -p pdxl-ck3 --features ck3 --test derived_proof -- --ignored --nocapture
//! ```

use std::collections::HashMap;

use pdxl_analysis::Schema;
use pdxl_fileset::{FileKind, FileSet};

fn fileset() -> FileSet {
    let game = std::env::var("PDXL_CK3_GAME").expect("set PDXL_CK3_GAME");
    let mut fs = FileSet::new();
    fs.add(&game, FileKind::Vanilla).expect("scan game");
    if let Ok(mod_dir) = std::env::var("PDXL_CK3_MOD") {
        fs.add(&mod_dir, FileKind::Mod).expect("scan mod");
    }
    fs
}

/// Per-kind (refs, unresolved) for one schema over the corpus.
fn measure(fs: &FileSet, schema: &Schema) -> (HashMap<String, usize>, HashMap<String, usize>) {
    let mut order: Vec<String> = Vec::new();
    let mut facts = HashMap::new();
    let mut refs: HashMap<String, usize> = HashMap::new();
    for entry in fs.iter() {
        if !entry.rel_path.ends_with(".txt") {
            continue;
        }
        let Ok(src) = std::fs::read(&entry.full_path) else {
            continue;
        };
        let (tree, _) = pdxl_parser::parse(entry.rel_path.clone(), src).into_parts();
        let f = pdxl_analysis::extract_facts(&tree, &entry.rel_path, &entry.rel_path, schema, None);
        for r in &f.refs {
            *refs.entry(r.kind.name().to_string()).or_default() += 1;
        }
        order.push(entry.rel_path.clone());
        facts.insert(entry.rel_path.clone(), f);
    }
    let rels: Vec<&str> = order.iter().map(String::as_str).collect();
    let (_, diags) = pdxl_analysis::merge_and_resolve(&rels, &facts);
    let mut unres: HashMap<String, usize> = HashMap::new();
    for d in &diags {
        if let Some(kind) = d
            .msg
            .strip_prefix("unknown ")
            .and_then(|m| m.split(' ').next())
        {
            *unres.entry(kind.to_string()).or_default() += 1;
        }
    }
    (refs, unres)
}

#[test]
#[ignore = "needs the CK3 corpus; run with PDXL_CK3_GAME set"]
fn derived_link_rules_proof() {
    let fs = fileset();
    let (hand_refs, hand_unres) = measure(&fs, &pdxl_ck3::schema_hand_only());
    let (der_refs, der_unres) = measure(&fs, &pdxl_ck3::schema());

    let mut kinds: Vec<&String> = der_refs.keys().collect();
    kinds.sort();
    eprintln!(
        "{:<28} {:>9} {:>9} {:>9}",
        "kind", "+refs", "+unres", "resolve%"
    );
    let (mut tot_gain, mut tot_noise) = (0i64, 0i64);
    for k in kinds {
        let gain = der_refs.get(k).copied().unwrap_or(0) as i64
            - hand_refs.get(k).copied().unwrap_or(0) as i64;
        let noise = der_unres.get(k).copied().unwrap_or(0) as i64
            - hand_unres.get(k).copied().unwrap_or(0) as i64;
        if gain == 0 && noise == 0 {
            continue;
        }
        let pct = if gain > 0 {
            100.0 * (gain - noise) as f64 / gain as f64
        } else {
            0.0
        };
        eprintln!("{k:<28} {gain:>9} {noise:>9} {pct:>8.1}%");
        tot_gain += gain;
        tot_noise += noise;
    }
    eprintln!(
        "{:<28} {:>9} {:>9} {:>8.1}%   ({} derived rules)",
        "TOTAL",
        tot_gain,
        tot_noise,
        100.0 * (tot_gain - tot_noise) as f64 / tot_gain.max(1) as f64,
        pdxl_ck3::derived::derived_link_rules()
            .iter()
            .map(|k| k.refs.len())
            .sum::<usize>(),
    );
}
