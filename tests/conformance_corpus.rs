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

use er_reconstruct::{FlagFact, FlagStatus};
use serde_json::Value;

/// Count facts in the `Set` state — the reader's "discovered" tally. `Clear` and
/// `Unknown` are both not-discovered; keeping `Unknown` out of the count is the
/// whole point of the tri-state (a `Set` count that swallowed `Unknown` would be
/// the 0/110 bug in reverse).
fn count_set(facts: &[FlagFact]) -> u64 {
    facts.iter().filter(|f| f.state == FlagStatus::Set).count() as u64
}

/// Assert the `Set` count of `facts` against optional `<key>_atleast` / `<key>_atmost`
/// bounds in the manifest. Bounds (not an exact `==`) because pickups have no
/// full-enumeration oracle — the numbers are known-truth inequalities, not
/// reconstruct's own output blessed as truth.
fn check_bound(facts: &[FlagFact], expected: &Value, key: &str, save: &str, slot: usize) {
    let set = facts.iter().filter(|f| f.state == FlagStatus::Set).count() as u64;
    if let Some(lo) = expected[format!("{key}_atleast")].as_u64() {
        assert!(
            set >= lo,
            "{key} for {save}#{slot}: {set} Set is below known-truth floor {lo}"
        );
    }
    if let Some(hi) = expected[format!("{key}_atmost")].as_u64() {
        assert!(
            set <= hi,
            "{key} for {save}#{slot}: {set} Set is above known-truth ceiling {hi}"
        );
    }
}

/// The manifest spells expected flag states as `"Set"`/`"Clear"`/`"Unknown"` —
/// the same tokens `FlagStatus` serializes to.
fn status_name(s: FlagStatus) -> &'static str {
    match s {
        FlagStatus::Set => "Set",
        FlagStatus::Clear => "Clear",
        FlagStatus::Unknown => "Unknown",
    }
}

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

        // Flag facts (slice #4), all OPTIONAL so identity-only cases still pass.
        // Counts cross-check against the reader's independent "discovered" tally;
        // targeted `flags` pin individual known-truth ids (e.g. the slot-0 gotcha:
        // Godrick + Margit defeated). A count is the number of `Set` facts —
        // `Unknown` and `Clear` are deliberately NOT counted as discovered.
        if let Some(want) = expected["graces_set"].as_u64() {
            let got_set = count_set(&got.graces);
            assert_eq!(
                got_set, want,
                "graces_set mismatch for {save_name}#{slot} (reconstruct {got_set} vs oracle {want})"
            );
        }
        if let Some(want) = expected["bosses_set"].as_u64() {
            let got_set = count_set(&got.bosses);
            assert_eq!(
                got_set, want,
                "bosses_set mismatch for {save_name}#{slot} (reconstruct {got_set} vs oracle {want})"
            );
        }

        // Pickup facts (slice #5). No full-enumeration export exists to give an
        // exact collected count, so the corpus guards them three independent ways:
        //  - totals: the reader's MACHINE-CHECKED table sizes (loaded fully?);
        //  - bounds: known-truth inequalities that encode the multi-slot
        //    differential (a near-endgame slot collects many; a level-9 slot few) —
        //    they catch the real regressions (all-Unknown, all-Clear, all-Set,
        //    wrong family) without trusting reconstruct's exact number;
        //  - targeted `flags`: specific ids known collected from an earlier export
        //    (pickup collection is monotonic, so collected-then stays Set now).
        if let Some(want) = expected["world_pickups_total"].as_u64() {
            assert_eq!(
                got.world_pickups.len() as u64, want,
                "world_pickups_total mismatch for {save_name}#{slot}"
            );
        }
        if let Some(want) = expected["dungeon_pickups_total"].as_u64() {
            assert_eq!(
                got.dungeon_pickups.len() as u64, want,
                "dungeon_pickups_total mismatch for {save_name}#{slot}"
            );
        }
        check_bound(&got.world_pickups, expected, "world_pickups_set", save_name, slot);
        check_bound(&got.dungeon_pickups, expected, "dungeon_pickups_set", save_name, slot);

        // `flags` pins per-id known-truth across ALL fact families (grace, boss,
        // world/dungeon pickup) — the id identifies which.
        if let Some(flags) = expected["flags"].as_array() {
            for want in flags {
                let id = want["id"].as_u64().expect("flag id") as u32;
                let want_state = want["state"].as_str().expect("flag state string");
                let fact = got
                    .graces
                    .iter()
                    .chain(got.bosses.iter())
                    .chain(got.world_pickups.iter())
                    .chain(got.dungeon_pickups.iter())
                    .find(|f| f.id == id)
                    .unwrap_or_else(|| {
                        panic!("flag id {id} is in no fact family for {save_name}#{slot}")
                    });
                assert_eq!(
                    status_name(fact.state),
                    want_state,
                    "flag {id} state mismatch for {save_name}#{slot}"
                );
            }
        }
        checked += 1;
    }

    assert!(
        checked > 0,
        "corpus directory {dir:?} present but no case save files found — \
         the harness must exercise at least one real save"
    );
    eprintln!("corpus harness checked {checked} real-save case(s)");
}
