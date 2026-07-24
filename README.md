# er-reconstruct — the Character Reconstructor

The shared reconstruction core for Elden Ring saves (ADR-0010 in
[ER-save-Reader](https://github.com/laszloprekop/ER-save-Reader)). It turns one
slot of a save into **facts** and stops there — no I/O, no rendering, no naming.
Both ER-save-Reader (native) and elden-map (WASM) link this one library instead of
maintaining two reconstructions, so a fix to a faulty reconstruction reflects in
both.

```rust
let bytes = std::fs::read("ER0000.sl2")?;
let character = er_reconstruct::reconstruct(&bytes, 0)?;
// ReconstructedCharacter { name, level, class_id } — facts only, ID-keyed.
```

## Status

**Seed (walking skeleton).** `ReconstructedCharacter` carries character identity
only — `name`, `level`, `class_id` (raw archetype id, no display string). The
contract is **append-only**: each later slice (stats, bosses/graces, pickups,
inventory, equipment, world position) adds fields, never reshapes these. See the
fact inventory in ER-save-Reader's `docs/RECONSTRUCTION-FACT-INVENTORY.md`.

The `save/` parsing was extracted from ER-save-Reader with its git history.

## Invariants

- **No hardcoded flag base tables** (ADR-0008). Flag positions are resolved per
  save via `wasm-event-flags`, which this crate layers on top of. The seed reads
  no flags yet.
- **Facts only** (ADR-0010). Names, map coordinates, and UI strings are each app's
  Enrichment, never baked into the output.
- **Read-only.** The dormant write-back path moved along with `save/` and sits
  behind the off-by-default `save-writeback` feature.

## Build

```
cargo test                        # native, identity facts + unit tests
cargo test --features save-writeback   # keeps the dormant write path compiling
```

The conformance harness (`tests/conformance_corpus.rs`) checks real saves against
known-truth facts; it skips when the out-of-repo saves are absent. Point it at them
with `ER_RECONSTRUCT_CORPUS_DIR`.

`wasm-event-flags` lives in ER-save-Reader; this crate references it by git and a
consumer unifies it with a `[patch]` on the same URL (see `Cargo.toml`).

## Maintainers & Attribution

Maintained by **Laszlo Prekop**.

The save-parsing code was extracted (with history) from
[ER-Save-Editor](https://github.com/ClayAmore/ER-Save-Editor) by **ClayAmore**;
further save-parsing contributions by **groobybugs**. Original authorship is
preserved in the git history and the copyright notices.

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option.
