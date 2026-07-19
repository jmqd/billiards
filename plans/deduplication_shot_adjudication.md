# Deduplication: three-cushion shot adjudication

## Rank and scope

3 of 12. Main typed three-cushion execution path.

## Problem

`execute_core` already observes every resolved event with `ThreeCushionAccumulator` and returns the final adjudication in `CoreShotResult`. `execute_three_cushion` currently routes through `execute_shot`, discards that adjudication, retains the event ledger, and calls `project_three_cushion` to replay the same ledger through a second accumulator. The compact wrapper already consumes the core adjudication directly.

## Decision

Make `execute_three_cushion` call `execute_core(..., retain_events = true)` and construct `ThreeCushionResult` from the core adjudication and retained fields. Keep public `project_three_cushion` for independently produced or persisted ledgers.

Use private consuming conversions on `CoreShotResult` only if they serve both full and compact projections and reduce field plumbing. Do not create a public generic result hierarchy.

## Files

- `src/shot_simulation.rs`
- `tests/shot_simulation.rs`

## Invariants

- Full and compact executions produce identical completion and final states.
- Event order and retained evidence are unchanged.
- `project_three_cushion` remains available and equivalent when explicitly applied.
- Settled, event-limit, and unsupported-contact termination feed the same adjudication.
- The refactor removes the second rule pass without changing simulation execution.

## Verification

Run focused full/compact parity, scoring, event-limit, and unsupported-contact tests. Assert standalone projection of the retained full ledger equals the completion produced during execution.

## Commit boundary

One execution-wrapper simplification. No rule changes, public result redesign, or event mapping changes.