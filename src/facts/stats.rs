//! Character stats facts (ADR-0010, slice §04 / issue #7).
//!
//! The plainest fact family: no flag resolution (slices #4/#5) and no GaItem decode
//! (#6/#7), just the scalar character sheet read straight off `PlayerGameData` at its
//! documented offsets. Mirrors the reader's `StatsViewModel::from_save`
//! (`src/vm/stats.rs`); the union widens it with the fields only elden-map surfaced —
//! the hp/fp/stamina `base_max` values — so the core carries the superset once.
//!
//! `level` and `class_id` are **not** here — they are identity (slice §01) and already
//! on [`super::super::ReconstructedCharacter`]. Everything else on the sheet is a stat.
//! The canonical fact names follow the union vocabulary: the save's `souls` /
//! `soulsmemory` / `sp` are **runes** / **runes_memory** / **stamina** here.

use crate::save::common::save_slot::PlayerGameData;

/// The character sheet as facts: the eight attributes, the two rune totals, the
/// hp/fp/stamina triples (current / max / base-max), and the two DLC blessing levels.
/// Pure scalars read from the save — no derivation, no display. A non-DLC save simply
/// reports `scadutree_level` / `spirit_ash_level` of 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Stats {
    pub vigor: u32,
    pub mind: u32,
    pub endurance: u32,
    pub strength: u32,
    pub dexterity: u32,
    pub intelligence: u32,
    pub faith: u32,
    pub arcane: u32,

    /// Runes currently held.
    pub runes: u32,
    /// Lifetime runes earned (the match-making figure). Never less than [`Self::runes`].
    pub runes_memory: u32,

    pub hp: u32,
    pub max_hp: u32,
    pub base_max_hp: u32,
    pub fp: u32,
    pub max_fp: u32,
    pub base_max_fp: u32,
    pub stamina: u32,
    pub max_stamina: u32,
    pub base_max_stamina: u32,

    /// DLC scadutree blessing level (0 on a base-game / no-DLC save).
    pub scadutree_level: u8,
    /// DLC revered spirit-ash blessing level (0 on a base-game / no-DLC save).
    pub spirit_ash_level: u8,
}

/// Read the stats sheet from a slot's [`PlayerGameData`]. A direct field mapping; the
/// only translation is the union naming (`souls`→`runes`, `sp`→`stamina`).
pub fn resolve_stats(pgd: &PlayerGameData) -> Stats {
    Stats {
        vigor: pgd.vigor,
        mind: pgd.mind,
        endurance: pgd.endurance,
        strength: pgd.strength,
        dexterity: pgd.dexterity,
        intelligence: pgd.intelligence,
        faith: pgd.faith,
        arcane: pgd.arcane,

        runes: pgd.souls,
        runes_memory: pgd.soulsmemory,

        hp: pgd.health,
        max_hp: pgd.max_health,
        base_max_hp: pgd.base_max_health,
        fp: pgd.fp,
        max_fp: pgd.max_fp,
        base_max_fp: pgd.base_max_fp,
        stamina: pgd.sp,
        max_stamina: pgd.max_sp,
        base_max_stamina: pgd.base_max_sp,

        scadutree_level: pgd.scadutree_lvl,
        spirit_ash_level: pgd.spirit_ash_lvl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // PlayerGameData has private padding fields, so it can only be built by mutating a
    // default (the same pattern its own `Read` impl and the reader's VM use). Distinct
    // sentinel values per field catch a mapping that swapped two fields.
    #[allow(clippy::field_reassign_with_default)]
    fn resolve_stats_maps_every_field() {
        let mut pgd = PlayerGameData::default();
        pgd.vigor = 1;
        pgd.mind = 2;
        pgd.endurance = 3;
        pgd.strength = 4;
        pgd.dexterity = 5;
        pgd.intelligence = 6;
        pgd.faith = 7;
        pgd.arcane = 8;
        pgd.souls = 100;
        pgd.soulsmemory = 200;
        pgd.health = 500;
        pgd.max_health = 510;
        pgd.base_max_health = 505;
        pgd.fp = 70;
        pgd.max_fp = 75;
        pgd.base_max_fp = 72;
        pgd.sp = 90;
        pgd.max_sp = 95;
        pgd.base_max_sp = 92;
        pgd.scadutree_lvl = 20;
        pgd.spirit_ash_lvl = 10;

        assert_eq!(
            resolve_stats(&pgd),
            Stats {
                vigor: 1,
                mind: 2,
                endurance: 3,
                strength: 4,
                dexterity: 5,
                intelligence: 6,
                faith: 7,
                arcane: 8,
                runes: 100,
                runes_memory: 200,
                hp: 500,
                max_hp: 510,
                base_max_hp: 505,
                fp: 70,
                max_fp: 75,
                base_max_fp: 72,
                stamina: 90,
                max_stamina: 95,
                base_max_stamina: 92,
                scadutree_level: 20,
                spirit_ash_level: 10,
            }
        );
    }
}
