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

use er_reconstruct::{EquipSlot, FlagFact, FlagStatus, InventoryFact, ItemCategory};
use serde_json::Value;

/// The manifest names equipment slots with the same tokens `EquipSlot` serializes to.
fn slot_name(s: EquipSlot) -> &'static str {
    match s {
        EquipSlot::RightHand1 => "RightHand1",
        EquipSlot::RightHand2 => "RightHand2",
        EquipSlot::RightHand3 => "RightHand3",
        EquipSlot::LeftHand1 => "LeftHand1",
        EquipSlot::LeftHand2 => "LeftHand2",
        EquipSlot::LeftHand3 => "LeftHand3",
        EquipSlot::Arrow1 => "Arrow1",
        EquipSlot::Arrow2 => "Arrow2",
        EquipSlot::Bolt1 => "Bolt1",
        EquipSlot::Bolt2 => "Bolt2",
        EquipSlot::Head => "Head",
        EquipSlot::Chest => "Chest",
        EquipSlot::Arms => "Arms",
        EquipSlot::Legs => "Legs",
        EquipSlot::Talisman1 => "Talisman1",
        EquipSlot::Talisman2 => "Talisman2",
        EquipSlot::Talisman3 => "Talisman3",
        EquipSlot::Talisman4 => "Talisman4",
    }
}

/// The manifest spells item categories with the same tokens `ItemCategory`
/// serializes to.
fn category_name(c: ItemCategory) -> &'static str {
    match c {
        ItemCategory::Weapon => "Weapon",
        ItemCategory::Armor => "Armor",
        ItemCategory::Accessory => "Accessory",
        ItemCategory::Item => "Item",
        ItemCategory::Aow => "Aow",
    }
}

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

        // Held-inventory facts (slice #6). The counts cross-check against the reader
        // export's distinct counts — a decode that dropped or invented a held item
        // moves them. `items` pins specific known-truth held items (the inventory
        // analogue of `flags`), searched across both held lists.
        if let Some(want) = expected["held_common_count"].as_u64() {
            assert_eq!(
                got.held_inventory.len() as u64, want,
                "held_common_count mismatch for {save_name}#{slot}"
            );
        }
        if let Some(want) = expected["held_key_count"].as_u64() {
            assert_eq!(
                got.held_key_items.len() as u64, want,
                "held_key_count mismatch for {save_name}#{slot}"
            );
        }
        if let Some(items) = expected["items"].as_array() {
            for want in items {
                let category = want["category"].as_str().expect("item category");
                let item_id = want["id"].as_u64().or_else(|| want["item_id"].as_u64())
                    .expect("item_id") as u32;
                let found = got
                    .held_inventory
                    .iter()
                    .chain(got.held_key_items.iter())
                    .find(|f: &&InventoryFact| {
                        f.item_id == item_id && category_name(f.category) == category
                    })
                    .unwrap_or_else(|| {
                        panic!("held item {category} {item_id} not found for {save_name}#{slot}")
                    });
                if let Some(qty) = want["quantity"].as_u64() {
                    assert_eq!(
                        u64::from(found.quantity), qty,
                        "quantity mismatch for held item {category} {item_id} ({save_name}#{slot})"
                    );
                }
            }
        }

        // Equipment facts (slice #7). `equipment_count` cross-checks against the
        // reader export's occupied fact-relevant slots (hands, projectiles, the four
        // armor pieces, talismans — NOT quick-slots/pouch, which are not equipment
        // facts). "Unarmed" hand slots carry a real id (110000), so they count as
        // occupied, exactly as the export shows them. Targeted `equipment` pins
        // per-slot known-truth: item_id always, `upgrade` only where meaningful — the
        // export over-computes `item_id % 100` for every item, so a talisman anchors
        // its id alone (its "upgrade" in the export is a display artifact; the fact
        // correctly reports 0).
        if let Some(want) = expected["equipment_count"].as_u64() {
            assert_eq!(
                got.equipment.len() as u64, want,
                "equipment_count mismatch for {save_name}#{slot}"
            );
        }
        if let Some(items) = expected["equipment"].as_array() {
            for want in items {
                let slot_want = want["slot"].as_str().expect("equipment slot name");
                let fact = got
                    .equipment
                    .iter()
                    .find(|f| slot_name(f.slot) == slot_want)
                    .unwrap_or_else(|| {
                        panic!("equipment slot {slot_want} not occupied for {save_name}#{slot}")
                    });
                assert_eq!(
                    u64::from(fact.item_id),
                    want["item_id"].as_u64().expect("equipment item_id"),
                    "equipment item_id mismatch at {slot_want} for {save_name}#{slot}"
                );
                if let Some(up) = want["upgrade"].as_u64() {
                    assert_eq!(
                        u64::from(fact.upgrade), up,
                        "equipment upgrade mismatch at {slot_want} for {save_name}#{slot}"
                    );
                }
            }
        }

        // Stats facts (slice §04). The manifest pins the fields with an INDEPENDENT
        // export oracle (the reader's ExportStats attributes + DLC levels, its general
        // block's runes, and — on newer exports — the hp/fp/stamina current/max); every
        // pinned field is checked exactly. base_max has no export oracle, so it (and the
        // rest) is guarded by universal known-truth INVARIANTS that no correct save can
        // violate: an active character has positive maxes; a current value never exceeds
        // its max; base-max is the unbuffed floor so it never exceeds max; and lifetime
        // runes are never fewer than runes currently held. A field-swap or wrong-offset
        // decode trips one of these.
        if let Some(st) = expected["stats"].as_object() {
            let s = &got.stats;
            let exact: [(&str, u64); 18] = [
                ("vigor", s.vigor.into()),
                ("mind", s.mind.into()),
                ("endurance", s.endurance.into()),
                ("strength", s.strength.into()),
                ("dexterity", s.dexterity.into()),
                ("intelligence", s.intelligence.into()),
                ("faith", s.faith.into()),
                ("arcane", s.arcane.into()),
                ("runes", s.runes.into()),
                ("runes_memory", s.runes_memory.into()),
                ("hp", s.hp.into()),
                ("max_hp", s.max_hp.into()),
                ("fp", s.fp.into()),
                ("max_fp", s.max_fp.into()),
                ("stamina", s.stamina.into()),
                ("max_stamina", s.max_stamina.into()),
                ("scadutree_level", s.scadutree_level.into()),
                ("spirit_ash_level", s.spirit_ash_level.into()),
            ];
            for (key, got_val) in exact {
                if let Some(want) = st.get(key).and_then(Value::as_u64) {
                    assert_eq!(
                        got_val, want,
                        "stats.{key} mismatch for {save_name}#{slot} (reconstruct {got_val} vs oracle {want})"
                    );
                }
            }
            assert!(
                s.max_hp > 0 && s.max_fp > 0 && s.max_stamina > 0,
                "stats maxes must be positive for active {save_name}#{slot}"
            );
            assert!(
                s.hp <= s.max_hp && s.fp <= s.max_fp && s.stamina <= s.max_stamina,
                "stats current exceeds max for {save_name}#{slot}"
            );
            assert!(
                s.base_max_hp <= s.max_hp
                    && s.base_max_fp <= s.max_fp
                    && s.base_max_stamina <= s.max_stamina,
                "stats base-max exceeds max for {save_name}#{slot}"
            );
            assert!(
                s.runes_memory >= s.runes,
                "stats lifetime runes below held for {save_name}#{slot}"
            );
        }

        // World-position facts (slice §09). The oracle is elden-map's extractor, which
        // is the same shared `extract_player_position` scan the core now calls — so this
        // guards the PLUMBING (the raw-slot-blob offset + slice) more than the resolver:
        // a wrong offset reads another slot's or garbage bytes and the position moves or
        // vanishes. map_id is pinned exactly (robust integer known-truth), and x/y/z are
        // checked finite, in the scan's valid range, and near the known anchor (a loose
        // tolerance, so float formatting is never the thing under test).
        if let Some(wp_want) = expected["world_position"].as_object() {
            let wp = got.world_position.as_ref().unwrap_or_else(|| {
                panic!("world_position resolved None for {save_name}#{slot}")
            });
            if let Some(map) = wp_want["map_id"].as_array() {
                let want: Vec<u64> = map.iter().filter_map(Value::as_u64).collect();
                let got_map: Vec<u64> = wp.map_id.iter().map(|&b| b as u64).collect();
                assert_eq!(
                    got_map, want,
                    "world_position.map_id mismatch for {save_name}#{slot}"
                );
            }
            for (name, got_v, key) in [("x", wp.x, "x"), ("y", wp.y, "y"), ("z", wp.z, "z")] {
                assert!(
                    got_v.is_finite() && got_v.abs() < 10_000.0,
                    "world_position.{name} out of range ({got_v}) for {save_name}#{slot}"
                );
                if let Some(want) = wp_want[key].as_f64() {
                    assert!(
                        (f64::from(got_v) - want).abs() < 0.5,
                        "world_position.{name} {got_v} not near anchor {want} for {save_name}#{slot}"
                    );
                }
            }
        }

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
