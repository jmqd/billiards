# Fixed stack storage for analytic pocket-capture candidates

## Status

- **Status:** Accepted micro-optimization candidate; benchmark- and allocation-gated.
- **Priority:** P2. The allocation is in the all-pocket capture loop, but it is smaller than the scan and geometry-preparation opportunities.
- **Confidence:** High that the heap allocation can be removed and behavior preserved; medium-low that the committed analytic-hit fixture will show a wall-time effect large enough to justify the production change.
- **Dependencies/order:** None. This plan can land alone. Prefer landing it before adaptive capture search or prepared geometry only if its baseline is collected first; otherwise benchmark it against the exact accepted predecessor tree so the tiny-container effect remains attributable.

## Problem and evidence

### Measured facts

- Assembly inspection recorded in `plans/performance_engineering.md` found a small allocation/sort/deallocation path inside `compute_next_ball_pocket_capture_on_table`.
- The macOS sample profile attributes most three-ball/event-limit time to pocket capture and its scan/gap work. That profile establishes that this code is hot, but does **not** establish that the three-element allocation is a large fraction of the total.
- The committed `pocket_predictors` group now includes `capture/fast_side_analytic_hit`, with a current quick mean around 10.12 ms/call, plus slow capture and target-miss controls around 9.45 ms and 11.61 ms. Setup is outside `b.iter`, the complete option is black-boxed, and current preflight asserts only `is_some()`/`is_none()`. The word `analytic` in the benchmark name and a final `Some` do not prove that an analytic candidate passes before fallback scanning.

### Source facts

For every pocket considered by `compute_next_ball_pocket_capture_on_table`:

1. Exactly three `Option<f64>` values are produced: `radial_entry_seconds`, `mouth_entry_seconds`, and `back_entry_seconds`.
2. They are placed into a three-element array, flattened, filtered by `time > f64::EPSILON`, and collected into a heap `Vec<f64>`.
3. The vector is sorted by `partial_cmp(...).expect("finite pocket-capture candidate times should sort")`.
4. The sorted values are consumed once; no candidate escapes the current pocket iteration.
5. If no analytic candidate validates, `scan_ball_pocket_capture_time_during_current_phase_raw` runs.
6. `PredictedBallPocketCapture` derives `PartialEq` and contains pocket identity, capture time, and the complete state at capture, so full exact output equality is testable.

The maximum live candidate count is therefore statically three. Dynamic capacity, a general-purpose vector sort, and a heap lifetime add no semantic capability.

### Hypothesis to test

A 24-byte `[f64; 3]` plus a stack-local length will remove one allocation/deallocation for each pocket iteration that reaches analytic candidate collection. The committed fast-side fixture should show the clearest effect once branch membership is independently proved. Because root helpers and other capture machinery may still allocate, this plan does not predict zero total predictor allocations.

## Scope

1. Replace only the analytic-entry `collect::<Vec<_>>()` and general sort in `compute_next_ball_pocket_capture_on_table` with deterministic fixed stack storage.
2. Preserve candidate source order, filter semantics, stable ascending order, and early break on the first validating candidate.
3. Use the committed direct fast-side capture benchmark, strengthen its untimed exact/path characterization, and black-box the full output.
4. Add exact capture-result equivalence fixtures and allocation/assembly verification.

## Explicit non-goals

- No change to radial, mouth-plane, or back-plane root solvers.
- No change to pocket gap evaluation, bisection/refinement, scan step count, fallback scan algorithm, tolerance, target geometry, or pocket ordering.
- No incremental event-cache invalidation or scheduler change.
- No general fixed-root container refactor for quadratic/cubic/rail roots.
- No `SmallVec`, new dependency, unsafe code, `MaybeUninit`, sorting network, or reusable abstraction for unrelated call sites.
- No approximate output comparison, physics/model change, or determinism change.

## Implementation design

### 1. Use a local three-slot buffer

At the analytic-entry collection in `compute_next_ball_pocket_capture_on_table`, replace the `Vec` collection/sort with local storage:

```rust
let mut analytic_entries = [0.0; 3];
let mut analytic_entry_count = 0usize;
```

Visit candidates in their current semantic order:

1. radial entry;
2. mouth-plane entry;
3. back-plane entry.

For each `Some(time)` where `time > f64::EPSILON`, insert it into the initialized prefix `analytic_entries[..analytic_entry_count]` in ascending order. Use strict `<` while shifting larger prior values to the right. Strict comparison preserves the current stable `slice::sort_by` tie policy: a later equal radial/mouth/back time is inserted after earlier equal times. Candidate identity is discarded by the current `Vec<f64>` and the downstream loop observes only seconds, so equal source identities are not publicly distinguishable; nevertheless retain stable source order rather than relying on that accident.

A direct, auditable shape is:

```rust
for time in [
    radial_entry_seconds,
    mouth_entry_seconds,
    back_entry_seconds,
]
.into_iter()
.flatten()
.filter(|time| *time > f64::EPSILON)
{
    let mut insertion_index = analytic_entry_count;
    while insertion_index > 0
        && time < analytic_entries[insertion_index - 1]
    {
        analytic_entries[insertion_index] = analytic_entries[insertion_index - 1];
        insertion_index -= 1;
    }
    analytic_entries[insertion_index] = time;
    analytic_entry_count += 1;
}
```

Then iterate by value over the live prefix:

```rust
for &candidate_seconds in &analytic_entries[..analytic_entry_count] {
    // existing gap validation/refinement body, unchanged
}
```

Keep the code local to `compute_next_ball_pocket_capture_on_table`. A private generic fixed-vector type or dependency is more machinery than three `f64` values justify.

### 2. Preserve all current edge semantics

The replacement must preserve these details exactly:

- `None` contributes no candidate.
- `0.0`, negative values, and values `<= f64::EPSILON` are filtered.
- A `NaN` is also filtered today because `NaN > f64::EPSILON` is false; do not turn that case into a new panic or accepted candidate.
- Positive infinity passes the current filter and sorts after finite values; preserve that ordering even though current helper contracts are expected to produce finite roots.
- Equal candidates retain radial-before-mouth-before-back insertion order. Do not use `<=` in insertion shifting. Although equal bare `f64` values are downstream-indistinguishable, changing the stable policy is outside a storage-only refactor.
- The first candidate whose gap is within `capture_tolerance` still determines the refinement bracket and breaks the loop.
- The six-pocket iteration order, strict `<` best-result comparison, analytic-to-scan fallback, and final raw advancement remain byte-for-byte structurally unchanged outside the storage block. Equal-time captures in different pockets therefore continue to select the first pocket in `Pocket::ALL`.

Do not add a new finite-value assertion: the current `partial_cmp().expect(...)` never sees `NaN` after the filter, and changing release/debug behavior is outside a storage-only optimization.

### 3. Clean cutover

Delete the `collect::<Vec<_>>()` and `sort_by` path completely. Do not retain a feature flag, legacy helper, `SmallVec` fallback, or compatibility implementation. The stack buffer is private and cannot change public API shape.

## Benchmark plan

### Committed primary fixture

Use the already-added Criterion benchmark:

- **Full benchmark ID:** `pocket_predictors/capture/fast_side_analytic_hit`.
- **API:** `compute_next_ball_pocket_capture_on_table`.
- **Input:** the committed straight 200 in/s rolling CenterRight-side approach built by `rolling_side_pocket_state_at_angle`.
- **Setup discipline:** `OnTableBallState`, `BallSetPhysicsSpec`, `TableSpec`, and `OnTableMotionConfig` are already constructed outside `b.iter`.
- **Current preflight:** only `is_some()`. Before editing storage, strengthen a focused untimed characterization to exact `Pocket`, time bits, and complete state and independently prove that at least one positive radial/mouth/back candidate passes the gap check before fallback scanning. A benchmark name is not path evidence.
- **Timed result:** the committed closure already black-boxes all inputs and the complete `Option<PredictedBallPocketCapture>`.
- **Configuration:** the group currently uses an 8-second measurement time, 20 samples, and the repository's 1-second warm-up. The current quick mean is about 10.12 ms/call.

Do not add the draft's duplicate `core_functions/compute_next_ball_pocket_capture_on_table/analytic_side_capture`. If path instrumentation disproves the committed fixture's analytic membership, correct or add a distinctly named future fixture before collecting any implementation baseline; never keep an “analytic” acceptance gate that actually times fallback scanning.

### Committed and existing controls

Run these filters unchanged:

- `pocket_predictors/capture/slow_side_30deg_hit`;
- `pocket_predictors/capture/slow_side_30deg_target_miss`;
- `pocket_predictors/jaw/fast_side_hit`;
- `pocket_predictors/scheduler/one_ball_slow_side_capture`;
- `pocket_cache_rebuild/one_ball_bank/event_limit_1`;
- `pocket_cache_rebuild/two_ball_pocket/event_limit_1`;
- `end_to_end/direct/pocket_aware_until_rest_cached`;
- `end_to_end/direct/pocket_aware_until_rest_manual`; and
- `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8`.

They verify that the local container change does not regress capture miss/hit behavior, unrelated jaw prediction, or combined pocket-aware paths; they are not primary proof of a small allocation optimization.

### Baseline protocol

Validate exact output and analytic branch membership on the unchanged tree before editing production storage. On the same host, power mode, Rust toolchain, release profile, and committed Criterion configuration, save three independent pre-change baselines and compare three post-change processes to the corresponding baseline:

```text
cargo bench --bench physics -- 'pocket_predictors/capture/fast_side_analytic_hit' --save-baseline fixed-capture-before-1
cargo bench --bench physics -- 'pocket_predictors/capture/fast_side_analytic_hit' --baseline fixed-capture-before-1
```

Repeat with `-2` and `-3`. Record the point estimate and both bounds of Criterion's 95% relative-change CI for every pair; three resamples in one process are not three process repetitions. If the 20-sample policy is too noisy, change it before collecting both sides of all three pairs; do not compare unlike sampling policies.

## Allocation and assembly contract

Profile an equal, fixed batch of 10,000 warmed fast-side analytic calls before and after, with fixture/spec construction outside the measured region. Use the platform allocation call tree plus release assembly inspection for `compute_next_ball_pocket_capture_on_table`.

The change is accepted only if:

1. no allocation or deallocation stack remains for the analytic-entry `collect::<Vec<f64>>()` site;
2. release assembly contains no `RawVec` growth/allocation/deallocation path attributable to the three analytic entries and no call to the general slice sort for this block;
3. the buffer remains stack-local and exactly three `f64` slots; it is not boxed or captured by a heap-allocating closure;
4. total allocation count and bytes for the 10,000-call batch do not increase;
5. the report explicitly distinguishes remaining allocations in root helpers or other predictor code—do not claim that the complete capture predictor is allocation-free unless the profile independently proves it.

If symbol inlining makes call-tree attribution ambiguous, temporarily mark a local measurement-only helper `#[inline(never)]`, collect the profile, and remove that attribute before landing. The fixed upper bound and assembly are the durable contract; do not add a permanent global allocator or a runtime allocation counter for three scalar slots.

## Correctness and exact-equivalence tests

### Characterize before editing

Before changing the storage block, add fixtures which call the public capture predictor and record the exact `Option<PredictedBallPocketCapture>` result. The expected value must include:

- `Pocket` identity;
- exact `Seconds` value, not an epsilon comparison; and
- complete `state_at_capture` position, height, linear and vertical velocity, and angular velocity.

After the implementation, use `assert_eq!` on the full option/result. Do not infer exactness from the committed benchmark's current `is_some()` preflight, and do not retain a production legacy implementation just to make the comparison.

### Required behavioral matrix

Add or extend focused coverage for:

1. **Analytic straight side hit:** `a_single_ball_heading_into_the_side_pocket_predicts_capture_before_the_rail`, using `rolling_toward_center_right_side_pocket`.
2. **Analytic straight corner hit:** `a_ball_heading_cleanly_into_a_corner_pocket_still_predicts_capture`.
3. **No positive analytic candidate / fallback scan:** construct and prove a fixture for which all three analytic options are absent or rejected but `scan_ball_pocket_capture_time_during_current_phase_raw` finds the capture. Assert the full public result exactly. If no such stable fixture exists, that is a stop condition for claiming fallback equivalence; do not relabel an analytic miss returning `None` as a fallback hit.
4. **No capture:** `a_fast_straight_side_pocket_entry_outside_the_tp37_target_width_is_rejected`; exact result is `None`.
5. **Filtered boundary:** internal unit coverage for candidate values `None`, negative, `0.0`, exactly `f64::EPSILON`, and just above epsilon; only the last is retained.
6. **Stable tie:** exercise the candidate insertion boundary so a second and third bit-identical time are inserted after the existing equal prefix, preserving radial, mouth, back source order. If the production helper exposes only bare `f64` output, assert returned insertion positions or use test-only tagged inputs rather than claiming equal bare values prove identity ordering.
7. **Near tie:** use `f64::from_bits` neighbors around one finite time to prove strict numeric ordering without collapsing candidates.
8. **Permutation/count matrix:** zero, one, two, and three present candidates in different source positions, asserting the exact initialized prefix and length.
9. **Pocket tie precedence:** characterize a fixture or focused internal candidate comparison in which two pockets have bit-identical capture times and prove the existing strict best-result comparison retains the first `Pocket::ALL` entry.

The tiny ordering tests should exercise a private, non-generic insertion helper only if extracting that helper improves clarity without adding runtime abstraction. A test must be able to distinguish stable source positions; comparing `[t, t, t]` alone cannot detect a tie-order bug. Behavioral capture fixtures remain mandatory because container-order tests alone do not prove unchanged refinement/output.

Existing named regressions `a_slow_angled_side_pocket_entry_outside_the_tp35_target_curve_is_rejected`, `a_fast_ball_entering_a_side_pocket_between_old_scan_samples_predicts_capture`, `side_pocket_capture_waits_until_the_ball_reaches_the_mouth_plane`, `a_slow_angled_corner_pocket_entry_outside_the_tp36_target_curve_is_rejected`, `a_fast_corner_pocket_entry_uses_tp38_signed_target_asymmetry`, and `a_fast_corner_pocket_entry_beyond_the_tp38_effective_target_angle_is_rejected` must continue to cover slow/fast side and corner acceptance, mouth-plane timing, target rejection, and signed corner asymmetry.

Run focused verification:

```text
cargo test --test n_ball_pockets
cargo test --lib pocket_mouth_tests
cargo bench --bench physics --no-run
```

## Statistical acceptance gates

This is a small candidate, so allocation removal is mandatory and the latency gate is intentionally modest but nonzero:

1. **Primary microbenchmark:** analytic branch membership is independently established; in at least two of the three paired process comparisons, Criterion's 95% relative-mean CI is wholly below 0%. In the third, the upper CI bound must be no greater than +1.0%.
2. **Practical threshold:** the median of the three point-estimate improvements for `pocket_predictors/capture/fast_side_analytic_hit` must be at least 1.0%. If the effect is smaller, reject the production change as unproven even if allocation disappears.
3. **Allocation/assembly:** all five allocation and assembly conditions above must pass.
4. **Adjacent paths:** none of the committed capture, jaw, scheduler, cache-rebuild, or older pocket-aware controls may have a 95% CI whose regression bound exceeds 2.0%.
5. **Correctness:** every public `Option<PredictedBallPocketCapture>` characterization is exactly equal before/after, candidate stable ordering is proved with distinguishable source positions, and pocket tie precedence is unchanged. No tolerance widening or accepted last-bit drift is allowed for a storage-only change.

## Risks, stop conditions, and rollback

### Risks

- Using `<=` while shifting would reverse equal-time source precedence.
- Iterating all three physical slots instead of the initialized prefix would introduce zero-valued candidates.
- Incrementing the length before shifting can write beyond index 2 on the third insertion.
- A sorting network with sentinels can mishandle `None`, infinity, or ties.
- The committed benchmark may be named `analytic_hit` while actually reaching fallback scanning; branch instrumentation, not the name or final `Some`, decides whether it is a valid primary fixture.
- Compiler optimization may already elide part of the allocation on one toolchain; source shape alone is not proof of a measurable win.

### Stop/reject conditions

Reject or revise the patch if any condition holds:

- exact full-result equality fails for any capture fixture;
- stable tie/source order changes;
- fallback behavior cannot be characterized independently;
- allocation/deallocation or general sort remains attributable to the candidate block;
- the primary benchmark misses its CI or 1% practical gate;
- an adjacent control crosses the 2% regression guardrail;
- the implementation needs unsafe code, a dependency, or a generic container abstraction to replace three local values.

If compiler output proves the current `Vec` allocation was already fully eliminated and the timing gate is missed, retain the benchmark/tests if useful and reject the production rewrite rather than claiming a source-aesthetic performance win.

### Rollback

Rollback is a single local production replacement: restore the three-option flatten/filter collection and stable `sort_by`. Keep the committed `pocket_predictors` benchmark, which predates this implementation. No public API, serialized output, cache state, or persisted artifact changes.

## arm64 SIMD and GPU decision

- **arm64 SIMD/NEON:** Not justified. There are at most three scalar values plus `Option`/filter branches. SIMD setup, masks, missing-lane representation, and horizontal ordering would exceed the work of three stable scalar insertions and could make tie behavior harder to audit.
- **GPU:** Rejected. The operation is 0–3 scalar insertions embedded in a branch-heavy, latency-sensitive predictor. GPU dispatch/transfer cannot amortize over this container and would not remove CPU-side candidate generation or deterministic refinement.

## Candidate commit message

`perf: store pocket capture candidates on the stack`
