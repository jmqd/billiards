# Deduplication: timeline subdivision

## Rank and scope

6 of 12. Playback-frame and rendered-trace sampling.

## Problem

`ScenarioShotTrace::playback_frames` and `ScenarioBallTrace::sampled_timeline_points` independently compute `ceil(duration / max_step).max(1)`, evenly spaced local elapsed values, and an exact final endpoint. These are two user-visible projections of the same timeline segments.

Their invalid-step behavior intentionally differs: playback rejects invalid positive/finite input, while sampled rendering degrades to endpoint-only sampling. Only subdivision after normalization should be shared.

## Decision

Introduce a private allocation-free timeline subdivision iterator producing local elapsed time plus an explicit endpoint marker. Validated playback calls it with a step; sampled rendering passes no step for endpoint-only fallback. Each caller retains its own state evaluation, global-time offsetting, and time deduplication.

## Files

- `src/dsl.rs`
- `tests/dsl.rs`

## Invariants

- Non-divisible durations produce equal subdivisions with maximum gap at most the requested step.
- Segment endpoints use the exact stored end state.
- Zero-duration segments remain well defined.
- Playback boundary epsilon and event-display grouping epsilon remain separate policies.
- No per-segment `Vec` is allocated by the abstraction.

## Verification

Run focused playback frame, rendered trace sampling, pre/post-event boundary, and pocket disappearance tests. Add a non-divisible duration case such as duration `1.0`, step `0.3`, plus zero-duration coverage.

## Commit boundary

Only subdivision arithmetic and iteration. No playback state-selection or rendering policy changes.