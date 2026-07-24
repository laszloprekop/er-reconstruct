//! # er-reconstruct — the Character Reconstructor
//!
//! The shared reconstruction core (ADR-0010 in ER-save-Reader): it turns one slot
//! of an Elden Ring save into **facts** and stops there. Both the ER-save-Reader
//! (native) and elden-map (WASM) link this one library instead of maintaining two
//! reconstructions, so a fix to a faulty reconstruction reflects in both.
//!
//! The public surface is the pure entry point [`reconstruct`] and its output
//! [`ReconstructedCharacter`]. Everything below is the parsing foundation extracted
//! from the reader's `save/` module (with history), kept `pub` only as far as the
//! entry point and the facade re-export need.
//!
//! ## ADR-0008 invariant
//!
//! This core holds reconstruction *logic*, never hardcoded flag base tables:
//! flag positions are resolved per save via `wasm-event-flags`, which this crate
//! layers on top of rather than absorbing. The seed reconstructs identity only and
//! reads no flags yet; later slices resolve them through that crate.

pub mod read;
pub mod save;
pub mod util;

// Dormant save write-back path, moved with the extracted `save/` code (ADR-0009).
// Off by default; the extracted `impl Write` blocks compile under the feature.
#[cfg(feature = "save-writeback")]
pub mod write;

mod reconstruct;

// The JS boundary for elden-map's wasm-pack build; wasm32 only (see `wasm.rs`).
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use reconstruct::{reconstruct, ReconstructError, ReconstructedCharacter};
