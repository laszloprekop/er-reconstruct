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

pub mod flag_ids;

use wasm_event_flags::{FlagState, ResolvedFlags};

pub use flag_ids::{BOSS_FLAG_IDS, GRACE_FLAG_IDS};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_sorted_unique() {
        for table in [GRACE_FLAG_IDS, BOSS_FLAG_IDS] {
            assert!(
                table.windows(2).all(|w| w[0] < w[1]),
                "flag-id tables must be strictly ascending (sorted + deduped)"
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
        let graces = resolve_graces(None);
        let bosses = resolve_bosses(None);
        assert_eq!(graces.len(), GRACE_FLAG_IDS.len());
        assert_eq!(bosses.len(), BOSS_FLAG_IDS.len());
        assert!(graces.iter().all(|f| f.state == FlagStatus::Unknown));
        assert!(bosses.iter().all(|f| f.state == FlagStatus::Unknown));
        // Facts preserve the table's ascending id order.
        assert!(graces.windows(2).all(|w| w[0].id < w[1].id));
        assert!(bosses.windows(2).all(|w| w[0].id < w[1].id));
    }
}
