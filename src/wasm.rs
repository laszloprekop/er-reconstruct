//! WASM entry point (ADR-0010). Compiled only for `wasm32` by `wasm-pack`, so the
//! native reader never pulls in `wasm-bindgen`.
//!
//! The same pure [`reconstruct`](crate::reconstruct) runs here as native — this is
//! only the JS boundary. Facts cross it as a JSON string (`ReconstructedCharacter`
//! is `Serialize`), which keeps the surface trivial and the `native == WASM` parity
//! a byte-for-byte comparison of that string. Errors cross as a thrown JS string.

use wasm_bindgen::prelude::*;

use crate::reconstruct::reconstruct;

/// Reconstruct one slot's identity facts from raw save bytes, as a JSON string
/// `{"name":…,"level":…,"class_id":…}`. Throws (JS) on an unreconstructable slot.
///
/// The JS/TS caller parses the string; keeping the boundary a string (rather than a
/// `wasm-bindgen` struct) means the WASM output is directly comparable to the native
/// `serde_json` rendering for the parity gate.
#[wasm_bindgen]
pub fn reconstruct_json(bytes: &[u8], slot: usize) -> Result<String, JsError> {
    let character = reconstruct(bytes, slot).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&character).map_err(|e| JsError::new(&e.to_string()))
}
