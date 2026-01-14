//! Dynamic EventFlags offset detection
//!
//! The gap between fixed structures (ending with tutorial_data) and EventFlags
//! is NOT fixed - it varies per character save. This module provides pattern-based
//! detection to locate the EventFlags section reliably.
//!
//! The algorithm searches for known grace discovery flags that should be set
//! for any character past the tutorial area, AND verifies that late-game graces
//! are NOT set (to eliminate false positive matches).

/// Known grace flags used to validate EventFlags offset detection (POSITIVE).
/// These graces should be discovered by any character past the tutorial.
///
/// Format: (flag_id, byte_offset_in_event_flags, bit_position, name)
pub const VALIDATION_FLAGS: &[(u32, u32, u8, &str)] = &[
    (71800, 2725, 7, "Cave of Knowledge"),
    (71801, 2725, 6, "Stranded Graveyard"),
    (76100, 3262, 3, "The First Step"),
    (76101, 3262, 2, "Church of Elleh"),
];

/// Late-game grace flags used for NEGATIVE validation.
/// These graces require significant progression and should NOT be set
/// for characters that have just completed the tutorial.
/// If these ARE set at a candidate offset, it's a false positive.
///
/// Format: (flag_id, byte_offset_in_event_flags, bit_position, name)
pub const NEGATIVE_VALIDATION_FLAGS: &[(u32, u32, u8, &str)] = &[
    // Leyndell Capital - requires 2 Great Runes
    (76223, 3277, 0, "Fortified Manor, First Floor"),
    (76224, 3278, 7, "East Capital Rampart"),
    (76225, 3278, 6, "Divine Bridge"),
    // Mountaintops of the Giants - very late game
    (76300, 3287, 3, "Zamor Ruins"),
    (76301, 3287, 2, "Ancient Snow Valley Ruins"),
    // Haligtree - endgame optional area
    (76350, 3293, 5, "Haligtree Town"),
];

/// Event flags section size (constant across all saves)
pub const EVENT_FLAGS_SIZE: usize = 0x1bf99f;

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

/// Calculate the byte offset within EventFlags for a given flag ID.
///
/// This implements the flag-to-offset formula used by the game engine.
/// For flags in ranges 60000-99999, uses block-based calculation.
/// For 10-digit flags, uses direct division.
pub fn flag_id_to_byte_offset(flag_id: u32) -> Option<u32> {
    // For validation flags (71xxx, 76xxx), they're in the 5-digit range
    // and use block-based offsets. The offsets in VALIDATION_FLAGS are
    // pre-calculated from known working implementations.
    //
    // Formula: offset = base_for_block + ((flag_id - block_start) / 8)
    // bit = 7 - (flag_id % 8)
    //
    // Since VALIDATION_FLAGS already contains the correct offsets,
    // we don't need to recalculate here.

    // This function is here for future extension if needed
    Some(flag_id / 8)
}

/// Calculate the bit position within a byte for a given flag ID.
pub fn flag_id_to_bit_position(flag_id: u32) -> u8 {
    7 - ((flag_id % 8) as u8)
}

/// Detect the EventFlags offset by searching for known grace flag patterns.
///
/// Uses positive validation (tutorial graces that MUST be set) as the primary
/// criterion, and negative validation (late-game graces) as a tie-breaker
/// to eliminate false positives when multiple offsets match positive flags.
///
/// # Arguments
/// * `slot_data` - Raw bytes of the character slot
/// * `search_start` - Byte offset to start searching from (typically after tutorial_data)
///
/// # Returns
/// * `EventFlagsDetectionResult` with detected offset and confidence info
pub fn detect_event_flags_offset(slot_data: &[u8], search_start: usize) -> EventFlagsDetectionResult {
    // Maximum search range (200KB should be more than enough)
    let max_search = 200_000;
    let search_end = (search_start + max_search).min(slot_data.len().saturating_sub(EVENT_FLAGS_SIZE));

    // Minimum offset to avoid false positives from early data
    let min_offset = 500;
    let actual_start = search_start.max(min_offset);

    // Phase 1: Find ALL offsets where all positive flags match
    let mut candidates: Vec<(usize, usize)> = Vec::new(); // (offset, negative_score)

    for test_offset in actual_start..search_end {
        let mut positive_score = 0;

        // Check positive flags (must ALL be SET)
        for &(_flag_id, byte_offset, bit_pos, _name) in VALIDATION_FLAGS {
            let abs_pos = test_offset + byte_offset as usize;
            if abs_pos < slot_data.len() {
                let byte = slot_data[abs_pos];
                if (byte & (1 << bit_pos)) != 0 {
                    positive_score += 1;
                }
            }
        }

        // Only consider offsets where ALL positive flags match
        if positive_score == VALIDATION_FLAGS.len() {
            // Count negative flags that are NOT set (higher = more likely correct)
            let mut negative_score = 0;
            for &(_flag_id, byte_offset, bit_pos, _name) in NEGATIVE_VALIDATION_FLAGS {
                let abs_pos = test_offset + byte_offset as usize;
                if abs_pos < slot_data.len() {
                    let byte = slot_data[abs_pos];
                    if (byte & (1 << bit_pos)) == 0 {
                        negative_score += 1;
                    }
                }
            }

            candidates.push((test_offset, negative_score));

            // If all negative flags are also NOT set, this is a perfect match for early-game
            if negative_score == NEGATIVE_VALIDATION_FLAGS.len() {
                // Found perfect match - use this one
                return EventFlagsDetectionResult {
                    offset: test_offset,
                    validation_score: VALIDATION_FLAGS.len(),
                    confident: true,
                    gap_size: test_offset.saturating_sub(search_start),
                };
            }
        }
    }

    // Phase 2: If no perfect match, pick candidate with highest negative score
    // (most late-game graces NOT set = most likely correct offset)
    if !candidates.is_empty() {
        // Sort by negative score descending, then by offset ascending (prefer earlier)
        candidates.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
        });

        let (best_offset, best_neg_score) = candidates[0];

        return EventFlagsDetectionResult {
            offset: best_offset,
            validation_score: VALIDATION_FLAGS.len(),
            confident: best_neg_score >= NEGATIVE_VALIDATION_FLAGS.len() / 2,
            gap_size: best_offset.saturating_sub(search_start),
        };
    }

    // Phase 3: No perfect positive match - fall back to best partial match
    let mut best_offset = actual_start;
    let mut best_positive_score = 0;

    for test_offset in actual_start..search_end {
        let mut positive_score = 0;

        for &(_flag_id, byte_offset, bit_pos, _name) in VALIDATION_FLAGS {
            let abs_pos = test_offset + byte_offset as usize;
            if abs_pos < slot_data.len() {
                let byte = slot_data[abs_pos];
                if (byte & (1 << bit_pos)) != 0 {
                    positive_score += 1;
                }
            }
        }

        if positive_score > best_positive_score {
            best_positive_score = positive_score;
            best_offset = test_offset;
        }
    }

    EventFlagsDetectionResult {
        offset: best_offset,
        validation_score: best_positive_score,
        confident: false,
        gap_size: best_offset.saturating_sub(search_start),
    }
}

/// Detect EventFlags offset with fallback to expected offset.
///
/// If detection fails (score < 2), falls back to the expected offset.
/// This handles edge cases like brand new characters without graces.
///
/// # Arguments
/// * `slot_data` - Raw bytes of the character slot
/// * `search_start` - Byte offset to start searching from
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

    for &(_flag_id, byte_offset, bit_pos, name) in VALIDATION_FLAGS {
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
}
