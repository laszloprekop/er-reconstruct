//! Native conformance-corpus harness (ADR-0010).
//!
//! Loads real saves and asserts `reconstruct(bytes, slot)` reproduces the KNOWN
//! TRUTH recorded in `tests/corpus/expected.json` — facts cross-checked against
//! the reader's independent export, not against reconstruct's own output. This is
//! the oracle during the strangler migration and the permanent regression guard;
//! a later slice adds the `native == WASM` parity leg over the same corpus.
//!
//! Both the manifest (personal save data — character names/levels) and the save
//! bytes are kept out of this public repo. The manifest `tests/corpus/expected.json`
//! is gitignored (copy `expected.example.json` to create it); the saves are located
//! via `ER_RECONSTRUCT_CORPUS_DIR`. When either is absent the harness **skips with a
//! notice** rather than failing, so a fresh clone still goes green.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

/// Where the real saves live — from the environment only; nothing personal is
/// baked in. `None` means the harness skips.
fn corpus_dir() -> Option<PathBuf> {
    std::env::var("ER_RECONSTRUCT_CORPUS_DIR")
        .ok()
        .map(PathBuf::from)
}

#[test]
fn corpus_saves_reconstruct_to_known_truth() {
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/expected.json");
    if !manifest_path.exists() {
        eprintln!(
            "SKIP corpus harness: no {manifest_path:?} (personal save data, gitignored). \
             Copy tests/corpus/expected.example.json to create it."
        );
        return;
    }
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("read corpus manifest"),
    )
    .expect("corpus manifest is valid JSON");

    let cases = manifest["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty(), "corpus must carry at least one case");

    let dir = match corpus_dir() {
        Some(dir) if dir.is_dir() => dir,
        _ => {
            eprintln!(
                "SKIP corpus harness: set ER_RECONSTRUCT_CORPUS_DIR to the directory \
                 holding the corpus saves to run against them."
            );
            return;
        }
    };

    let mut checked = 0usize;
    for case in cases {
        let save_name = case["save"].as_str().expect("save name");
        let slot = case["slot"].as_u64().expect("slot") as usize;
        let expected = &case["expected"];

        let save_path = dir.join(save_name);
        if !save_path.is_file() {
            eprintln!("SKIP case {save_name}#{slot}: file {save_path:?} not present");
            continue;
        }

        let bytes = fs::read(&save_path).expect("read save bytes");
        let got = er_reconstruct::reconstruct(&bytes, slot)
            .unwrap_or_else(|e| panic!("reconstruct {save_name}#{slot} failed: {e}"));

        assert_eq!(
            got.name,
            expected["name"].as_str().unwrap(),
            "name mismatch for {save_name}#{slot}"
        );
        assert_eq!(
            u64::from(got.level),
            expected["level"].as_u64().unwrap(),
            "level mismatch for {save_name}#{slot}"
        );
        assert_eq!(
            u64::from(got.class_id),
            expected["class_id"].as_u64().unwrap(),
            "class_id mismatch for {save_name}#{slot}"
        );
        checked += 1;
    }

    assert!(
        checked > 0,
        "corpus directory {dir:?} present but no case save files found — \
         the harness must exercise at least one real save"
    );
    eprintln!("corpus harness checked {checked} real-save case(s)");
}
