//! The Character Reconstructor's entry point (ADR-0010, CONTEXT.md →
//! *Reconstruction architecture*).
//!
//! `reconstruct(bytes, slot)` is the pure black box: `(save bytes, slot index) →
//! ReconstructedCharacter`. Deterministic, no I/O, no timers, no rendering — the
//! caller owns the file, the display, and the naming. This is what makes it safe
//! to share across the reader and elden-map, native and WASM.
//!
//! # Seed contract
//!
//! `ReconstructedCharacter` is **seeded** with character identity only — name,
//! level, class. The contract is **append-only**: every later slice (bosses,
//! pickups, inventory, stats, position) adds fields, never reshapes the ones
//! here. It carries **facts**: ID-keyed resolved state, no display strings and no
//! map coordinates. `class` is the raw archetype id from this save's bytes; the
//! id → "Vagabond"/"Astrologer" name is a Canonical Name lookup that lives in
//! each app's Enrichment stage, never baked in here.

use std::io;

use binary_reader::BinaryReader;
use wasm_event_flags::ResolvedFlags;

use crate::facts::{
    resolve_bosses, resolve_dungeon_pickups, resolve_graces, resolve_world_pickups, FlagFact,
};
use crate::read::read::Read;
use crate::save::save::save::Save;

/// The number of character slots a save carries.
const SLOT_COUNT: usize = 10;

/// One slot's reconstructed facts (ADR-0010). Append-only across slices.
///
/// Facts only. `name` is the player's own name decoded from the slot (a
/// reconstructed value, not a game display label); `level` is the character
/// level; `class_id` is the raw archetype id (0 = Vagabond … 9 = Wretch).
///
/// `graces` and `bosses` (slice #4) are the first flag-derived facts: each is
/// every known grace / boss-defeat flag id paired with its tri-state for this
/// save, ascending by id, resolved per save through `wasm-event-flags` — no
/// hardcoded flag positions (ADR-0008). Their `Unknown` state means "position
/// unresolvable", not "clear". `world_pickups` and `dungeon_pickups` (slice #5)
/// are the same shape keyed by `getItemFlagId`, routed to the pickup families;
/// low/simple-flag world ids that belong to no pickup family read `Unknown`, as
/// the reader shows them. No display names, no coordinates: id → item/name is a
/// Canonical Name lookup in each app's Enrichment stage. Later slices append
/// fields; they never change the ones here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReconstructedCharacter {
    pub name: String,
    pub level: u32,
    pub class_id: u8,
    pub graces: Vec<FlagFact>,
    pub bosses: Vec<FlagFact>,
    pub world_pickups: Vec<FlagFact>,
    pub dungeon_pickups: Vec<FlagFact>,
}

/// Why a reconstruction could not be produced. Distinct from a *fact* that a
/// slot is empty: these mean "no character to reconstruct here", not "a character
/// whose fields are zero".
#[derive(Debug)]
pub enum ReconstructError {
    /// `slot` is outside `0..SLOT_COUNT`.
    SlotOutOfRange(usize),
    /// The bytes are neither a recognised PC nor a PS Save Wizard save.
    UnrecognizedSave,
    /// The save parsed, but `slot` holds no active character.
    InactiveSlot(usize),
    /// The bytes recognised as a save but failed to parse.
    Parse(io::Error),
}

impl std::fmt::Display for ReconstructError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReconstructError::SlotOutOfRange(s) => {
                write!(f, "slot {s} is out of range (0..{SLOT_COUNT})")
            }
            ReconstructError::UnrecognizedSave => {
                write!(f, "bytes are not a recognised Elden Ring save")
            }
            ReconstructError::InactiveSlot(s) => write!(f, "slot {s} holds no active character"),
            ReconstructError::Parse(e) => write!(f, "failed to parse save: {e}"),
        }
    }
}

impl std::error::Error for ReconstructError {}

/// Reconstruct one slot's character identity from raw save bytes.
///
/// Pure and deterministic: it reads `bytes`, never the filesystem. The caller is
/// responsible for having loaded the save.
pub fn reconstruct(bytes: &[u8], slot: usize) -> Result<ReconstructedCharacter, ReconstructError> {
    if slot >= SLOT_COUNT {
        return Err(ReconstructError::SlotOutOfRange(slot));
    }

    let mut br = BinaryReader::from_u8(bytes);
    br.set_endian(binary_reader::Endian::Little);

    if !Save::is(&mut br) {
        return Err(ReconstructError::UnrecognizedSave);
    }

    let save = Save::read(&mut br).map_err(ReconstructError::Parse)?;

    if !save.save_type.active_slots()[slot] {
        return Err(ReconstructError::InactiveSlot(slot));
    }

    let pgd = save
        .save_type
        .get_player_game_data(slot)
        .ok_or(ReconstructError::UnrecognizedSave)?;

    // Resolve the flag region's Origin ONCE and share it across every flag fact:
    // pinning it is a ~13,400-byte scan, and graces + bosses read from the same
    // region. `None` (unresolvable origin, or no flag bytes for this slot) makes
    // every flag read Unknown rather than a guessed Clear (ADR-0008).
    let event_flags = save.save_type.get_event_flags(slot);
    let resolved = event_flags.and_then(ResolvedFlags::from_event_flags);

    Ok(ReconstructedCharacter {
        name: decode_name(&pgd.character_name),
        level: pgd.level,
        class_id: pgd.arche_type,
        graces: resolve_graces(resolved.as_ref()),
        bosses: resolve_bosses(resolved.as_ref()),
        world_pickups: resolve_world_pickups(resolved.as_ref()),
        dungeon_pickups: resolve_dungeon_pickups(resolved.as_ref()),
    })
}

/// Decode a fixed-width, null-terminated UTF-16LE character name into a `String`,
/// stopping at the first null. Lossy: an unpaired surrogate becomes U+FFFD rather
/// than failing the whole reconstruction.
fn decode_name(raw: &[u16]) -> String {
    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    String::from_utf16_lossy(&raw[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_name_stops_at_null() {
        let mut raw = [0u16; 0x10];
        for (i, c) in "Tarnished".encode_utf16().enumerate() {
            raw[i] = c;
        }
        assert_eq!(decode_name(&raw), "Tarnished");
    }

    #[test]
    fn decode_name_empty_is_empty_string() {
        assert_eq!(decode_name(&[0u16; 0x10]), "");
    }

    #[test]
    fn unrecognized_bytes_error_not_panic() {
        assert!(matches!(
            reconstruct(&[0u8; 64], 0),
            Err(ReconstructError::UnrecognizedSave)
        ));
    }

    #[test]
    fn slot_out_of_range_is_rejected_before_parsing() {
        assert!(matches!(
            reconstruct(&[], SLOT_COUNT),
            Err(ReconstructError::SlotOutOfRange(SLOT_COUNT))
        ));
    }
}
