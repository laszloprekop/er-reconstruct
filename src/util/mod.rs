pub mod regulation;

// Read only by the dormant save write-back path (ADR-0009); gated so the default
// build does not carry it.
#[cfg(feature = "save-writeback")]
pub mod bit;
