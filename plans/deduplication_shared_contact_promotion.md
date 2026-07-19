# Deduplication: shared contact promotion

## Rank and scope

1 of 12. Core event-scheduling correctness path.

## Problem

`PocketAwareEventCache::next_event` and `shared_ball_ball_contact_from_candidates` independently implement the same simultaneous ball-ball promotion policy: select contacts within `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS`, canonicalize pair order, derive sorted unique ball indices, reject disjoint pairs, and construct a coupled-contact event. The ordinary scheduler also embeds the same tolerance as a literal in its neighboring tie comparator.

The duplication is dangerous because promotion changes the collision solver from pairwise to coupled resolution. Drift in tolerance, ordering, or connectedness changes physics.

## Decision

Introduce a private `SharedBallBallContactSummary` and one helper that accepts the earliest event time plus `(time, first, second)` contacts. The helper owns tolerance filtering, pair canonicalization, deterministic sorting, unique ball extraction, and the connected-contact requirement. Each scheduler retains its enum-specific event construction.

Use the named simultaneous-event tolerance everywhere. Preserve the current strict comparisons and sort order exactly; do not reassociate floating-point expressions.

## Files

- `src/lib.rs`
- Focused existing coverage in `tests/n_ball_events.rs`, `tests/n_ball_pockets.rs`, `tests/next_events.rs`, and `tests/numerical_regressions.rs`

## Invariants

- The earliest non-ball event still wins when it precedes the contact window.
- Contacts exactly inside/outside tolerance retain current behavior.
- Disjoint simultaneous pairs are not promoted into one component.
- Pair and ball-index ordering remains deterministic.
- Pocket-aware and ordinary schedulers apply one policy but keep distinct event enums.

## Verification

Run focused shared-contact, event-ordering, and pocket-aware scheduling tests. Compare both scheduler paths for overlapping, disjoint, and tolerance-boundary contact sets.

## Commit boundary

One production refactor and only the focused regression adjustments required to prove policy equivalence. No collision-response changes.