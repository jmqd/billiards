# Deduplication: rail boundary geometry

## Rank and scope

2 of 12. Core rail prediction and contact-state correctness path.

## Problem

Four separate rail matches encode the same axis, contact coordinate, and inward gap sign in `rail_collision_gap_quadratic_coefficients`, `raw_rail_gap_at_state`, `raw_rail_gap_derivative_at_state`, and `snap_raw_state_to_rail_contact`.

A mismatch can predict against one plane, test approach using another orientation, and snap the impact state to a third boundary.

## Decision

Add a private allocation-free `RailBoundary` descriptor containing planar axis, contact coordinate, and inward sign. Give it methods for gap, gap derivative, quadratic gap coefficients, and contact snapping. Construct it once from `Rail`, ball radius, and `TableSpec`.

Keep each current arithmetic form branch-equivalent. In particular, retain expressions such as `plane - y` rather than rewriting them as negated alternatives that can change signed zero or low bits.

## Files

- `src/lib.rs`
- Focused coverage in `tests/rail_event_scheduling.rs`, `tests/rail_event_execution.rs`, `tests/bank_paths.rs`, and `tests/numerical_regressions.rs`

## Invariants

- All four rails retain current axis and inward orientation.
- Exact-contact, approaching, and separating predicates are unchanged.
- Predicted and snapped coordinates use the same boundary.
- Custom table dimensions and ball radii remain supported.
- No generic vector/dot-product rewrite changes floating-point order.

## Verification

Exercise all four rails, exact and epsilon-offset contacts, toward/away velocities, curved rolling paths, and custom tables. Retain exact boundary assertions after event resolution.

## Commit boundary

Only rail-boundary geometry extraction. No root-window, cushion-response, or calibration changes.