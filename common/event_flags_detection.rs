//! Dynamic EventFlags offset detection
//!
//! SINGLE SOURCE OF TRUTH: This module delegates to wasm-event-flags crate.
//! The same detection algorithm is used by both ER-save-Editor (native) and
//! elden-map (via WASM).
//!
//! The algorithm searches for known grace discovery flags that should be set
//! for any character past the tutorial area, AND verifies that late-game graces
//! are NOT set (to eliminate false positive matches).

// Re-export constants from the shared crate
pub use wasm_event_flags::{
    POSITIVE_VALIDATION_FLAGS,
    SEARCH_START,
};

/// Legacy constant name for backward compatibility
pub const VALIDATION_FLAGS: &[(u32, u32, u8, &str)] = &[
    (71800, 2725, 7, "Cave of Knowledge"),
    (71801, 2725, 6, "Stranded Graveyard"),
    (76100, 3262, 3, "The First Step"),
    (76101, 3262, 2, "Church of Elleh"),
];

/// Result of EventFlags offset detection
#[derive(Debug, Clone)]
pub struct EventFlagsDetectionResult {
    /// Detected offset from start of slot data
    pub offset: usize,
    /// Number of validation flags matched (0-4)
    pub validation_score: usize,
    /// Whether detection is confident (all validation flags matched)
    pub confident: bool,
    /// Size of the gap before EventFlags (from expected position)
    pub gap_size: usize,
}

/// Detect the EventFlags offset by searching for known grace flag patterns.
///
/// DELEGATES TO: wasm-event-flags crate (single source of truth)
///
/// Uses positive validation (tutorial graces that MUST be set) as the primary
/// criterion, and negative validation (late-game graces) as a tie-breaker
/// to eliminate false positives when multiple offsets match positive flags.
///
/// # Arguments
/// * `slot_data` - Raw bytes of the character slot
/// * `_search_start` - Ignored (uses SEARCH_START from shared crate)
///
/// # Returns
/// * `EventFlagsDetectionResult` with detected offset and confidence info
pub fn detect_event_flags_offset(slot_data: &[u8], _search_start: usize) -> EventFlagsDetectionResult {
    // Use the shared implementation from wasm-event-flags crate
    let result = wasm_event_flags::detect_event_flags_offset_impl(slot_data);

    EventFlagsDetectionResult {
        offset: result.offset,
        validation_score: result.positive_score,
        confident: result.confident,
        gap_size: result.offset.saturating_sub(SEARCH_START),
    }
}

/// Detect EventFlags offset with fallback to expected offset.
///
/// DELEGATES TO: wasm-event-flags crate (single source of truth)
///
/// If detection fails (score < 2), falls back to the fallback offset.
/// This handles edge cases like brand new characters without graces.
///
/// # Arguments
/// * `slot_data` - Raw bytes of the character slot
/// * `search_start` - Byte offset to start searching from (ignored, uses shared constant)
/// * `fallback_offset` - Expected offset to use if detection fails
pub fn detect_event_flags_offset_with_fallback(
    slot_data: &[u8],
    search_start: usize,
    fallback_offset: usize,
) -> EventFlagsDetectionResult {
    let result = detect_event_flags_offset(slot_data, search_start);

    // If we matched at least 2 flags, trust the detection
    if result.validation_score >= 2 {
        result
    } else {
        // Fall back to expected offset
        EventFlagsDetectionResult {
            offset: fallback_offset,
            validation_score: 0,
            confident: false,
            gap_size: fallback_offset.saturating_sub(search_start),
        }
    }
}

/// Verify that EventFlags at a given offset contain expected patterns.
///
/// This can be used to validate an offset after reading.
pub fn verify_event_flags_offset(slot_data: &[u8], offset: usize) -> (usize, Vec<&'static str>) {
    let mut score = 0;
    let mut matched_graces = Vec::new();

    for &(_flag_id, byte_offset, bit_pos, name, _tier) in POSITIVE_VALIDATION_FLAGS {
        let abs_pos = offset + byte_offset as usize;

        if abs_pos < slot_data.len() {
            let byte = slot_data[abs_pos];
            if (byte & (1 << bit_pos)) != 0 {
                score += 1;
                matched_graces.push(name);
            }
        }
    }

    (score, matched_graces)
}

/// Calculate the bit position within a byte for a given flag ID.
pub fn flag_id_to_bit_position(flag_id: u32) -> u8 {
    7 - ((flag_id % 8) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_position_calculation() {
        // Flag 76100: bit position should be 3
        // 76100 % 8 = 4, 7 - 4 = 3
        assert_eq!(flag_id_to_bit_position(76100), 3);

        // Flag 76101: bit position should be 2
        // 76101 % 8 = 5, 7 - 5 = 2
        assert_eq!(flag_id_to_bit_position(76101), 2);

        // Flag 71800: bit position should be 7
        // 71800 % 8 = 0, 7 - 0 = 7
        assert_eq!(flag_id_to_bit_position(71800), 7);

        // Flag 71801: bit position should be 6
        // 71801 % 8 = 1, 7 - 1 = 6
        assert_eq!(flag_id_to_bit_position(71801), 6);
    }

    #[test]
    fn test_constants_match_shared() {
        use crate::db::pickup_flags::EVENT_FLAGS_SIZE;
        assert_eq!(EVENT_FLAGS_SIZE, 0x1bf99f);
        assert_eq!(SEARCH_START, 0x12000);
    }
}
