# Deduplication: ball visual metadata

## Rank and scope

11 of 12. Static SVG and playback ball identity/presentation.

## Problem

Static SVG rendering and playback JSON independently match every `BallType` to stable ID/class, fill color, and optional number label. The grouped aliases are currently aligned but adding or changing a ball type can update one output mode and silently leave the other stale.

## Decision

Introduce one allocation-free crate-private ball visual descriptor containing static ID/class, fill, and optional label. Place it in the rendering owner module and expose a narrow `pub(crate)` accessor for playback serialization. Static SVG and playback JSON consume the same exhaustive match.

Keep PNG sprite selection separate because it is backend-specific. Do not merge viewer DOM rendering or introduce a generic renderer.

## Files

- `src/diagram.rs`
- `src/svg_generator.rs`
- `tests/rendering_geometry.rs`
- `tests/svg_generator.rs`

## Invariants

- Wire IDs, SVG classes, fills, and labels remain byte-for-byte unchanged.
- All `BallType` variants are handled exhaustively.
- Number labels appear for the same variants in static and playback modes.
- The descriptor returns borrowed static data and allocates nothing.
- PNG assets and DSL human prose remain outside this commit.

## Verification

Exercise all ball variants through the descriptor and compare representative static SVG and playback payload metadata. Retain existing cue, nine, eight, carom, and numbered-label viewer assertions.

## Commit boundary

One rendering metadata source of truth. Playback payload ownership and xtask mirrors are deferred to a separate future candidate.