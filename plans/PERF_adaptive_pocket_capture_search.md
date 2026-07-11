# Adaptive pocket-capture search

## Status

- **Status:** Implemented in `e2e2425` (benchmark contract) and `329e946` (`perf(physics): adaptively bracket pocket capture search`).
- **Priority:** P0; the measured pocket-aware critical path is dominated by capture prediction.
- **Confidence:** High that work can be removed without changing the model; medium-high that the interval proof will prune most miss paths. The statistical and differential gates below decide whether the implementation ships.
- **Owner scope:** `compute_next_ball_pocket_capture_on_table` and its private search helpers in `src/lib.rs`, focused pocket benchmarks in `benches/physics.rs`, and capture/simulation equivalence tests.
- **Result:** Across three paired release-process comparisons, median changes were **-98.3%** for `far_miss`, **-94.4%** for the sliding `gate_miss_60deg_80ips`, **-91.4%** for the constant-composition batch/64 cell, and **-84.2%** for the three-ball/eight-event endpoint. Every required pocket/end-to-end CI was wholly negative; the standalone rail control stayed within its +3% gate.
- **Correctness:** The adaptive path preserves the legacy 512-cell lattice, left-to-right first-entry ordering, and 60-step refiner. Differential tests include curved rolling, tiny nonzero quadratic acceleration, tiny-deceleration curvature, boundary, analytic, fallback, and exact public-state cases.

## Dependencies and landing order

1. Add the benchmark and differential-oracle fixtures in this plan against the unchanged implementation and save the Criterion baseline.
2. Land `PERF_prepared_pocket_geometry.md` first. The search needs immutable per-pocket raw centers, axes, mouth/back planes, and conservative target envelopes; it must not reintroduce `BigDecimal` work in every bound evaluation.
3. Land `PERF_current_phase_prediction_context.md` before this plan so phase, horizon, radius, and raw trajectory data have one authoritative representation.
4. Land `PERF_fixed_capture_candidates.md` before changing the search if it is ready; that change is independent and leaves the analytic-candidate semantics easier to freeze in the oracle.
5. Implement the search in the phases below. Land this before `PERF_incremental_pocket_event_cache.md` so the cache oracle is built around the final capture predictor.

This order is required for attribution. Do not combine raw-geometry conversion, candidate-container replacement, adaptive search, and incremental cache repair in one benchmark comparison.

## Problem and evidence

`compute_next_ball_pocket_capture_on_table` (`src/lib.rs:9135-9290`) classifies one current motion phase, derives its horizon, then visits every member of `Pocket::ALL`. For each pocket it:

1. evaluates `pocket_capture_gap_during_current_phase_raw` at time zero;
2. tries radial-circle, mouth-plane, and back-plane analytic candidate times (`src/lib.rs:9181-9226`);
3. if none validates, calls `scan_ball_pocket_capture_time_during_current_phase_raw` (`src/lib.rs:8988-9039`);
4. the fallback evaluates as many as 512 uniformly spaced times per pocket and then performs 60 bisection iterations in `refine_ball_pocket_capture_time_during_current_phase_raw` (`src/lib.rs:8956-8986`).

A gap evaluation advances the raw state and recomputes the signed entry angle, speed-dependent left/right target bounds, lateral offset, acceptance gate, mouth-plane gap, and back-plane gap (`src/lib.rs:8932-8954`). The gap is the maximum of those necessary predicates.

Measured facts:

- The unchanged three-ball/eight-event profile placed 3,476 samples below `compute_next_ball_pocket_capture_on_table`, 3,470 below the fixed scan, and 3,842/3,893 below `PocketAwareEventCache::build`.
- The same path repeatedly reaches exact-decimal pocket geometry and the global slow-corner transition lookup. Those costs are handled by the prerequisite prepared-geometry plan, not hidden in this algorithm change.
- The committed `pocket_predictors` Criterion group already isolates `capture/slow_side_30deg_hit` (quick point about 9.45 ms/call), `capture/slow_side_30deg_target_miss` (about 11.61 ms/call), `capture/fast_side_analytic_hit`, `jaw/fast_side_hit`, and `scheduler/one_ball_slow_side_capture`. Inputs are constructed outside the timed closure and complete return values are black-boxed, but the current preflight checks only `Some`/`None` and does not prove which analytic/fallback branch ran. These are current evidence, not the future differential or scaling harness.
- Current quick endpoint points are approximately 495-504 ms for `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8` and 39 ms for each two-ball pocket-aware until-rest variant. Those endpoint results establish importance but do not separate hit, miss, analytic, and fallback branches. Assembly previously confirmed the all-pocket loop and scan/refine calls; that is direct evidence of work, not evidence that any unproved geometric rejection is valid.

The current broad plan in `plans/performance_engineering.md:200-271` suggests gating and bracketing, but it does not define a no-false-negative proof, a search order that preserves the current first capture, or a differential oracle. This plan supplies those contracts.

## Semantic contract and non-goals

The optimized predictor must remain an exact search implementation of the current capture predicates. It must preserve:

- the slow/fast target interpolation and the side/corner angle gates;
- the mouth-plane and back-plane requirements;
- current `capture_tolerance`, analytic-candidate ordering, first-pocket tie behavior, 60-step refinement, `PredictedBallPocketCapture` payload, and deterministic event ordering;
- the fast straight side-pocket regression at `tests/n_ball_pockets.rs:845-862`, whose valid capture occurs before the first old 512-step sample;
- rejected fast/angled entries, late drops, curved motion, custom table dimensions, and all six mirrored pockets.

Explicit non-goals:

- no widening or narrowing of a physical target, angle gate, capture plane, tolerance, or phase horizon;
- no new jaw/rattle model and no change to jaw-versus-capture event precedence;
- no approximate closest-approach test, sampled swept AABB, heuristic maximum subdivision depth, or “probably moving away” rejection;
- no event reordering and no relaxed simulation-output comparison;
- no public API compatibility shim for private search helpers; cut over the private caller cleanly after the oracle passes.

A fast false negative is a failed implementation. If an interval cannot be proven empty, the algorithm must subdivide or run the exact legacy leaf check; it may not reject the interval.

## Design: proven pruning with baseline-identical leaves

### 1. Preserve an explicit legacy oracle

Extract the existing fallback unchanged as `legacy_scan_ball_pocket_capture_time_during_current_phase_raw` behind `#[cfg(any(test, debug_assertions))]`. It retains the exact 512 sample times, `previous_gap > capture_tolerance && gap <= capture_tolerance` transition, and current refiner call. Release code must not call this whole-horizon oracle.

Add a test/debug search result record:

```rust
struct CaptureSearchStats {
    pockets_considered: u16,
    pockets_broad_phase_rejected: u16,
    interval_nodes: u32,
    exact_gap_evaluations: u32,
    legacy_leaf_cells: u16,
}
```

Counters are returned only from an internal instrumented entry point and are absent from the release public path. They establish branch membership before benchmarking; wall time alone is not proof that a valid capture was not discarded.

### 2. Build conservative pocket-local envelopes

Extend prepared pocket geometry with a private `PocketCaptureEnvelope`:

```rust
struct PocketCaptureEnvelope {
    entry_axis: [f64; 2],
    tangent_axis: [f64; 2],
    min_longitudinal: f64, // mouth projection - radius - capture_tolerance
    max_longitudinal: f64, // pocket projection + slow capture radius + capture_tolerance
    min_lateral: f64,      // -supremum(right target bound) - capture_tolerance
    max_lateral: f64,      //  supremum(left target bound) + capture_tolerance
}
```

The longitudinal slab follows from the scanner's accepted inequalities `pocket_mouth_plane_gap_raw <= capture_tolerance` and `pocket_back_plane_gap_raw <= capture_tolerance` (`src/lib.rs:7803-7824`), not from the zero-gap physical boundary. Likewise, the lateral interval must enclose `target_gap <= capture_tolerance`. Build the geometry's zero-tolerance base bounds once, then expand all four query bounds outward by the exact per-query `capture_tolerance = 1e-9 * horizon.max(1.0)`. The lateral base interval must be an outward-rounded algebraic enclosure of the same slow/fast target equations used by `pocket_target_bounds_in_inches` (`src/lib.rs:7585-7601`) over the full supported signed-angle domain and speed fraction `[0, 1]`. Do not infer it from a nominal pocket width unless a source invariant proves that bound for every `TableSpec`.

Implement the minimal private outward-rounded interval operations needed by the proof. Every lower bound is rounded toward negative infinity and every upper bound toward positive infinity after applying the model tolerance, including a separate documented floating-point error allowance for projection and trajectory evaluation. Arithmetic error padding is not a substitute for `capture_tolerance`. NaN or an unbounded intermediate means “unknown,” never “empty.” Unit tests must cover side/corner pockets, all mirrored signs, default and custom table dimensions, slow/fast interpolation endpoints, critical angles, extrema of the target equations, and separations immediately below/at/above the expanded tolerance boundary.

### 3. Use only necessary-condition broad-phase rejection

For a time interval `[a, b]`, compute conservative intervals for the ball center projected onto the pocket entry and tangent axes. For non-curved sliding/rolling motion, use the same quadratic motion coefficients as the current raw advance and include endpoint plus derivative-root extrema. For side-spin curved rolling, enclose displacement from the state at `a` by an outward-rounded path-length bound using the existing `raw_phase_planar_speed_upper_bound`; a loose disk is acceptable because it can only reduce pruning, not correctness.

An interval is empty only if at least one necessary accepted-capture condition is proven impossible for every time in it:

- projected longitudinal position cannot overlap the `capture_tolerance`-expanded `[min_longitudinal, max_longitudinal]`; or
- projected lateral position cannot overlap the `capture_tolerance`-expanded `[min_lateral, max_lateral]`; or
- an interval enclosure of the current acceptance-angle gap is wholly greater than `capture_tolerance`.

The first implementation should use the two positional proofs. Add the angle proof only if its outward bound is independently reviewable and benchmarks show it matters. “Velocity points away” is not sufficient when sliding acceleration or side-spin curvature can turn the path; it may be used only when an interval bound proves the aligned velocity remains non-positive and the reachable set cannot enter the envelope before the horizon.

The whole-phase broad phase applies these tests once per pocket. A rejection is therefore a mathematical consequence of the existing predicates, not a replacement predicate.

### 4. Bracket candidate windows without changing analytic hits

Keep the current analytic candidates before fallback and preserve their exact ordering semantics:

1. form the array in fixed capture-circle, mouth-plane, back-plane order;
2. apply the current finite/positive filtering;
3. use the same stable chronological sort, so equal-time candidates retain that type order.

Validate each with the exact current gap and retain the existing `[0, min(horizon, candidate + capture_tolerance)]` refiner interval and 60 iterations. This is required for the between-old-samples tunneling regression; neither the broad phase nor window construction may bypass a validating analytic candidate.

If no analytic candidate validates, derive conservative time windows in which the trajectory can overlap the capture envelope. Split windows at:

- exact longitudinal/lateral projection roots and extrema for quadratic motion;
- the existing analytic candidate times;
- the start/end of a curved-motion enclosure segment.

For curved segments, use conservative reachability intervals; do not treat the current quadratic mouth/back helper as exact for a curved trajectory. Window construction is only an acceleration structure: an uncertain root, interval, or mapping marks the containing time range unknown rather than discarding it. No cell may be omitted merely because it contains no analytic root.

Map each surviving closed time window outward to every legacy cell `[t_i, t_{i+1}]` it intersects, where each endpoint is computed with the exact legacy expression `horizon * i as f64 / 512.0`. Round the window-to-index conversion outward and include both cells sharing a boundary that is within the arithmetic error allowance. Tests must cover windows and accepted transitions exactly on, one ULP below, and one ULP above `t_0`, interior grid points, and `t_512`; an empty-width surviving window still retains its intersecting cell(s).

### 5. Search grid-aligned blocks adaptively, in chronological order

Represent a node as a contiguous legacy cell range `[first_cell, past_last_cell)`. Search windows by earliest cell, then each node's left child first. Deduplicate overlapping/outward-mapped windows without changing this chronological order.

- If the interval proof shows one necessary predicate is strictly greater than `capture_tolerance` over the node's entire **closed** time extent, prune the node without exact gap samples. Touching an expanded envelope boundary is overlap and cannot be pruned.
- Otherwise split the cell range in half.
- At a one-cell leaf, evaluate the exact two legacy endpoints required by the old transition check. Reuse a cached endpoint value when adjacent leaves are visited, keyed by the integer lattice index rather than recomputed time.
- On the first leaf satisfying the old `previous_gap > capture_tolerance && gap <= capture_tolerance` condition, call the unchanged 60-iteration refiner with those exact endpoints.
- If an enclosure or sample-window decision is unknown, descend. The worst case visits all 512 legacy cells, with the same initial plus 512 endpoint gap values as today; there is no depth cap and no approximate empty result.

This construction proves two properties:

1. every block skipped by the new search contains no time at which the current necessary capture predicates can all pass within tolerance; and
2. every unproved block eventually executes the same endpoint transition and refiner as the legacy scanner.

Thus the optimization removes unconditional expensive gap calls while preserving the legacy fallback’s first detected bracket and output bits. It does not claim that the legacy 512-cell semantics detect every mathematically possible sub-cell excursion; the existing analytic candidates remain responsible for the known high-speed tunneling case. Expanding continuous-time physics coverage would be a separate model change.

### 6. Release cutover

Replace `scan_ball_pocket_capture_time_during_current_phase_raw` with the adaptive implementation after all differential gates pass. Keep the legacy whole-horizon scanner only in tests/debug assertions for one release cycle. Do not leave two production modes, a runtime flag, or a compatibility wrapper.

## Benchmark plan

### Committed predictor evidence and missing branch cells

Keep and strengthen the committed `pocket_predictors` cells:

- `capture/slow_side_30deg_hit` (quick point about 9.45 ms/call);
- `capture/slow_side_30deg_target_miss` (about 11.61 ms/call);
- `capture/fast_side_analytic_hit`;
- `jaw/fast_side_hit`; and
- `scheduler/one_ball_slow_side_capture`.

Before timing, replace the hit/miss-only preflight with an exact complete expected `Option<PredictedBallPocketCapture>` and assert test/debug branch counters. Do not duplicate these cells under a new singular `pocket_predictor` group.

Add the following missing cells to the same `pocket_predictors` group:

- `capture/no_pockets`: `TableSpec::three_cushion_carom()`, proving the dispatch floor;
- `capture/far_miss`: default pool table, center-table rolling ball directed away from all pockets;
- `capture/gate_miss_60deg_80ips`: the rejected steep side-pocket fixture from `tests/n_ball_pockets.rs:940-969`;
- `capture/scan_fallback`: a frozen fixture proven by the instrumented setup to reach the fallback rather than accept an analytic candidate;
- `capture/fast_between_samples_hit`: `tests/n_ball_pockets.rs:845-862`;
- `capture/fast_between_samples_miss`: the offset fast straight entry from `tests/n_ball_pockets.rs:894-908`.

Construct `TableSpec`, ball, motion config, and states outside `b.iter`. Inside `b.iter`, `black_box` every input and the complete returned `Option`, not only the time or `is_some()`.

### Missing scaling group

Add `pocket_predictor_batch/capture/{1,4,16,64}`. Each prebuilt batch repeats a deterministic mix of far misses, gate misses, the committed analytic hits, proven fallback cases, straight motion, and curved rolling motion in the same proportions. The timed closure must evaluate every state and black-box the complete ordered output vector. Report ns/query and fit latency against batch size. Fixture construction, expected outputs, and branch classification stay outside timing.

This scaling group is not a substitute for branch cells. It checks that the per-query win persists and that no hidden setup/cache cost is merely amortized at one size.

### Existing adjacent/end-to-end filters

Record and compare:

- `core_functions/compute_next_ball_rail_impact_on_table`;
- `end_to_end/direct/pocket_aware_until_rest_cached`;
- `end_to_end/direct/pocket_aware_until_rest_manual`;
- `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8`.

The rail filter is an adjacent-path guard. The two simulation implementations must produce the same complete outputs outside Criterion before their timings are compared.

## Correctness and differential tests

### Predictor oracle

For every fixture below, require exact equality of the complete public `Option<PredictedBallPocketCapture>` between the unchanged predictor and the cutover predictor. For fixtures proven to reach fallback, also invoke the legacy and adaptive raw searches from the identical state/phase/pocket/horizon context and require exact equality of their `Option<Seconds>`:

- all existing slow/fast side and corner target accept/reject tests in `tests/n_ball_pockets.rs:780-1013` and `:1330-1495`;
- fast capture before the first old sample (`:845-862`);
- mouth-plane ordering (`:864-892`), back-plane edges, immediate capture, and no capture;
- all six mirrored pockets;
- sliding, straight rolling, side-spin curved rolling, spinning/rest, and phase horizons immediately above `f64::EPSILON`;
- default table, three-cushion no-pocket table, and custom pocket width/depth/table scale;
- analytic hit, fallback hit, fallback miss, tangent/grazing, target critical-angle boundaries, times within one ULP of a legacy grid endpoint, and trajectories separated from every envelope face immediately below/at/above the exact horizon-scaled `capture_tolerance`.

Add a deterministic parameter lattice over pocket, phase, speed `[0.1, 5, 60, 80, 200]` ips, signed angle around every critical/max gate, lateral offsets around each target boundary, side spin signs, and horizon/grid-boundary offsets. Add a fixed-seed randomized legal-state corpus as supplementary coverage. Random sampling is not the proof; the outward interval invariants are.

Tests for each pruned node must assert, through the debug oracle, that the legacy scanner finds no accepted endpoint transition inside it. Tests for the envelope arithmetic must assert enclosure of directly evaluated predicate values at endpoints, extrema, and dense diagnostic samples.

### Scheduler and simulation oracle

Run the old and new capture predictors through complete pocket-aware simulations and require `assert_eq!` on the entire `NBallSystemSimulation`: elapsed time, ordered event variants, source indices/pairs, pocket/rail identity, prediction payloads, and final states. Include:

- the existing cached-versus-manual capture and shared-contact fixtures (`tests/n_ball_pockets.rs:1886-1993`);
- the three-ball/eight-event pinball fixture;
- a capture tied within `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS` of jaw, rail, transition, and ball-ball candidates;
- early capture with another moving ball, curved jaw/capture scheduling, late drop after jaw impact, and zero-time contact cascades.

No tolerance-only comparison is accepted for the event sequence or final state. If a different adaptive bracket changes the public payload or a tie winner, retain the legacy grid-aligned leaf behavior rather than loosening the test.

## Statistical acceptance gates

Use the same release profile, Rust toolchain, host, power mode, and Criterion configuration before and after. Save a baseline from the unchanged revision. For every required cell, run at least three paired before/after processes on an otherwise idle host. Use at least 30 samples and 10 seconds measurement for direct microbenchmarks; use at least 20 samples for slow end-to-end cells. Report Criterion’s 95% bootstrap confidence interval for relative change, outliers, and fitted batch-size slopes.

Ship only if all of the following hold:

- `pocket_predictors/capture/far_miss` and `/gate_miss_60deg_80ips`: relative-time CI wholly below zero in all three paired runs and pooled median improvement at least 25%;
- `pocket_predictor_batch/capture/64`: CI wholly below zero and median improvement at least 15%, with no worse fitted ns/query slope;
- committed `pocket_predictors/capture/slow_side_30deg_hit` and `/fast_side_analytic_hit`, plus missing `/fast_between_samples_hit` and `/scan_fallback`: no 95% CI indicating a regression greater than 3%;
- at least one of `end_to_end/dsl/preparsed_three_ball_pinball_event_limit_8` or `end_to_end/direct/pocket_aware_until_rest_cached` improves by at least 3% with a wholly negative CI, and neither cached nor manual endpoint regresses by more than 3%;
- `core_functions/compute_next_ball_rail_impact_on_table` does not regress by more than 3%;
- profile/counter corroboration on far/gate misses shows at least 80% fewer exact gap evaluations and no fixed 512-evaluation stack as the common path. Timing without this work-count proof is insufficient.

Re-run noisy or discordant cells. Do not claim a win from one process, a point estimate whose CI crosses zero, or only the no-pocket dispatch floor.

## Risks, rejection criteria, and rollback

- **Unsound envelope:** any oracle case where a pruned block contains a legacy accepted transition is an immediate rejection. Fix the bound; never add a fixture exception.
- **Curved motion under-enclosure:** if a reviewable path-length/interval bound cannot be established, mark curved nodes unknown and descend to legacy leaves. Do not use the quadratic plane roots as proof for curved motion.
- **Output drift:** any complete predictor or simulation inequality rejects the cutover. Do not widen event tolerances.
- **Worst-case overhead:** if fallback/analytic hits regress by more than 3% or miss-path practical thresholds are not met, remove the adaptive implementation and keep only benchmark coverage.
- **Code complexity:** if the interval proof requires a general-purpose solver larger than this narrow kernel, stop and retain the prepared-geometry/candidate wins. Complexity unsupported by measured pruning is not accepted.

Rollback is a single clean restoration of the current scan call; benchmark and equivalence fixtures remain. Do not retain a production feature flag or dual behavior.

## SIMD and GPU decision

arm64 SIMD is rejected for this candidate. There are only six heterogeneous pockets; the work contains divergent early exits, target interpolation, trigonometry, interval tree traversal, and scalar root/refinement dependencies. NEON batching would make outward rounding and first-event ordering harder to audit and is not supported by profile evidence. Reconsider SIMD only after the scalar algorithm lands and a profile identifies one wide homogeneous projection kernel.

GPU offload is rejected. Per query the data set is tiny, event/cache decisions are latency-sensitive and sequential, transfer/dispatch costs dominate, and deterministic scalar ordering must be preserved. No GPU implementation or claim belongs in this change.

## Candidate commit message

`perf(physics): adaptively bracket pocket capture search`
