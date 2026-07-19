# Deduplication: candidate result accumulation

## Rank and scope

9 of 12. Simulator result accounting, statistics, and eligibility.

## Problem

Candidate report construction manually maintains scored, missed, indeterminate, and failed counters; separately decides which outcomes feed `BernoulliReducer`; converts internal results into public dispositions; computes repeated optional statistics; and derives eligibility. The four-way classification is a single correctness policy spread across parallel code.

## Decision

Introduce a private `CandidateAccumulator` owning the four counters and Bernoulli reducer. One exhaustive `record` method converts each trial result into applied controls plus `TrialDisposition` while updating exactly the permitted statistics. A `finish` method constructs `CandidateReport` and checks the accounting invariant.

Do not conflate indeterminate physics with adapter failure. Keep public CSV rendering separate from accounting.

## Files

- `integrations/simul-three-cushion/src/adapter.rs`
- `integrations/simul-three-cushion/tests/experiment.rs`
- Existing report distinction coverage in `integrations/simul-three-cushion/tests/cli.rs`

## Invariants

- Scored is a Bernoulli success; miss is a Bernoulli failure.
- Indeterminate and failed trials do not enter Bernoulli statistics.
- Candidate eligibility requires zero indeterminate and zero failed trials.
- Requested count equals the sum of all four dispositions.
- Applied controls and failure detail are preserved.

## Verification

Run table-driven accounting coverage for all four result classes, report totals/ranking tests, CLI disposition output, and serial/parallel equivalence.

## Commit boundary

Only candidate accounting and report assembly. No seed, trial execution, ranking formula, or CSV schema changes.