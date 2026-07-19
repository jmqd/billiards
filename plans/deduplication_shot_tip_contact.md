# Deduplication: validated cue-tip contact

## Rank and scope

4 of 12. Typed shot-control validation and construction.

## Problem

`ShotControls::new` constructs `CueTipContact` from side and height offsets to validate the radial tip bound, then discards it. `ShotControls::to_shot` reconstructs the identical value from immutable stored scalars and repeats the same error path.

## Decision

Store the validated `CueTipContact` inside `ShotControls` and remove the redundant scalar fields. Construct it once in `ShotControls::new`; existing scalar getters read from the stored contact; `to_shot` clones the validated contact into the low-level `Shot`.

Keep heading, speed, elevation, and non-finite checks at their current boundary. Invalid tip geometry must still fail during `ShotControls::new`, not during execution.

## Files

- `src/shot_simulation.rs`
- `tests/shot_simulation.rs`

## Invariants

- Accepted and rejected control inputs are unchanged.
- Combined side/height radial validation occurs exactly once at construction.
- Getter values remain exactly the supplied offsets.
- Non-center-tip shots produce identical results.
- No new allocation or public field exposure is introduced.

## Verification

Run focused control-validation and typed-shot execution tests. Cover non-finite values, just-inside/outside radial tip bounds, getter parity, and the existing non-center scoring fixture.

## Commit boundary

Only the internal `ShotControls` representation and duplicate construction removal. No lower-level cue model or public API compatibility shim.