//! Held-inventory facts and the **GaItem-decode foundation** (ADR-0010, slice #6).
//!
//! The first **non-flag** facts. Bosses and pickups (slices #4/#5) are tri-state
//! bits resolved per save through `wasm-event-flags`; inventory is instead
//! *decoded* from two per-slot structures the save carries directly:
//!
//! - [`EquipInventoryData`] — the held lists (`common_items` + `key_items`), each
//!   entry an [`EquipInventoryItem`] `{ ga_item_handle, quantity, inventory_index }`.
//! - `ga_items: &[GaItem]` — the **gaitem map**. A weapon/armor/ash-of-war handle is
//!   an indirection: it names a [`GaItem`] whose `item_id` holds the real param id
//!   (with weapon reinforcement in the low two digits). Accessories and consumable
//!   items need no indirection — their id is XOR-decoded from the handle itself.
//!
//! The decode mirrors the reader's `InventoryItemViewModel::from_save`
//! (`src/vm/inventory/mod.rs`); the names it resolves each id to are Enrichment and
//! stay in each app.
//!
//! # A fact is item identity, never a handle
//!
//! GaItem handles churn as items are gained and dropped (`CONTEXT.md` → the handle
//! "is not universal, and treating it as one is a bug"; inventory deltas must be
//! computed by item identity). So a fact keeps `category + item_id + quantity` and
//! drops both the handle and the per-save `inventory_index`.

use crate::save::common::save_slot::{EquipInventoryData, EquipInventoryItem, GaItem};

/// The high nibble of a `ga_item_handle` tags its category. These are the
/// `InventoryGaitemType` discriminants from the reader (`src/vm/inventory/mod.rs`).
const CATEGORY_MASK: u32 = 0xf000_0000;
const HANDLE_WEAPON: u32 = 0x8000_0000;
const HANDLE_ARMOR: u32 = 0x9000_0000;
const HANDLE_ACCESSORY: u32 = 0xa000_0000;
const HANDLE_ITEM: u32 = 0xb000_0000;
const HANDLE_AOW: u32 = 0xc000_0000;

/// XOR keys that recover a bare param id, mirroring the reader exactly
/// (`src/vm/inventory/mod.rs`). The two indirect categories clear the tag off the
/// *gaitem-map* `item_id` with an `InventoryItemType` key (armor `0x10000000`, ash
/// `0x80000000`); the two direct categories clear it off the *handle* itself with the
/// `InventoryGaitemType` tag (accessory `0xa0000000`, item `0xb0000000` = the
/// `HANDLE_*` constants above). Weapons take the map `item_id` verbatim (reinforced).
const ITEM_TYPE_ARMOR: u32 = 0x1000_0000;
const ITEM_TYPE_AOW: u32 = 0x8000_0000;

/// Which kind of item a held slot holds. A structural fact of the save (the handle's
/// high nibble), not a display label — the id → name lookup that needs it lives in
/// each app's Enrichment stage. Serialized by name so the `native == WASM` parity
/// boundary carries a stable, self-describing token (like [`super::FlagStatus`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ItemCategory {
    Weapon,
    Armor,
    Accessory,
    Item,
    Aow,
}

/// One held-inventory fact: an item's identity (category + param id) and how many are
/// held. No handle, no `inventory_index` — those churn per save and are not identity
/// (`CONTEXT.md`). For a weapon the `item_id` is the full reinforced value (base id +
/// upgrade in its low two digits), exactly as the gaitem map stores it; splitting off
/// the upgrade level for display is Enrichment.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct InventoryFact {
    pub category: ItemCategory,
    pub item_id: u32,
    pub quantity: u32,
}

/// Look a weapon/armor/ash-of-war handle up in the gaitem map. The reader does the
/// same linear find and `.unwrap()`s it — the save's own invariant is that every held
/// indirection has a map entry. Here we return `None` on a miss and let the caller
/// drop the slot rather than panic (ADR-0008/0010: refuse, don't guess).
fn map_item_id(handle: u32, ga_items: &[GaItem]) -> Option<u32> {
    ga_items
        .iter()
        .find(|g| g.gaitem_handle == handle)
        .map(|g| g.item_id)
}

/// Decode one `common_items` slot into a fact, or `None` when it carries nothing
/// (Empty padding) or names a map entry that isn't there. Pure: `(slot, gaitem map) →
/// fact`.
fn decode_common_item(item: &EquipInventoryItem, ga_items: &[GaItem]) -> Option<InventoryFact> {
    let handle = item.ga_item_handle;
    let (category, item_id) = match handle & CATEGORY_MASK {
        HANDLE_WEAPON => (ItemCategory::Weapon, map_item_id(handle, ga_items)?),
        HANDLE_ARMOR => (
            ItemCategory::Armor,
            map_item_id(handle, ga_items)? ^ ITEM_TYPE_ARMOR,
        ),
        HANDLE_AOW => (
            ItemCategory::Aow,
            map_item_id(handle, ga_items)? ^ ITEM_TYPE_AOW,
        ),
        HANDLE_ACCESSORY => (ItemCategory::Accessory, handle ^ HANDLE_ACCESSORY),
        HANDLE_ITEM => (ItemCategory::Item, handle ^ HANDLE_ITEM),
        // Empty padding (the common-items list is fixed-length; most slots are 0),
        // or an unrecognised tag. Not an item.
        _ => return None,
    };
    Some(InventoryFact {
        category,
        item_id,
        quantity: item.quantity,
    })
}

/// Held **common** inventory: every non-empty `common_items` slot decoded to a fact,
/// in save (index) order. Weapons/armor/ashes resolve through `ga_items`; accessories
/// and consumables decode straight from the handle. Order is preserved so `native ==
/// WASM` compares element-for-element.
pub fn resolve_held_common(inv: &EquipInventoryData, ga_items: &[GaItem]) -> Vec<InventoryFact> {
    inv.common_items
        .iter()
        .filter_map(|item| decode_common_item(item, ga_items))
        .collect()
}

/// Held **key items**: the save keeps them in a separate list, and the reader decodes
/// every entry as a consumable [`ItemCategory::Item`] (id = `handle ^ ITEM_TYPE_ITEM`),
/// keeping those actually held (`quantity > 0`) — empty key slots read quantity 0.
pub fn resolve_held_key_items(inv: &EquipInventoryData) -> Vec<InventoryFact> {
    inv.key_items
        .iter()
        .filter(|item| item.quantity > 0)
        .map(|item| InventoryFact {
            category: ItemCategory::Item,
            item_id: item.ga_item_handle ^ HANDLE_ITEM,
            quantity: item.quantity,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(handle: u32, quantity: u32) -> EquipInventoryItem {
        EquipInventoryItem {
            ga_item_handle: handle,
            quantity,
            inventory_index: 0,
        }
    }

    fn gaitem(handle: u32, item_id: u32) -> GaItem {
        GaItem {
            gaitem_handle: handle,
            item_id,
            ..GaItem::default()
        }
    }

    #[test]
    fn accessory_and_item_decode_from_the_handle() {
        // No gaitem-map indirection: id is the handle XOR the category key.
        // XORing the handle by its own category tag clears the tag: the id is the
        // low 28 bits. Accessory 0xa0000000|1234 -> 1234; Item 0xb0000000|999 -> 999.
        assert_eq!(
            decode_common_item(&item(HANDLE_ACCESSORY | 1234, 1), &[]),
            Some(InventoryFact { category: ItemCategory::Accessory, item_id: 1234, quantity: 1 })
        );
        assert_eq!(
            decode_common_item(&item(HANDLE_ITEM | 999, 7), &[]),
            Some(InventoryFact { category: ItemCategory::Item, item_id: 999, quantity: 7 })
        );
    }

    #[test]
    fn weapon_armor_aow_decode_through_the_gaitem_map() {
        let map = [
            gaitem(HANDLE_WEAPON | 1, 3000010),        // reinforced weapon: base 3000000 +10
            gaitem(HANDLE_ARMOR | 2, ITEM_TYPE_ARMOR | 20000), // armor id 20000, tagged
            gaitem(HANDLE_AOW | 3, ITEM_TYPE_AOW | 700), // ash-of-war id 700, tagged
        ];
        assert_eq!(
            decode_common_item(&item(HANDLE_WEAPON | 1, 1), &map),
            Some(InventoryFact { category: ItemCategory::Weapon, item_id: 3000010, quantity: 1 })
        );
        assert_eq!(
            decode_common_item(&item(HANDLE_ARMOR | 2, 1), &map),
            Some(InventoryFact { category: ItemCategory::Armor, item_id: 20000, quantity: 1 })
        );
        assert_eq!(
            decode_common_item(&item(HANDLE_AOW | 3, 1), &map),
            Some(InventoryFact { category: ItemCategory::Aow, item_id: 700, quantity: 1 })
        );
    }

    #[test]
    fn empty_padding_and_missing_map_entry_decode_to_none() {
        // A zero (empty) slot: no category tag.
        assert_eq!(decode_common_item(&item(0, 0), &[]), None);
        // A weapon handle with no matching gaitem-map entry: refuse, don't guess.
        assert_eq!(decode_common_item(&item(HANDLE_WEAPON | 42, 1), &[]), None);
    }

    #[test]
    fn resolve_held_common_skips_empties_and_keeps_order() {
        let map = [gaitem(HANDLE_WEAPON | 5, 1000000)];
        let inv = EquipInventoryData {
            common_items: vec![
                item(HANDLE_ITEM | 10, 3),  // consumable -> id 10
                item(0, 0),                 // empty padding, dropped
                item(HANDLE_WEAPON | 5, 1), // weapon via map
            ],
            ..EquipInventoryData::default()
        };
        let facts = resolve_held_common(&inv, &map);
        assert_eq!(
            facts,
            vec![
                InventoryFact { category: ItemCategory::Item, item_id: 10, quantity: 3 },
                InventoryFact { category: ItemCategory::Weapon, item_id: 1000000, quantity: 1 },
            ]
        );
    }

    #[test]
    fn resolve_held_key_items_drops_zero_quantity() {
        let inv = EquipInventoryData {
            key_items: vec![
                item(HANDLE_ITEM | 8100, 1),
                item(0, 0), // empty key slot, quantity 0 → dropped
            ],
            ..EquipInventoryData::default()
        };
        let facts = resolve_held_key_items(&inv);
        assert_eq!(
            facts,
            vec![InventoryFact {
                category: ItemCategory::Item,
                item_id: 8100,
                quantity: 1,
            }]
        );
    }
}
