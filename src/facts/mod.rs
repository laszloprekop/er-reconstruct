//! Reconstructed facts above bare identity (ADR-0010). The seed carried name /
//! level / class; this module adds the first **flag-derived** facts — graces and
//! boss defeats — and holds the machinery every later flag slice reuses.
//!
//! # What a flag fact is, and is not
//!
//! A fact is `{ id, state }`: an event-flag id paired with its resolved tri-state
//! for *this* save. The id is game knowledge (which ids are graces, which are
//! bosses — see [`flag_ids`]); the state is resolved per save through
//! `wasm-event-flags`, never from a base table baked in here (ADR-0008). The
//! `Unknown` state is load-bearing: it means the flag's position could not be
//! pinned, which is distinct from `Clear`, and collapsing the two is exactly the
//! bug that once reported 0 boss defeats on a finished character
//! (`CONTEXT.md` → *Unknown*). Names for these ids stay in each app's Enrichment.

pub mod equipment;
pub mod flag_ids;
pub mod inventory;
pub mod pickup_ids;
pub mod stats;

use wasm_event_flags::{FlagState, ResolvedFlags};

pub use equipment::{resolve_equipment, EquipSlot, EquipmentFact};
pub use flag_ids::{BOSS_FLAG_IDS, GRACE_FLAG_IDS};
pub use inventory::{
    resolve_held_common, resolve_held_key_items, InventoryFact, ItemCategory,
};
pub use pickup_ids::{DUNGEON_PICKUP_FLAG_IDS, WORLD_PICKUP_FLAG_IDS};
pub use stats::{resolve_stats, Stats};

/// A flag's resolved tri-state, serialized by name (`"Set"`/`"Clear"`/
/// `"Unknown"`). A local mirror of `wasm_event_flags::FlagState` so the fact set
/// can be `Serialize`/`Eq` (the wasm type is neither) and so the JSON boundary —
/// the `native == WASM` parity comparison — carries a stable, self-describing
/// token rather than a bare integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum FlagStatus {
    Set,
    Clear,
    Unknown,
}

impl From<FlagState> for FlagStatus {
    fn from(s: FlagState) -> Self {
        match s {
            FlagState::Set => FlagStatus::Set,
            FlagState::Clear => FlagStatus::Clear,
            FlagState::Unknown => FlagStatus::Unknown,
        }
    }
}

/// One event-flag fact: the id and its resolved state for this save.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FlagFact {
    pub id: u32,
    pub state: FlagStatus,
}

/// Route a boss-defeat flag id to its Flag Family and read it from an
/// already-resolved region. Mirrors the reader's `world_flag_state`: bosses mean
/// "defeated", so a 10-digit tile id resolves as `tile_world` (never the pickup
/// family 500 bytes away), and legacy-map ids as `dungeon`. The ambiguity
/// CLAUDE.md warns about is resolved by the caller's semantics here, not by the
/// value. Holds no base table — it delegates to the per-save `ResolvedFlags`.
fn boss_family_state(resolved: &ResolvedFlags, id: u32) -> FlagStatus {
    match id {
        1_000_000_000..=1_999_999_999 => resolved.tile_world(id),
        10_000_000..=999_999_999 => resolved.dungeon(id),
        50_000..=79_999 => resolved.world_state(id),
        _ => FlagState::Unknown,
    }
    .into()
}

/// Grace facts for this save's flag region: every [`GRACE_FLAG_IDS`] id paired
/// with its resolved state, ascending. `resolved` is `None` when the origin could
/// not be pinned — then every grace reads `Unknown` rather than a guessed `Clear`.
pub fn resolve_graces(resolved: Option<&ResolvedFlags>) -> Vec<FlagFact> {
    GRACE_FLAG_IDS
        .iter()
        .map(|&id| FlagFact {
            id,
            // Graces are world-state block flags; resolved by id directly.
            state: resolved.map_or(FlagStatus::Unknown, |r| r.world_state(id).into()),
        })
        .collect()
}

/// Boss-defeat facts for this save's flag region: every [`BOSS_FLAG_IDS`] id paired
/// with its resolved state, ascending. Routes each id to its family; `None` origin
/// yields all `Unknown`.
pub fn resolve_bosses(resolved: Option<&ResolvedFlags>) -> Vec<FlagFact> {
    BOSS_FLAG_IDS
        .iter()
        .map(|&id| FlagFact {
            id,
            state: resolved.map_or(FlagStatus::Unknown, |r| boss_family_state(r, id)),
        })
        .collect()
}

/// Route a **pickup** `getItemFlagId` to its Flag Family and read it. Mirrors the
/// reader's `pickup_state` — the `_pickup` siblings of `boss_family_state`, because
/// these ids mean "item collected", never "boss defeated": a 10-digit tile id is
/// `tile_pickup` (whose region sits 500 bytes from `tile_world`; the caller's
/// semantics pick, never the value — CLAUDE.md), a legacy-map id is
/// `dungeon_pickup`. Ids in neither family (low/simple/block flags a few world
/// rows carry) read `Unknown`, exactly as the reader shows them. No base table
/// here — it delegates to the per-save `ResolvedFlags`.
fn pickup_family_state(resolved: &ResolvedFlags, id: u32) -> FlagStatus {
    match id {
        1_000_000_000..=1_999_999_999 => resolved.tile_pickup(id),
        10_000_000..=999_999_999 => resolved.dungeon_pickup(id),
        50_000..=79_999 => resolved.world_state(id),
        _ => FlagState::Unknown,
    }
    .into()
}

/// World-pickup facts: every [`WORLD_PICKUP_FLAG_IDS`] id paired with its resolved
/// state, ascending. Routes each id to its family; `None` origin yields all
/// `Unknown`.
pub fn resolve_world_pickups(resolved: Option<&ResolvedFlags>) -> Vec<FlagFact> {
    WORLD_PICKUP_FLAG_IDS
        .iter()
        .map(|&id| FlagFact {
            id,
            state: resolved.map_or(FlagStatus::Unknown, |r| pickup_family_state(r, id)),
        })
        .collect()
}

/// Legacy-dungeon-pickup facts: every [`DUNGEON_PICKUP_FLAG_IDS`] id (localId >=
/// 7000) paired with its resolved state, ascending. All resolve through the
/// legacy-dungeon pickup family; `None` origin yields all `Unknown`.
pub fn resolve_dungeon_pickups(resolved: Option<&ResolvedFlags>) -> Vec<FlagFact> {
    DUNGEON_PICKUP_FLAG_IDS
        .iter()
        .map(|&id| FlagFact {
            id,
            state: resolved.map_or(FlagStatus::Unknown, |r| pickup_family_state(r, id)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_sorted_unique() {
        for table in [
            GRACE_FLAG_IDS,
            BOSS_FLAG_IDS,
            WORLD_PICKUP_FLAG_IDS,
            DUNGEON_PICKUP_FLAG_IDS,
        ] {
            assert!(
                table.windows(2).all(|w| w[0] < w[1]),
                "flag-id tables must be strictly ascending (sorted + deduped)"
            );
        }
    }

    #[test]
    fn pickup_tables_do_not_overlap() {
        // The reader's two tables partition the primary source exactly; a shared
        // id would mean one pickup double-counted across the world/dungeon facts.
        for &id in DUNGEON_PICKUP_FLAG_IDS {
            assert!(
                WORLD_PICKUP_FLAG_IDS.binary_search(&id).is_err(),
                "id {id} appears in both pickup tables"
            );
        }
    }

    #[test]
    fn every_dungeon_pickup_is_legacy_dungeon_family() {
        // dungeon_pickup only answers the legacy-map range; an out-of-range id
        // would read Unknown forever.
        for &id in DUNGEON_PICKUP_FLAG_IDS {
            assert!(
                (10_000_000..1_000_000_000).contains(&id),
                "dungeon pickup id {id} is not in the legacy-dungeon range"
            );
        }
    }

    #[test]
    fn every_boss_id_routes_to_a_known_family() {
        // No boss id may fall through boss_family_state's ranges — a fall-through
        // is a silent Unknown that would read as "not defeated" forever.
        for &id in BOSS_FLAG_IDS {
            let routed = matches!(id,
                1_000_000_000..=1_999_999_999 | 10_000_000..=999_999_999 | 50_000..=79_999);
            assert!(routed, "boss flag id {id} routes to no family");
        }
    }

    #[test]
    fn every_grace_id_is_a_world_state_flag() {
        // world_state() only answers [50000, 80000); an id outside that reads
        // Unknown unconditionally. Catch a bad table before it ships.
        for &id in GRACE_FLAG_IDS {
            assert!((50_000..80_000).contains(&id), "grace flag id {id} is not world-state");
        }
    }

    #[test]
    fn no_flags_resolvable_means_all_unknown() {
        // A None region (unresolvable origin) must produce Unknown for every id,
        // never a defaulted Clear.
        let facts = [
            (resolve_graces(None), GRACE_FLAG_IDS.len()),
            (resolve_bosses(None), BOSS_FLAG_IDS.len()),
            (resolve_world_pickups(None), WORLD_PICKUP_FLAG_IDS.len()),
            (resolve_dungeon_pickups(None), DUNGEON_PICKUP_FLAG_IDS.len()),
        ];
        for (produced, want_len) in facts {
            assert_eq!(produced.len(), want_len);
            assert!(produced.iter().all(|f| f.state == FlagStatus::Unknown));
            // Facts preserve the table's ascending id order.
            assert!(produced.windows(2).all(|w| w[0].id < w[1].id));
        }
    }
}
