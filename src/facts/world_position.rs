//! Player world-position facts (ADR-0010, slice §09 / issue #8).
//!
//! The archetypal **union add**: this reader has no reason to surface where a character
//! is standing, but elden-map needs it to place a pin, so the fact is ported *into* the
//! core rather than dropped. Unlike every other slice there is **no reader oracle** —
//! elden-map's extractor is the only prior art, and its "single source of truth" is the
//! signature scan `wasm_event_flags::extract_player_position` (the crate this core
//! already depends on). So this slice does not re-implement the scan; it *surfaces the
//! same resolver as a fact*, exactly as graces/bosses surface `ResolvedFlags`. Calling
//! the identical function is what guarantees the core's value equals elden-map's today,
//! so elden-map can later delete its copy with no drift.
//!
//! Position lives at a **dynamic** offset (it trails the per-save event-flags region),
//! which is why the resolver scans for the map-id signature instead of reading a fixed
//! offset — and why the core's own structural `player_coords` field reads zero and is
//! not used here. The scan needs the whole slot blob; [`super::super::reconstruct`]
//! re-slices it from the save bytes via `get_raw_slot_start`.

/// Where the character is standing. The primary coordinate triple and the map/block id
/// are the load-bearing facts (a map pin); `x2`/`y2`/`z2` are the secondary coordinates
/// the save also carries, and `facing_angle` the heading — all part of elden-map's
/// `PlayerPosition`, so all carried in the union. Turning any of this into a screen pin
/// or POI label is elden-map's Enrichment. No `f32` field can be `Eq`, so this fact (and
/// `ReconstructedCharacter`) is `PartialEq` only.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct WorldPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub x2: f32,
    pub y2: f32,
    pub z2: f32,
    pub facing_angle: f32,
    /// The four raw map-id bytes (region / area / block / … ) identifying which map the
    /// coordinates are in. Kept as bytes; naming the map is Enrichment.
    pub map_id: [u8; 4],
}

/// Resolve the player's position from a slot's raw blob via the shared signature scan.
/// `None` when the scan finds no valid position (e.g. a brand-new character with no
/// world state yet) — an honest absence, never guessed coordinates. The scan's `offset`
/// is dropped: it is a per-save artifact, not identity.
pub fn resolve_world_position(slot_data: &[u8]) -> Option<WorldPosition> {
    let r = wasm_event_flags::extract_player_position_impl(slot_data);
    r.valid.then_some(WorldPosition {
        x: r.x,
        y: r.y,
        z: r.z,
        x2: r.x2,
        y2: r.y2,
        z2: r.z2,
        facing_angle: r.facing_angle,
        map_id: [r.map_id_0, r.map_id_1, r.map_id_2, r.map_id_3],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn too_short_or_signatureless_slot_is_none() {
        // The scan needs a real slot; empty or all-zero bytes carry no position
        // signature, so the fact is honestly absent (not zeroed coordinates).
        assert_eq!(resolve_world_position(&[]), None);
        assert_eq!(resolve_world_position(&[0u8; 4096]), None);
    }
}
