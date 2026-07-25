//! Equipment facts (ADR-0010, slice #7) — what a character has *equipped*, built on
//! the #6 GaItem-decode foundation ([`super::inventory`]).
//!
//! Held inventory (#6) is a *list* of goods; equipment is **positional** — fixed
//! slots, each either empty or holding one item: the two hands (three armament slots
//! each), arrows and bolts (two each), the four armor pieces, and four talismans.
//! The save keeps the loadout in `chr_asm2` as **gaitem handles**, one per slot; the
//! decode is exactly the reader's `EquipmentViewModel::from_save`
//! (`src/vm/equipment.rs`):
//!
//! - **Weapons** (hands) and **projectiles** (arrows/bolts) indirect through the
//!   slot's gaitem map ([`map_item_id`]) to a param id. A weapon id is the full
//!   reinforced value (base id + upgrade in its low two digits); the upgrade level is
//!   surfaced alongside it as `item_id % 100`, exactly as the reader export does.
//! - **Armor** indirects through the map, then clears the armor tag
//!   ([`ITEM_TYPE_ARMOR`]) off the map id.
//! - **Talismans** need no indirection — the id is the handle with its accessory tag
//!   ([`HANDLE_ACCESSORY`]) cleared, the same direct XOR held accessories use.
//!
//! # A fact is one occupied slot
//!
//! The user chose a flat `Vec<EquipmentFact>` over a fixed struct (append-only
//! contract): each fact names its [`EquipSlot`] and carries item identity + weapon
//! upgrade. **Only occupied slots** appear — an empty slot (a `0` or `u32::MAX`
//! sentinel in the handle or the indirected id) is simply absent, never a bogus fact.
//! Facts come in [`EquipSlot`] order so `native == WASM` compares element-for-element.
//! Names and icons are Enrichment and stay in each app; quick-slot and pouch loadouts
//! are not equipment facts and are omitted (a later slice may append them).

use crate::facts::inventory::{map_item_id, HANDLE_ACCESSORY, ITEM_TYPE_ARMOR};
use crate::save::common::save_slot::{ChrAsm2, GaItem};

/// Which equipment slot a fact describes. Positional game knowledge, serialized by
/// name so the parity boundary carries a stable token (like [`super::FlagStatus`] /
/// [`super::ItemCategory`]). Definition order is the fact-emission order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum EquipSlot {
    RightHand1,
    RightHand2,
    RightHand3,
    LeftHand1,
    LeftHand2,
    LeftHand3,
    Arrow1,
    Arrow2,
    Bolt1,
    Bolt2,
    Head,
    Chest,
    Arms,
    Legs,
    Talisman1,
    Talisman2,
    Talisman3,
    Talisman4,
}

/// One equipment fact: an occupied slot's identity. `item_id` is the equipped item's
/// param id — for a weapon the full reinforced value the gaitem map stores, matching
/// [`super::InventoryFact`]. `upgrade` is the reinforcement level (`item_id % 100`) for
/// weapons and `0` for everything else. No handle, no name (churn / Enrichment).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EquipmentFact {
    pub slot: EquipSlot,
    pub item_id: u32,
    pub upgrade: u32,
}

/// An empty equipment slot carries one of two sentinels: `0` (no handle) or
/// `u32::MAX` (`0xFFFFFFFF`, a *cleared* slot — and its gaitem-map entry indirects to
/// `u32::MAX` too, so the sentinel survives the map lookup). Either means "nothing
/// equipped"; a fact is emitted only for a real id. Missing this second sentinel is
/// how a cleared arrow/bolt slot leaks out as a bogus `item_id 4294967295`.
fn is_empty(v: u32) -> bool {
    v == 0 || v == u32::MAX
}

/// Decode a **weapon** hand slot: indirect the handle to its full reinforced param id.
/// `None` for an empty slot (handle 0 / not in map, or an empty sentinel) — refuse,
/// don't guess. Hand slots are never truly empty (they fall back to Unarmed, 110000),
/// but the guard is uniform with the other slots.
fn weapon(slot: EquipSlot, handle: u32, ga_items: &[GaItem]) -> Option<EquipmentFact> {
    let item_id = map_item_id(handle, ga_items)?;
    (!is_empty(item_id)).then_some(EquipmentFact {
        slot,
        item_id,
        upgrade: item_id % 100,
    })
}

/// Decode a **projectile** (arrow/bolt) slot: same map indirection as a weapon, but
/// projectiles carry no reinforcement, so `upgrade` is 0. Empty quivers are the common
/// case and drop on the sentinel check.
fn projectile(slot: EquipSlot, handle: u32, ga_items: &[GaItem]) -> Option<EquipmentFact> {
    let item_id = map_item_id(handle, ga_items)?;
    (!is_empty(item_id)).then_some(EquipmentFact {
        slot,
        item_id,
        upgrade: 0,
    })
}

/// Decode an **armor** slot: indirect through the map, then clear the armor tag off the
/// map id. An empty slot's map id is a sentinel (the reader guards `!= 0`; we also drop
/// `u32::MAX`), so it drops before the XOR.
fn armor(slot: EquipSlot, handle: u32, ga_items: &[GaItem]) -> Option<EquipmentFact> {
    let mapped = map_item_id(handle, ga_items)?;
    (!is_empty(mapped)).then_some(EquipmentFact {
        slot,
        item_id: mapped ^ ITEM_TYPE_ARMOR,
        upgrade: 0,
    })
}

/// Decode a **talisman** slot: no indirection — the id is the handle with its accessory
/// tag cleared. An empty slot's handle is a sentinel, checked before the XOR.
fn talisman(slot: EquipSlot, handle: u32) -> Option<EquipmentFact> {
    (!is_empty(handle)).then_some(EquipmentFact {
        slot,
        item_id: handle ^ HANDLE_ACCESSORY,
        upgrade: 0,
    })
}

/// Equipment facts for this save's slot: every occupied `chr_asm2` slot decoded to a
/// fact, in [`EquipSlot`] order. Mirrors the reader's `EquipmentViewModel::from_save`
/// exactly, minus the quick-slot/pouch loadout (not equipment facts). Empty slots are
/// absent, never zero-id facts.
pub fn resolve_equipment(asm: &ChrAsm2, ga_items: &[GaItem]) -> Vec<EquipmentFact> {
    use EquipSlot::*;

    let mut facts = Vec::new();

    let hands = [RightHand1, RightHand2, RightHand3];
    for (&slot, &handle) in hands.iter().zip(asm.right_hand_armaments.iter()) {
        facts.extend(weapon(slot, handle, ga_items));
    }
    let hands = [LeftHand1, LeftHand2, LeftHand3];
    for (&slot, &handle) in hands.iter().zip(asm.left_hand_armaments.iter()) {
        facts.extend(weapon(slot, handle, ga_items));
    }
    for (&slot, &handle) in [Arrow1, Arrow2].iter().zip(asm.arrows.iter()) {
        facts.extend(projectile(slot, handle, ga_items));
    }
    for (&slot, &handle) in [Bolt1, Bolt2].iter().zip(asm.bolts.iter()) {
        facts.extend(projectile(slot, handle, ga_items));
    }
    facts.extend(armor(Head, asm.head, ga_items));
    facts.extend(armor(Chest, asm.chest, ga_items));
    facts.extend(armor(Arms, asm.arms, ga_items));
    facts.extend(armor(Legs, asm.legs, ga_items));
    let tali = [Talisman1, Talisman2, Talisman3, Talisman4];
    for (&slot, &handle) in tali.iter().zip(asm.talismans.iter()) {
        facts.extend(talisman(slot, handle));
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gaitem(handle: u32, item_id: u32) -> GaItem {
        GaItem {
            gaitem_handle: handle,
            item_id,
            ..GaItem::default()
        }
    }

    const HANDLE_WEAPON: u32 = 0x8000_0000;
    const HANDLE_ARMOR: u32 = 0x9000_0000;

    #[test]
    fn weapon_keeps_full_reinforced_id_and_splits_upgrade() {
        // Longsword base 2000000, +3 reinforced -> 2000003; upgrade is the low digits.
        let map = [gaitem(HANDLE_WEAPON | 1, 2000003)];
        assert_eq!(
            weapon(EquipSlot::RightHand1, HANDLE_WEAPON | 1, &map),
            Some(EquipmentFact { slot: EquipSlot::RightHand1, item_id: 2000003, upgrade: 3 })
        );
    }

    #[test]
    fn projectile_carries_no_upgrade() {
        let map = [gaitem(HANDLE_WEAPON | 2, 340000)];
        assert_eq!(
            projectile(EquipSlot::Arrow1, HANDLE_WEAPON | 2, &map),
            Some(EquipmentFact { slot: EquipSlot::Arrow1, item_id: 340000, upgrade: 0 })
        );
    }

    #[test]
    fn armor_clears_the_tag_off_the_map_id() {
        // The map id carries the armor item-type tag; the fact id is the bare protector id.
        let map = [gaitem(HANDLE_ARMOR | 3, ITEM_TYPE_ARMOR | 410000)];
        assert_eq!(
            armor(EquipSlot::Head, HANDLE_ARMOR | 3, &map),
            Some(EquipmentFact { slot: EquipSlot::Head, item_id: 410000, upgrade: 0 })
        );
    }

    #[test]
    fn talisman_decodes_straight_from_the_handle() {
        assert_eq!(
            talisman(EquipSlot::Talisman1, HANDLE_ACCESSORY | 1010),
            Some(EquipmentFact { slot: EquipSlot::Talisman1, item_id: 1010, upgrade: 0 })
        );
    }

    #[test]
    fn empty_slots_drop() {
        // Empty weapon/armor: handle 0 is not in the map -> None.
        assert_eq!(weapon(EquipSlot::RightHand1, 0, &[]), None);
        assert_eq!(armor(EquipSlot::Head, 0, &[]), None);
        // A map entry whose id is 0 is also empty (armor guard mirrors the reader).
        assert_eq!(armor(EquipSlot::Head, HANDLE_ARMOR | 4, &[gaitem(HANDLE_ARMOR | 4, 0)]), None);
        // Empty talisman: handle 0.
        assert_eq!(talisman(EquipSlot::Talisman1, 0), None);
    }

    #[test]
    fn cleared_slots_with_max_sentinel_drop() {
        // A cleared projectile slot: handle 0xFFFFFFFF, and its gaitem-map entry
        // indirects to 0xFFFFFFFF. Without the u32::MAX guard this leaks out as a
        // bogus item_id 4294967295 (the real backup-save regression).
        let map = [gaitem(u32::MAX, u32::MAX)];
        assert_eq!(projectile(EquipSlot::Arrow1, u32::MAX, &map), None);
        assert_eq!(weapon(EquipSlot::RightHand1, u32::MAX, &map), None);
        // A cleared talisman slot: handle 0xFFFFFFFF, caught before the XOR.
        assert_eq!(talisman(EquipSlot::Talisman1, u32::MAX), None);
    }

    #[test]
    // ChrAsm2 has private padding fields, so it can only be built by mutating a
    // default (the same pattern its own `Read` impl and the reader's VM use).
    #[allow(clippy::field_reassign_with_default)]
    fn resolve_equipment_emits_only_occupied_slots_in_order() {
        let map = [
            gaitem(HANDLE_WEAPON | 1, 2000003),
            gaitem(HANDLE_ARMOR | 2, ITEM_TYPE_ARMOR | 410000),
        ];
        let mut asm = ChrAsm2::default();
        asm.right_hand_armaments = [HANDLE_WEAPON | 1, 0, 0];
        asm.chest = HANDLE_ARMOR | 2;
        asm.talismans = [HANDLE_ACCESSORY | 1010, 0, 0, 0];
        let facts = resolve_equipment(&asm, &map);
        assert_eq!(
            facts,
            vec![
                EquipmentFact { slot: EquipSlot::RightHand1, item_id: 2000003, upgrade: 3 },
                EquipmentFact { slot: EquipSlot::Chest, item_id: 410000, upgrade: 0 },
                EquipmentFact { slot: EquipSlot::Talisman1, item_id: 1010, upgrade: 0 },
            ]
        );
    }
}
