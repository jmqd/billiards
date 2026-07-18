# Deduplication: trial schedule identity

## Rank and scope

7 of 12. Deterministic simulator trial/replay scheduling.

## Problem

Trial generation manually builds `TrialKey`, repeats it inside `ReplayKey`, derives `common_random_group` from replication ID, and stores both `TrialSpec.key` and `TrialSpec.replay.trial`. Tests reproduce the nested construction. The model can represent divergent trial and replay identities even though callers intend equality.

## Decision

Add `TrialSpec::new(candidate, replication_id)` as the sole constructor. Store the identity once inside `ReplayKey`; expose a cheap `key()` accessor for `replay.trial`; remove the redundant `key` field. Preserve candidate-major, replication-minor ordering and `common_random_group = u64::from(replication_id)`.

Do not introduce a generic scheduling framework or alter seed/replay protocol strings.

## Files

- `integrations/simul-three-cushion/src/adapter.rs`
- Focused integration coverage in `integrations/simul-three-cushion/tests/experiment.rs`

## Invariants

- `candidate_id`, `replication_id`, replay trial identity, and common-random group cannot diverge.
- Trial ordering and report ordering remain unchanged.
- Replay keys remain byte-for-byte stable.
- Serial and parallel reports remain identical.

## Verification

Exercise constructor edge IDs, deterministic replay keys, exact reproducibility, and serial/parallel semantic equality.

## Commit boundary

Only trial identity representation and construction. Preparation, noise application, and aggregation remain separate commits.