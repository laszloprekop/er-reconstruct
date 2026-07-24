//! Dynamic EventFlags offset detection
//!
//! SINGLE SOURCE OF TRUTH: This module delegates to wasm-event-flags crate.
//! The same detection algorithm is used by both ER-save-Reader (native) and
//! elden-map (via WASM).
//!
//! 2026-07-05: Primary is the gaEnd-windowed grace-validation scan
//! ([gaEnd+30k, gaEnd+45k]); the former "structural computation" was
//! disproven (~146k overshoot onto a lookalike region) and is no longer used
//! for detection. Fallback: legacy full-range content search. See
//! crates/wasm-event-flags/tests/anchor_conformance.rs for the fixtures that
//! define the convention, and note the per-family float caveat there.

// Re-export from the shared crate. `POSITIVE_VALIDATION_FLAGS` was re-exported
// here only for `verify_event_flags_offset`, deleted 2026-07-22: it scored a
// detected offset by testing tutorial graces at hardcoded byte positions, and
// those graces are clear on minimal characters, so a correct offset could score
// zero. Read the flags from the crate directly if you need them.
pub use wasm_event_flags::SEARCH_START;

/// Result of EventFlags offset detection
#[derive(Debug, Clone)]
pub struct EventFlagsDetectionResult {
    /// Detected offset from start of slot data
    pub offset: usize,
    /// Number of validation flags matched (0-4)
    pub validation_score: usize,
    /// Whether detection is confident (all validation flags matched)
    pub confident: bool,
    /// Size of the gap before EventFlags (from expected position).
    ///
    /// Unread: a diagnostic recorded alongside the detection, not an input to
    /// it. Kept because it is the only record of how far a detection landed
    /// from expectation, which is what distinguishes a clean hit from a lucky one.
    #[allow(dead_code)]
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

    // Trust the detection if confident (structural detection) or if grace flags validate
    if result.confident || result.validation_score >= 2 {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The constants this module re-exports must stay identical to the shared
    /// crate's. Restored 2026-07-22: it was removed as collateral when the
    /// tutorial-grace validator went, but it guards a different thing — drift
    /// between this module's view of the format and `wasm-event-flags`'.
    #[test]
    fn test_constants_match_shared() {
        assert_eq!(wasm_event_flags::EVENT_FLAGS_SIZE, 0x1bf99f);
        assert_eq!(SEARCH_START, 0x12000);
    }
}
