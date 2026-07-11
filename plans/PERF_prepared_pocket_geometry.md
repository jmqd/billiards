# Prepare pocket-capture geometry once per query

## Status

- **Status:** Implemented in `d0bf626` (`perf: prepare pocket capture geometry once per query`).
- **Priority:** P0 among pocket-capture micro-optimizations.
- **Confidence:** High that the work is redundant and removable; medium-high that the committed target-miss fixture can clear the latency gate once its slow-corner scan/cache attribution is confirmed.
- **Implementation order:** Implement and benchmark this plan before `PERF_current_phase_prediction_context.md`. There is no semantic dependency, but pocket scanning currently dominates the committed one-ball scheduler fixture, so removing geometry preparation from each gap evaluation makes the later context result measurable. Land and assess this change independently before changing phase preparation.
- **Broader dependencies:** None. This plan deliberately precedes, and does not depend on, adaptive capture search or incremental `PocketAwareEventCache` work.
- **Result:** Three paired release-process comparisons put the primary target-miss median at **-86.4%**; every primary 95% CI was wholly negative. Slow/fast capture, scheduler, cache-rebuild, and end-to-end controls improved; same-policy resampling cleared noisy jaw/rail negative-control gates.
- **Corroboration:** Exact-bit, custom-table, per-radius, stored-`None`, and call-local key tests pass. `xctrace` was unavailable on the host, so Instruments Time Profiler/lock traces could not be collected; source/type review confirms the prepared evaluator has no global mutex/hash lookup or heap-owned six-record scratch.

## Problem and evidence

### Measured facts

The unchanged-tree macOS `sample` profile recorded in `local://perf-plan-context.md` attributes 3,476 samples to `compute_next_ball_pocket_capture_on_table` and 3,470 to `scan_ball_pocket_capture_time_during_current_phase_raw` in the three-ball/eight-event workload. The same profile contains the global slow-corner transition cache and repeated `BigDecimal`-to-`f64` conversion path. The committed `pocket_predictors` group now provides direct quick baselines of about 9.45 ms/call for `capture/slow_side_30deg_hit` and 11.61 ms/call for `capture/slow_side_30deg_target_miss`; its jaw and one-ball scheduler controls are about 34.7 us and 9.58 ms. The older holistic snapshots remain approximately 495–504 ms/call for `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8` and 39.2 ms/call for `end_to_end/direct/pocket_aware_until_rest_cached`.

The committed capture fixtures construct inputs outside `b.iter`, preflight only `Some`/`None`, and black-box the complete `Option<PredictedBallPocketCapture>`. These measurements identify the pocket scan as a hot path, but neither the benchmark name nor a final `None` proves that the target-miss fixture traverses fallback scanning and the slow-corner transition lookup. Branch attribution and exact preflight characterization are required before using it as the primary implementation gate.

### Source-observed repeated work

All symbols below are currently in `src/lib.rs`.

- `compute_next_ball_pocket_capture_on_table` iterates `Pocket::ALL`. For each pocket it evaluates an initial gap, up to three analytic candidates, a 60-iteration refinement, and, on fallback, up to 512 scan samples.
- Every evaluation enters `pocket_capture_gap_during_current_phase_raw`, which reconstructs target bounds, lateral geometry, acceptance geometry, the mouth plane, and the back plane from `TableSpec`.
- `pocket_target_bounds_in_inches` rebuilds both slow and fast target geometry. `side_pocket_slow_target_geometry`, `corner_pocket_slow_target_geometry`, and `fast_pocket_target_geometry` repeatedly resolve pocket width/depth and the long-rail length.
- `pocket_lateral_offset_raw`, `pocket_mouth_plane_gap_raw`, and `pocket_back_plane_gap_raw` repeatedly call `pocket_center_in_inches`, `pocket_jaw_reference_point_in_inches`, and `pocket_slow_capture_radius_in_inches`. The jaw reference path also resolves table face coordinates and pocket width.
- `TableSpec::diamond_to_inches` multiplies `BigDecimal` values. The hot helpers immediately convert the result back to `f64`.
- For a corner pocket, `pocket_slow_target_bounds_in_inches` calls `corner_pocket_slow_target_sleft` for both signed sides. Each call invokes `corner_pocket_slow_target_transitions`, constructs a six-field key, locks `SLOW_CORNER_POCKET_TRANSITION_CACHE`, hashes the key, copies the cached `Option`, and unlocks. This occurs inside every gap evaluation even after the expensive transition roots are cached.
- The persistent `SlowCornerPocketTargetTransitionKey` contains exact `f64::to_bits()` values for mouth width, wall angle, hole radius, shelf depth to hole, long-rail length, and ball radius. The process-global `SLOW_CORNER_POCKET_TRANSITION_CACHE` intentionally caches both success and `None`.
- Analytic setup also repeats the same center, capture radius, mouth projection, and back projection in `first_pocket_mouth_plane_crossing_time_during_current_phase_raw`, `first_pocket_back_plane_crossing_time_during_current_phase_raw`, and `compute_next_ball_pocket_capture_on_table`.

### Hypothesis to test

Preparing the six immutable pocket records once at predictor entry will remove synchronization, hashing, and pocket/table-geometry decimal conversion from the gap/refinement/scan loops. The source-level work removal is certain. Pre-existing motion/configuration conversions reached through `raw_advance_within_phase_on_table` are outside this plan. A steady-state latency improvement of at least 10% for a slow-corner miss is the acceptance hypothesis, not a claim established by the existing profile.

## Scope

Prepare only geometry used by one call to `compute_next_ball_pocket_capture_on_table`:

1. resolve the six pockets from the exact `TableSpec` and ball radius supplied to that call;
2. resolve slow-corner transition data at most once per distinct existing transition key during preparation;
3. pass immutable prepared records through analytic candidate generation, gap evaluation, scan, and bisection; and
4. preserve the public predictor signature and result exactly.

The implementation may refactor table-based internal target-bound helpers to prepare one temporary record and delegate to the same prepared evaluator where those helpers are still needed by aiming or post-jaw code. There must be one target-bound formula, not a second independently maintained convention.

## Explicit non-goals

- Do not change pocket target equations, constants, target interpolation, signed-angle conventions, acceptance windows, capture radius, mouth/back-plane inequalities, tolerances, analytic root ordering, the 512-step scan, or the 60-step bisection.
- Do not change which pocket wins equal-time selection or when `state_at_capture` is materialized.
- Do not move preparation into `TableSpec`, add a persistent table cache, or alter the lifecycle/rebuild strategy of `PocketAwareEventCache`. `TableSpec::pockets` and pocket shapes are publicly mutable, so a hidden long-lived derived cache would need invalidation that is outside this plan.
- Do not remove or redesign `SLOW_CORNER_POCKET_TRANSITION_CACHE`; only remove repeated access to it from evaluation loops.
- Do not combine this work with fixed analytic-candidate storage, adaptive capture search, jaw preparation, rail preparation, raw motion classification, or general `BigDecimal` removal.
- Do not alter public APIs or physics output. Determinism and bit-for-bit predictor output are required.

## Design

### Immutable records

Add private, stack-owned records with equivalent names to the following shape:

```rust
#[derive(Clone, Copy)]
enum PreparedSlowPocketTarget {
    Side(SlowPocketTargetGeometry),
    Corner {
        geometry: SlowCornerPocketTargetGeometry,
        transitions: Option<SlowCornerPocketTargetTransitions>,
    },
}

#[derive(Clone, Copy)]
struct PreparedPocketCaptureGeometry {
    pocket: Pocket,
    center_x: f64,
    center_y: f64,
    entry_x: f64,
    entry_y: f64,
    tangent_x: f64,
    tangent_y: f64,
    capture_radius: f64,
    mouth_projection_minus_ball_radius: f64,
    back_projection: f64,
    slow_max_entry_angle_degrees: f64,
    fast_max_entry_angle_degrees: f64,
    slow_target: PreparedSlowPocketTarget,
    fast_target: FastPocketTargetGeometry,
}

struct PreparedPocketCaptureQuery {
    ball_radius: f64,
    pockets: [PreparedPocketCaptureGeometry; 6],
}
```

Names may follow local style, but the represented values and ownership must be explicit. Store the radius once on the query, not six times on the pocket records, and make every radius-dependent evaluator a query method so a prepared record cannot be paired with a different radius argument. Do not store `&TableSpec` in either record; that would permit accidental decimal conversion in the hot evaluator. The prepared array is indexed/iterated in the existing `Pocket::ALL` order.

For each record, compute once and in the same arithmetic order as today:

- `center_x/center_y` from `pocket_center_in_inches`;
- the entry axis from `pocket_entry_axis` and its tangent `(-entry_y, entry_x)`;
- `capture_radius` from `pocket_slow_capture_radius_in_inches`;
- the first-jaw mouth projection, then subtract `ball_radius`, matching `pocket_mouth_plane_gap_raw`;
- `back_projection = entry · center + capture_radius`, matching `pocket_back_plane_gap_raw`;
- the pocket-type-specific slow/fast maximum entry angles;
- the existing slow target geometry and fast target geometry; and
- for corner pockets, the `Option<SlowCornerPocketTargetTransitions>` obtained with the existing exact key.

Preparing the already-subtracted mouth threshold is safe only if it is computed as the current expression in the current order. Do not algebraically reassociate calculations in this refactor.

### Cache key and lifetime

Keep the process-global transition cache and its key semantics unchanged:

- key: the six existing `to_bits()` fields in `SlowCornerPocketTargetTransitionKey`;
- value: `Option<SlowCornerPocketTargetTransitions>`, including cached `None`;
- lifetime: process lifetime, as today;
- miss behavior: preserve the current root solve and insertion behavior, including the current compute-outside-lock structure.

During `PreparedPocketCaptureQuery::new(table, ball_radius)`, use fixed stack scratch for up to six resolved corner keys, not four. Although a standard table has four corner pockets, every `TableSpec` pocket specification is mutable and a custom table can classify any of the six positions as `PocketType::Corner`; scratch capacity and iteration must therefore be derived from the six-entry `Pocket::ALL` domain rather than from standard-table topology. Before consulting the global map for a corner, compare the full existing key against keys already resolved in this query. Reuse the exact `Option` on a bit-identical key; otherwise perform one current global lookup/solve and store the result in scratch. Do not introduce another `HashMap` or heap allocation. On a standard table, the four identical corner keys therefore cause one global lookup during preparation, not four. Custom pocket types or per-pocket widths may produce as many as six distinct keys and values.

The query record itself has no persistent key. Its validity identity is the exact, current borrowed constructor inputs: all `TableSpec` contents observed by the existing geometry helpers plus `ball_radius.to_bits()`. It is created after the public predictor's `has_pockets` and phase-horizon early exits, borrowed immutably for that one predictor call, and dropped on return. Its private methods fetch geometry from `self.pockets` and use only `self.ball_radius`; they must not accept a second radius or a geometry record from another query. The query must never be stored in `PocketAwareEventCache` or reused across calls.

### Prepared evaluation API

Make the prepared operations private methods on `PreparedPocketCaptureQuery`, keyed by the current `Pocket::ALL` index, rather than free functions that accept geometry and radius independently:

```rust
impl PreparedPocketCaptureQuery {
    fn target_bounds_in_inches(
        &self,
        pocket_index: usize,
        signed_entry_angle_degrees: f64,
        speed: f64,
    ) -> (f64, f64);

    fn capture_gap_during_current_phase(
        &self,
        state: RawOnTableBallState,
        phase: MotionPhase,
        pocket_index: usize,
        t_seconds: f64,
        config: &OnTableMotionConfig,
    ) -> f64;
}
```

Each method obtains `let geometry = &self.pockets[pocket_index]` and uses `self.ball_radius`. Do not expose a method taking an arbitrary `&PreparedPocketCaptureGeometry` plus a radius; that API would allow a custom-table record to be evaluated with a radius different from its target transitions and mouth threshold.

The prepared target evaluator must call a corner slow-target helper that accepts the record's resolved `transitions` directly. `corner_pocket_slow_target_sleft` must not perform a global lookup when called from a prepared evaluator. Both signed sides use the same prepared `Option`.

The prepared gap evaluator must derive, in the current order:

1. `at_t` with `raw_advance_within_phase_on_table` and `self.ball_radius`;
2. signed entry angle and slow/fast interpolated target bounds;
3. lateral offset from prepared center/tangent;
4. target gap;
5. entry-angle acceptance gap using the prepared slow/fast maximum-angle constants;
6. mouth gap from the prepared mouth threshold; and
7. back gap from the prepared back projection.

Return the same chained `max` expression in the same order. It must not accept `&TableSpec` or another radius, call `diamond_to_inches`, or call the global transition cache.

Similarly, make the capture-only mouth-plane and back-plane analytic crossing helpers query methods keyed by the same pocket index. The radial candidate uses that indexed record's center/radius geometry and `self.ball_radius` for motion. `refine_ball_pocket_capture_time_during_current_phase_raw` and `scan_ball_pocket_capture_time_during_current_phase_raw` accept the query plus pocket index and forward both unchanged.

### Public and non-capture behavior

`compute_next_ball_pocket_capture_on_table` remains public with its current signature. After `table.has_pockets()` and after the existing raw phase/horizon setup, construct `PreparedPocketCaptureQuery` once, then zip/iterate it in exact `Pocket::ALL` order. This placement avoids preparation for pocketless tables and for non-finite/epsilon horizons.

Existing object-aiming and post-jaw helpers that accept `Pocket` plus `&TableSpec` may remain table-shaped wrappers, but they must delegate to the prepared primitive rather than preserve a second formula. Their preparation remains local to those calls and is not broadened into this optimization.

## Ordered implementation

1. **Characterize the committed fixtures first.** Record exact complete outputs for `pocket_predictors/capture/slow_side_30deg_target_miss` and `/slow_side_30deg_hit`, and use temporary instrumentation or a debugger to prove which pockets enter fallback scan and the slow-corner transition path. Do not add a duplicate benchmark merely because the original draft predated `bench_pocket_predictors`.
2. **Capture exact baseline signatures.** For the correctness matrix below, record full result signatures and internal gap `f64::to_bits()` values from the unchanged implementation.
3. **Introduce preparation types and constructor.** Resolve all six records with six-entry fixed storage and query-local exact-key reuse. Add constructor-level tests for standard and customized pocket geometry/type assignments.
4. **Separate transition lookup from evaluation.** Add a corner slow-target evaluator that receives the already-resolved transition `Option`; retain the existing lookup helper only at preparation boundaries and table-shaped wrappers.
5. **Cut the gap path over.** Convert target bounds, lateral/angle acceptance, mouth, back, scan, and refinement to prepared records. Remove any now-obsolete capture-only table-shaped helper; leave no compatibility alias.
6. **Cut analytic candidates over.** Use prepared center/radius/projections without changing root generation, filtering, sorting, candidate order, tolerance, or refinement.
7. **Run exact equivalence tests, then the committed predictor filters and profile gates.** Only after this plan passes should `PERF_current_phase_prediction_context.md` establish its own baseline.

## Benchmark plan

### Committed primary fixture

Use the already-added Criterion benchmark:

`pocket_predictors/capture/slow_side_30deg_target_miss`

It constructs a default table, ball set, motion configuration, and a slow rolling 30-degree CenterRight approach with a 1.8-inch perpendicular offset outside the target. Setup is outside `b.iter`; the timed closure black-boxes all references and the complete `Option<PredictedBallPocketCapture>`. The current quick baseline is about 11.61 ms/call under the group's committed 8-second/20-sample configuration.

Before taking the implementation baseline, strengthen the untimed characterization outside the benchmark loop or in a focused test: record exact `None`, then prove with temporary instrumentation/debugger attribution that the call reaches fallback gap scanning for one or more corner pockets and exercises the slow-corner transition-cache lookup. The committed untimed preflight calls already warm the process-global transition cache before Criterion sampling; retain that ordering and exclude any first-time root solve from both sides of the timed comparison. The existing `is_none()` assertion establishes output class only; it does not establish either internal path.

Use the committed paired control:

`pocket_predictors/capture/slow_side_30deg_hit`

Its quick baseline is about 9.45 ms/call. Record and assert the exact pocket, capture time bits, and full state signature outside timing before the production change; the current benchmark only asserts `is_some()`. Keep black-boxing the complete option.

### Future matrix extension, only if attribution requires it

Do not add another fixture by default. Add

`pocket_predictors/capture/slow_corner_30deg_target_miss`

only if branch instrumentation shows the committed target miss does not exercise the slow-corner fallback/cache mechanism strongly enough to attribute the proposed work removal. Construct it from TopRight geometry, assert exact `None`, and prove fallback membership before timing. If the miss and capture branches differ materially, add a centered `slow_corner_30deg_hit` control. These names describe future extensions, not benchmarks currently present.

### Committed and existing controls

Run these separately; do not combine their samples with the primary estimate:

- `pocket_predictors/capture/fast_side_analytic_hit`;
- `pocket_predictors/jaw/fast_side_hit`;
- `pocket_predictors/scheduler/one_ball_slow_side_capture`;
- `pocket_cache_rebuild/one_ball_bank/event_limit_1`;
- `pocket_cache_rebuild/two_ball_pocket/event_limit_1`;
- `end_to_end/direct/pocket_aware_until_rest_cached`;
- `end_to_end/direct/pocket_aware_until_rest_manual`;
- `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8`; and
- `core_functions/compute_next_ball_rail_impact_on_table`.

The jaw and rail filters are negative controls: this plan must not broaden capture preparation into jaw or standalone rail prediction. The scheduler, cache-rebuild, and end-to-end cases are mixed integration controls, not isolated evidence for geometry preparation.

### Paired procedure and statistical gate

Use the same pinned arm64 host, power mode, Rust toolchain, release profile, feature set, environment, and the committed Criterion configuration for both sides of each pair. Do not compare results collected on different code plus unrelated changes, and do not compare the new 8-second/20-sample fixture against the older 5-second/20-sample groups as though they were one estimate.

For each of three independent process pairs, save the unchanged baseline and compare the implementation against that pair's baseline:

```text
cargo bench --bench physics -- 'pocket_predictors/capture/slow_side_30deg_target_miss' --save-baseline prepared-pocket-r1
cargo bench --bench physics -- 'pocket_predictors/capture/slow_side_30deg_target_miss' --baseline prepared-pocket-r1
```

Repeat as `r2` and `r3` from fresh processes. Report Criterion's relative mean estimate and both 95% confidence bounds for every pair, plus the median of the three point estimates. If the committed sample policy proves noisy, change it before collecting either side of all three pairs; never compare unlike policies.

Accept only when all are true:

1. attribution proves the committed miss exercises the intended slow-corner scan/cache mechanism, or the explicitly future slow-corner fixture replaces it as primary;
2. the 95% confidence interval for relative mean latency is wholly below 0% in all three primary comparisons;
3. the median primary improvement is at least 10%;
4. the committed slow-hit and fast-analytic-hit controls have no 95% CI upper bound above +2%; and
5. none of the scheduler, cache-rebuild, cached/manual/pinball, jaw, or rail controls has a 95% CI upper bound above +2%.

A faster holistic benchmark cannot rescue a failed isolated gate, and an isolated win cannot excuse an adjacent-path regression.

### Work-removal corroboration

Profile an equal fixed number of warmed target-miss invocations before and after with Instruments Time Profiler plus Points of Contention/Locks; use Allocations if pocket/table-geometry `BigDecimal` frames are allocation-visible. Setup and the first warm-up call are excluded.

After the change require:

- no `Mutex<HashMap>` lock/hash lookup beneath the prepared gap, scan, or refinement stack;
- no `TableSpec::diamond_to_inches`, pocket/table-geometry `BigDecimal` multiplication/conversion, `pocket_center_in_inches`, `pocket_jaw_reference_point_in_inches`, or geometry-constructor frame beneath the prepared gap stack; pre-existing motion/configuration conversions beneath `raw_advance_within_phase_on_table` are explicitly outside this gate;
- at most one global slow-corner transition lookup per distinct exact six-field key during a predictor invocation, including custom tables with more than four corner-typed pockets, confirmed by an instrumented debug run or debugger breakpoint count; and
- no new heap allocation for the six-record prepared query or its six-entry key scratch.

This profile corroborates the mechanism. Criterion remains the latency acceptance authority.

## Correctness and exact-output gates

Before editing the evaluator, record golden signatures from the current implementation. Permanent tests must compare exact pocket identity and `to_bits()` for time plus every raw state component; tolerance-only assertions are insufficient for this preparation-only refactor.

Required matrix:

1. centered slow 30-degree side capture and offset rejection from `a_slow_angled_side_pocket_entry_outside_the_tp35_target_curve_is_rejected`;
2. centered and rejected slow 30-degree corner entries on both mirrored signed sides, extending `a_slow_angled_corner_pocket_entry_outside_the_tp36_target_curve_is_rejected`;
3. fast straight side capture between old scan samples from `a_fast_ball_entering_a_side_pocket_between_old_scan_samples_predicts_capture`;
4. fast side and corner signed-angle accept/reject cases, including `a_fast_corner_pocket_entry_uses_tp38_signed_target_asymmetry` and `fast_corner_target_bounds_follow_pocket_local_signs_for_mirrored_corners`;
5. mouth-plane wait and target-expansion-before-first-old-scan regressions from `side_pocket_capture_waits_until_the_ball_reaches_the_mouth_plane` and `pocket_capture_brackets_target_expansion_before_the_fixed_scan_can_sample_it`;
6. a fallback-scan miss, an analytic capture, and an immediate zero-time capture;
7. a table with one corner width and depth changed while the other corners remain default, plus a custom table whose pocket-type assignments exercise more than four corner records and six distinct keys;
8. two alternating calls using default/custom table specs and two ball radii, proving query-local values, pocket types, and transition keys do not leak between calls;
9. a pocketless table, which must still return `None` without preparing pocket records; and
10. a private API-boundary test or type-shape review proving no prepared evaluator accepts a second ball radius or a record detached from its owning query.

Add an internal prepared-evaluator test over fixed `(pocket, signed angle, speed, time)` cases. Compare each final gap's `f64::to_bits()` with the unchanged-path golden value. Include a corner whose transition solve returns `Some`, any constructible geometry whose transition solve returns `None` if one exists, and custom per-pocket width/depth. If no valid `None` geometry exists, retain the cache-value unit test that proves a stored `None` is passed through unchanged rather than inventing an invalid production table.

For public predictions, require exact `Option<PredictedBallPocketCapture>` signatures before/after: pocket, time, position, velocity, and angular velocity. Also run all existing focused pocket tests. Any bit drift is rejection, even when within current tolerances, because this change only moves invariant calculations out of a loop.

## Risks and mitigations

- **Stale/custom geometry or radius:** A record accidentally reused across tables, radii, or calls can accept the wrong pocket. Keep the query stack-local, store radius once on it, expose only index-keyed query methods, and test alternating custom/default calls.
- **Signed corner asymmetry:** Reusing one transition record is valid only because the current key covers geometry/radius while `theta` remains an evaluator input. Test both signs and mirrored pockets.
- **Cached `None` loss:** Flattening `Option<Option<_>>` incorrectly can recompute or fabricate transitions. Preserve `HashMap::get` hit semantics and test `None` pass-through.
- **Arithmetic drift:** Precomputing projections can change rounding if expressions are reassociated. Preserve current operation order and enforce `to_bits()` goldens.
- **Hidden preparation cost:** A faster gap with overly expensive query construction may lose on immediate exits. The isolated capture control and end-to-end guardrails cover this.
- **Duplicate formula:** Leaving old and prepared target math side by side invites divergence. Route table-shaped callers through the prepared primitive.

## Stop, rejection, and rollback

Reject or revise the implementation if exact-output tests drift, any cache key/table/radius lifetime invariant is violated, six-entry custom corner coverage fails, the prepared gap still reaches the mutex or any pocket/table-geometry decimal conversion path, the 10% primary latency threshold is missed, or an adjacent-path upper confidence bound exceeds +2%.

Rollback is a single independent production change: restore table-shaped capture helpers and remove the private preparation records. Keep the committed `pocket_predictors` benchmarks; they predate the implementation and remain useful hotspot fixtures. Remove only a future slow-corner extension if it is misleading or cannot be made deterministic.

## arm64 SIMD and GPU decision

**arm64 SIMD: not justified.** The removable work is synchronization, hashing, exact-decimal conversion, branch-heavy trigonometry/root selection, and six heterogeneous pockets. There is no wide regular lane structure, and NEON would not address the measured mechanism.

**GPU: not justified.** One query contains tiny dependent scans with early exits, global-cache traffic, and scalar root/refinement control flow. Transfer/dispatch overhead would dominate. This plan improves scalar preparation and does not claim GPU readiness.

## Candidate commit message

`perf: prepare pocket capture geometry once per query`
