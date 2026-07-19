# Deduplication: prepared trial records

## Rank and scope

8 of 12. Deterministic execution-noise and record construction.

## Problem

`execute_spec` and `failed_record` independently construct the same `TrialContext`, apply execution noise to the same candidate controls, and copy trial/replay identity into `TrialRecord`. Evaluator setup failure must report the exact applied controls and replay identity normal evaluation would have used, but that invariant is implicit in duplicate code.

## Decision

Introduce a private copy-only `PreparedTrial { key, replay, applied }` plus one `prepare_trial(config, spec)` function. Give the value a small consuming or borrowing record constructor. Normal evaluation evaluates `prepared.applied`; forced evaluator-construction failure records the same prepared identity and controls.

Keep evaluator execution failure, evaluator construction failure, simulator indeterminacy, and misses as distinct dispositions. Do not merge serial and parallel failure policies.

## Files

- `integrations/simul-three-cushion/src/adapter.rs`
- `integrations/simul-three-cushion/tests/experiment.rs`

## Invariants

- Normal and forced-failure paths share identical key, replay, and applied controls.
- Seed domains and named execution-noise tags remain at current callsites.
- Candidate/replication common-random behavior is unchanged.
- Serial and parallel ordering and reproducibility remain exact.

## Verification

Run preparation/noise identity tests, exact reproducibility, forced-failure record checks, and serial/parallel equality.

## Commit boundary

Only preparation and record assembly. Trial scheduling lands first; aggregation remains separate.