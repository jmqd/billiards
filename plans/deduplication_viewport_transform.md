# Deduplication: viewport transform

## Rank and scope

10 of 12. Raster overlay placement and rendering consistency.

## Problem

`DiagramViewport::position_to_scene_point` and `assets::diamond_to_pixel` encode the same table-to-pixel transform. Raster balls and spin glyphs use the viewport, while dashed lines, smooth polylines, markers, labels, and ghost balls call the legacy hard-coded transform inside `drawing.rs`. A non-default viewport can therefore separate overlays from balls and from SVG output.

## Decision

Make `DiagramViewport` the sole table-to-scene transform. Convert table-space positions at the `diagram.rs` dispatch boundary. Change raster drawing primitives to accept scene/pixel coordinates. Process polyline and chevron points without allocating a transformed `Vec`. Delete `assets::diamond_to_pixel` once all production callers and its tests migrate.

Keep raster clipping, sprite selection, rounding, text layout, and alpha blending backend-specific.

## Files

- `src/diagram.rs`
- `src/drawing.rs`
- `src/assets.rs`
- `tests/rendering_geometry.rs`

## Invariants

- Default viewport output remains pixel-equivalent.
- Every raster overlay honors `DiagramScene.viewport`.
- SVG and raster scene coordinates remain aligned within documented raster rounding.
- Ghost size, text placement, clipping, and sprite behavior are unchanged.
- The hot path does not allocate transformed point collections.

## Verification

Run focused raster geometry and drawing tests. Cover default anchor parity and a non-default viewport containing a ball, line, marker, ghost, and label whose centers must coincide.

## Commit boundary

Only coordinate ownership and raster primitive inputs. No generic renderer, backend merger, or style redesign.