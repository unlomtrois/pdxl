//! Manual measurement for the table-driven-references proof (see
//! `src/derived.rs`): compares the hand schema against hand + derived
//! country rules over the real corpus. Ignored by default.
//!
//! ```sh
//! PDXL_EU5_GAME="…/Europa Universalis V/game" \
//! PDXL_EU5_MOD="…/mod/eu5-compagna-communis" \
//!   cargo test --release -p pdxl-eu5 --test derived_proof -- --ignored --nocapture
//! ```

use std::collections::HashMap;

use pdxl_analysis::Schema;
use pdxl_fileset::{FileKind, FileSet};

fn fileset() -> FileSet {
    let game = std::env::var("PDXL_EU5_GAME").expect("set PDXL_EU5_GAME");
    let mut fs = FileSet::new();
    fs.add(&game, FileKind::Vanilla).expect("scan game");
    if let Ok(mod_dir) = std::env::var("PDXL_EU5_MOD") {
        fs.add(&mod_dir, FileKind::Mod).expect("scan mod");
    }
    fs
}

/// Country-ref stats for one schema over the corpus.
fn measure(fs: &FileSet, schema: &Schema, label: &str) -> (usize, usize) {
    let mut order: Vec<String> = Vec::new();
    let mut facts = HashMap::new();
    let mut per_key: HashMap<String, usize> = HashMap::new();
    let mut country_refs = 0usize;
    for entry in fs.iter() {
        if !entry.rel_path.ends_with(".txt") {
            continue;
        }
        let Ok(src) = std::fs::read(&entry.full_path) else {
            continue;
        };
        let (tree, _) = pdxl_parser::parse(entry.rel_path.clone(), src.clone()).into_parts();
        let f = pdxl_analysis::extract_facts(&tree, &entry.rel_path, &entry.rel_path, schema, None);
        for r in &f.refs {
            if r.kind.name() == "country" {
                country_refs += 1;
                // Attribute to the key: find the line's key crudely via the
                // source before the ref (good enough for a distribution).
                let start = r.start as usize;
                let prefix = &src[..start.min(src.len())];
                let line_start = prefix
                    .iter()
                    .rposition(|&b| b == b'\n')
                    .map_or(0, |i| i + 1);
                let line = &src[line_start..start.min(src.len())];
                let key = line
                    .split(|&b| b == b'=')
                    .next()
                    .map(|k| String::from_utf8_lossy(k).trim().to_string())
                    .unwrap_or_default();
                *per_key.entry(key).or_default() += 1;
            }
        }
        order.push(entry.rel_path.clone());
        facts.insert(entry.rel_path.clone(), f);
    }
    let rels: Vec<&str> = order.iter().map(String::as_str).collect();
    let (_, diags) = pdxl_analysis::merge_and_resolve(&rels, &facts);
    let unresolved_country = diags
        .iter()
        .filter(|d| d.msg.contains("unknown country"))
        .count();
    eprintln!("== {label}: {country_refs} country refs, {unresolved_country} unresolved");
    let mut keys: Vec<_> = per_key.into_iter().collect();
    keys.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (k, n) in keys.iter().take(15) {
        eprintln!("   {n:6}  {k}");
    }
    // Unresolved sample for noise judgment.
    for d in diags
        .iter()
        .filter(|d| d.msg.contains("unknown country"))
        .take(12)
    {
        eprintln!("   ✗ {}:{} {}", d.file, d.start, d.msg);
    }
    (country_refs, unresolved_country)
}

#[test]
#[ignore = "needs the EU5 corpus; run with PDXL_EU5_GAME set"]
fn derived_link_rules_proof() {
    let fs = fileset();
    let (base_refs, base_unres) = measure(&fs, &pdxl_eu5::schema_hand_only(), "hand rules only");
    let (der_refs, der_unres) = measure(
        &fs,
        &pdxl_eu5::schema(),
        "production (hand + derived links)",
    );
    eprintln!(
        "== delta: +{} country refs, {:+} unresolved ({} derived link rules)",
        der_refs as i64 - base_refs as i64,
        der_unres as i64 - base_unres as i64,
        pdxl_eu5::derived::derived_link_rules()
            .iter()
            .map(|k| k.refs.len())
            .sum::<usize>(),
    );
}
