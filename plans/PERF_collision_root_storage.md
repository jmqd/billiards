# Store collision polynomial roots and interval candidates inline

Date: 2026-07-11

Status: **Accepted candidate; implementation not started**
Priority: **P1**
Confidence: **High for allocation removal; medium for isolated latency gain**
Dependencies/order: **Independent.** The committed `collision_predictor_paths` group already supplies linear-hit, curved-hit control, parallel-miss, and near-grazing-miss baselines. Add the missing stationary floor and near-grazing hit before production edits. This plan may land before or after shared-contact scratch and collision-refinement-stall changes; after any predictor change, establish a fresh baseline rather than combining mechanisms in one performance claim.

## Objective

Replace heap-backed polynomial root vectors and collision interval-boundary vectors with fixed-capacity stack storage. Preserve the exact formulas, tolerances, ordering, deduplication, interval traversal, and returned event results. Use only crate-local Rust code and the standard library; add no dependency.

The bounded capacities follow from the existing mathematics:

- a quadratic has at most two real roots;
- a cubic has at most three real roots;
- the quadratic relative-motion collision search has at most five boundaries: `0`, `horizon`, and three interior derivative roots.

These are mathematical bounds, not workload assumptions, so no heap fallback is needed for this narrowly scoped root/candidate path.

## Current mechanism and evidence

### Source facts

The relevant implementation is in `src/lib.rs`:

- `RelativeQuadraticMotion::derivative_roots` returns `Vec<f64>` from the derivative cubic (`src/lib.rs:5676-5689`).
- `real_roots_quadratic` returns `Vec::new()` or a heap-backed one/two-element `vec!` (`src/lib.rs:5692-5715`).
- `real_roots_cubic` returns a heap-backed one/two-element `vec!`, collects three trigonometric roots into a `Vec`, or delegates to the quadratic helper (`src/lib.rs:5717-5754`).
- `first_ball_ball_contact_time_for_relative_motion` starts with `vec![0.0, horizon]`, pushes finite interior derivative roots, sorts, then allocates a second `Vec::with_capacity` to deduplicate boundaries (`src/lib.rs:5757-5782`). It traverses those boundaries in ascending order and bisects the first separated-to-contact interval (`src/lib.rs:5784-5815`).
- The phase-aware public predictor uses this polynomial path only when neither phase has active curved rolling (`src/lib.rs:6274-6321`). Active TP B.2 curved rolling instead uses `first_curved_entry_time_adaptive`; it is an adjacent correctness/performance guardrail, not evidence for this storage optimization.
- The same quadratic root helper is consumed by ordinary rail collision prediction (`src/lib.rs:6426-6447`), pocket mouth-plane crossing (`src/lib.rs:7845-7861`), pocket back-plane crossing (`src/lib.rs:7883-7899`), and side-pocket centerline crossing (`src/lib.rs:7972-7985`). Each sorts the tiny root vector before choosing the first acceptable time.

Existing observable benchmark coverage now includes:

- `collision_predictor_paths/linear_hit`;
- `collision_predictor_paths/curved_rolling_hit`;
- `collision_predictor_paths/parallel_miss`;
- `collision_predictor_paths/grazing_miss`;
- the older ordinary predictor point `core_functions/compute_next_ball_ball_collision_during_current_phases_on_table`;
- batch prediction at `throughput_functions/compute_next_ball_ball_collision_during_current_phases_on_table/{100,1000}`.

Direct hit/miss, phase horizon, grazing hit, curved hit/ghost, and rolling-stop contracts remain covered in `tests/ball_collision_timing.rs`; scheduler tie ordering and the canonical curved collision remain covered in `tests/n_ball_events.rs`.

The committed branch group still lacks a stationary early-return floor and a near-grazing **hit**. Its current `grazing_miss` uses an offset of `2R + 1e-4` and covers the near-tangent miss side only.

### Measured versus hypothesized evidence

Quick unchanged-tree Criterion ranges are:

- `collision_predictor_paths/linear_hit`: **6.488–6.498 us/call**;
- `collision_predictor_paths/curved_rolling_hit`: **1.0781–1.0789 ms/call**;
- `collision_predictor_paths/parallel_miss`: **6.754–6.770 us/call**;
- `collision_predictor_paths/grazing_miss`: **5.725–5.953 us/call**.

These are current-path scale measurements, not paired evidence of an optimization. The curved cell is explicitly a non-polynomial control.

Measured aggregate evidence in `plans/performance_engineering.md` reports 405,780 `alloc::raw_vec::finish_grow` calls on the profiled pipeline and identifies fixed-size root vectors as source-visible allocation sites. Assembly inspection also showed tiny analytic candidate allocation/sort/deallocation on the pocket path. Those are broad pipeline facts, not a measured collision-predictor attribution.

For the collision predictor, the following are source-proven:

- every non-immediate polynomial search constructs the initial two-boundary `Vec`;
- it constructs a second deduplicated-boundary `Vec`;
- any nonempty quadratic/cubic root result adds a root-vector allocation at source level.

Whether optimized code retains every source-level allocation and whether removing it materially lowers public latency remain hypotheses until allocation attribution and paired branch benchmarks pass. Exact-unit conversion and output construction may dilute the allocator win.

## Scope

### In scope

1. One private fixed-capacity `f64` buffer for up to three polynomial roots.
2. One private fixed-capacity `f64` buffer for up to five collision interval boundaries.
3. In-place sort and in-place tolerance deduplication over initialized slices.
4. Migration of every `real_roots_quadratic`/`real_roots_cubic` caller in `src/lib.rs`, so the helpers do not retain a parallel `Vec` API.
5. Public-API collision-predictor benchmarks that isolate polynomial hit/miss/degenerate strata, plus existing rail and curved-path guardrails.
6. Root multiplicity/order tests, end-to-end numerical equivalence, and allocation attribution.

### Non-goals

- Do not change quadratic or cubic formulas, coefficient scaling, discriminant calculation, `powi`/`powf`/`cbrt`/trigonometric calls, or root multiplicity.
- Do not change the `1e-12` coefficient/discriminant threshold, boundary filter, `1e-10` dedup tolerance, gap/derivative tolerances, horizon, or 80-step refinement loop.
- Do not change event semantics, tangent/closing rules, simultaneous-event policy, or candidate priority.
- Do not merge this work with refinement-stall termination, raw motion classification, adaptive curved search, pocket-capture algorithm changes, scheduler redesign, or solver scratch.
- Do not make root helpers public solely for Criterion.
- Do not add `smallvec`, `arrayvec`, SIMD crates, allocator crates, or any other dependency.
- Do not claim that the active curved-rolling collision fixture exercises polynomial roots; it explicitly does not in current source.

## Proposed private representation

Use a concrete, copyable private buffer rather than a general collection abstraction:

```rust
#[derive(Clone, Copy, Debug)]
struct FixedF64Candidates<const N: usize> {
    values: [f64; N],
    len: u8,
}

type PolynomialRoots = FixedF64Candidates<3>;
type RelativeMotionBoundaries = FixedF64Candidates<5>;
```

Required methods:

```rust
fn new() -> Self;
fn push(&mut self, value: f64);
fn as_slice(&self) -> &[f64];
fn as_mut_slice(&mut self) -> &mut [f64];
fn sort_by(&mut self, compare: impl FnMut(&f64, &f64) -> Ordering);
fn dedup_by_tolerance(&mut self, tolerance: f64);
```

Invariants:

- `len <= N`; `push` uses a `debug_assert!` and an unconditional bounds-safe write. Capacity overflow is a programming error because degree supplies the proof.
- Only `values[..len]` is observable. Initialize unused slots to `0.0`; do not use `unsafe` or `MaybeUninit`.
- Root helpers preserve their current formula emission order and multiplicity. Sorting occurs at the same consumer points as today.
- Sorting retains the existing `partial_cmp(...).expect(...)` behavior. Do not replace it with `total_cmp`, silently discard NaNs, or otherwise change invalid-input behavior.
- Deduplication retains the first value in ascending order and compares each candidate with the most recently retained value, matching the current loop exactly.
- No iterator implementation may allocate. Slice iteration is sufficient.

A single capacity-three `PolynomialRoots` avoids conversion when cubic degeneration delegates to `real_roots_quadratic`. The quadratic helper uses only its first two slots. Do not introduce separate root enums or a dependency for this small fixed shape.

## Implementation sequence and exact cutover

1. **Complete the public benchmark matrix and save the unchanged baseline.** Keep the committed branch fixtures, add only the missing cells below, and validate every fixture's branch-relevant outcome outside the timed loop.
2. **Add `FixedF64Candidates` privately near `RelativeQuadraticMotion`.** Add unit tests for zero-length, full-capacity slice exposure, sorting, and in-place dedup. Keep the type purpose-specific; do not move it into a public utilities module.
3. **Change `real_roots_quadratic` to return `PolynomialRoots`.** Emit zero, one, or two values using `push` in the exact current branches and expression order. The two-root branch must push `(-b - sqrt_discriminant)/(2a)` before `(-b + sqrt_discriminant)/(2a)`, exactly as now.
4. **Change `real_roots_cubic` to return `PolynomialRoots`.** Preserve:
   - degenerate delegation to the quadratic helper;
   - the current single-root expression and order;
   - the two emitted values for near-zero discriminant, including a repeated root if the formulas produce one;
   - the `index = 0..3` trigonometric generation order for three roots.
   Do not deduplicate algebraic multiplicity in the root helper.
5. **Migrate `RelativeQuadraticMotion::derivative_roots`.** It returns `PolynomialRoots` without an intermediate conversion.
6. **Flatten collision boundaries.** In `first_ball_ball_contact_time_for_relative_motion`:
   - create `RelativeMotionBoundaries::new()`;
   - push `0.0`, `horizon`, then each derivative root that passes the existing finite/interior filter;
   - sort the initialized slice with the current finite-time comparator;
   - compact the same buffer in place using `1e-10 * horizon.max(1.0)` and the existing `>` test;
   - traverse the retained slice from index 1 while carrying the same `left` and `left_gap` values.
   Do not create a second candidate buffer and do not change which duplicate survives.
7. **Migrate every quadratic consumer.** Ordinary rail, pocket mouth-plane, pocket back-plane, and side-pocket centerline functions continue to sort the initialized root slice with their existing comparator/error text and then iterate it in ascending order. Remove only the heap allocation and by-value `Vec` iteration.
8. **Delete the old vector-returning paths.** There is one root representation and no compatibility wrapper. The candidate commit migrates all in-tree call sites together.
9. **Inspect optimized arm64 assembly.** Confirm that the fixed buffers remain stack/scalar operations and that no allocator call remains reachable from `real_roots_quadratic`, `real_roots_cubic`, or polynomial boundary construction. Assembly inspection corroborates mechanism; Criterion decides value.

## Deterministic ordering contract

Ordering matters because consumers accept the first root/boundary satisfying their geometry and horizon checks.

The candidate must preserve these rules:

1. Root formulas emit in current source order.
2. Consumers sort ascending with `partial_cmp` and panic on a non-comparable value exactly as today.
3. Collision boundaries include endpoints before roots are appended, but sorting determines traversal.
4. Dedup uses `abs(candidate - previous_retained) > tolerance`; values inside or exactly at the tolerance are discarded in favor of the earlier sorted value.
5. The collision search examines intervals from earliest to latest and returns the right/contact-side endpoint from the same bisection helper.
6. Rail and pocket consumers continue to return the earliest acceptable clamped root.

Do not rely on stable sort to distinguish equal `f64` values: equal candidates are numerically identical for subsequent logic, and the dedup rule retains the first sorted value. Preserve the current comparator rather than introducing a new tie key.

## Benchmark plan

### Existing committed public-API fixtures

The committed `collision_predictor_paths` group already constructs and validates inputs outside `b.iter`, uses eight-second measurement time and 30 samples, black-boxes input references, and black-boxes the complete `Option<PredictedBallBallCollision>`:

1. `collision_predictor_paths/linear_hit`
   - Uses `collision_predictor_states()`: a rolling cue ball starts `2R + 7.5` inches behind a resting object at 10 in/s under the current motion config.
   - Asserts `Some`. This is the primary ordinary polynomial hit cell.

2. `collision_predictor_paths/parallel_miss`
   - Uses separated rolling balls with equal planar velocity and matching no-slip spin.
   - Asserts `None`. Relative velocity and acceleration degenerate; the cell exercises endpoint-boundary construction but may return an empty root buffer.

3. `collision_predictor_paths/grazing_miss`
   - Uses the same ordinary rolling direction with transverse separation `2R + 1e-4`.
   - Asserts `None`. This is the committed near-tangent polynomial miss cell.

4. `collision_predictor_paths/curved_rolling_hit`
   - Uses the canonical nonzero-`wz` curved fixture.
   - Asserts `Some`, but executes `first_curved_entry_time_adaptive`, not polynomial root storage. It is a regression control and must not be counted as an expected root-storage win.

Do not rename these IDs or create duplicate aliases. The quick ranges above are historical scale evidence only; save a fresh paired baseline on the implementation host.

### Missing cells to add with this candidate

Add to the same `collision_predictor_paths` group:

1. `stationary_miss`
   - Two separated resting balls.
   - Assert `None`.
   - This is an early-path floor; it may not enter polynomial boundary construction and is a regression guardrail, not a primary win cell.

2. `grazing_hit`
   - Reuse the no-side-spin fixture from `tests/ball_collision_timing.rs`: rolling speed 120 in/s, deceleration 5 in/s², object at `x = 60`, `y = 2R - 0.0001`.
   - Assert `Some`, the analytic expected impact time already used by the test, touching geometry, and closing motion.
   - This covers the near-tangent hit side missing from the committed matrix.

All states, ball specs, and configs must remain outside `b.iter`; keep each branch separate rather than averaging a distribution.

### Existing adjacent filters

Keep and compare:

- `core_functions/compute_next_ball_ball_collision_during_current_phases_on_table`
- `throughput_functions/compute_next_ball_ball_collision_during_current_phases_on_table/1000`
- `core_functions/compute_next_ball_rail_impact_on_table`

The rail point guards a migrated quadratic consumer. Pocket consumers require correctness tests and allocation/assembly corroboration here; their broader latency optimization belongs to the separate pocket plans, so do not add an overlapping pocket algorithm benchmark solely to make this candidate look larger.

## Allocation gates

Measure allocations separately from Criterion using a dependency-free counting allocator around `std::alloc::System` in a dedicated test/benchmark binary, or macOS Instruments Allocations with stack attribution. Prebuild fixtures, warm once, reset counters, run 10,000 public predictor calls, and black-box complete results. Never include fixture construction or baseline serialization.

Required gates:

- Zero allocations attributable to `real_roots_quadratic`, `real_roots_cubic`, `RelativeQuadraticMotion::derivative_roots`, collision boundary construction, sorting, or deduplication.
- `parallel_miss`: at least **two fewer allocations per public call** than baseline, corresponding to the initial boundary vector and deduplicated boundary vector; do not require a root-vector delta when the degenerate helper returns `Vec::new()`.
- `linear_hit`, `grazing_hit`, and `grazing_miss`: at least **three fewer allocations per public call** only when unchanged-code allocation attribution confirms that fixture produced a nonempty heap-backed root result.
- Migrated rail and pocket root helpers show no root-vector allocation stack in the profiler.

Report total public-call allocations as well as attributed deltas. Exact-unit conversion and `Some` output construction may still allocate; those are not in scope and must not be hidden by claiming total zero allocation.

## Correctness and equivalence verification

### Root-level permanent tests

Add private unit tests beside the helpers for coefficient sets with known results:

1. Quadratic no-root, repeated-root, and two-root cases.
2. A negative leading coefficient, proving consumer sorting rather than formula emission order determines ascending traversal.
3. Cubic one-real-root, near-zero-discriminant two-emission, and three-real-root cases.
4. Cubic degeneration to quadratic with identical length, values, and formula order.
5. Boundary sort/dedup cases with roots outside the horizon, roots equal to `0`/`horizon`, and two roots separated by less than, exactly, and greater than `1e-10 * horizon.max(1.0)`.
6. Full capacity: three cubic roots plus both endpoints produces five candidates without overflow.

Compare root values with the test tolerance already used for these formulas, but also compare emitted length and post-sort order exactly. Tests must fail if multiplicity is accidentally collapsed, the capacity is wrong, or dedup retains the later value.

### Public behavior tests

Retain and run:

- `tests/ball_collision_timing.rs`, especially direct hit/miss, immediate contact, near-grazing hit, curved hit/ghost, stop-before-contact, and no-contact-at-stop cases;
- `tests/n_ball_events.rs` for earliest-pair, simultaneous-pair tie, shared-contact ordering, and curved scheduling;
- `tests/rail_event_scheduling.rs` and `tests/n_ball_pockets.rs` for the migrated quadratic consumers.

Add the near-grazing miss as a permanent integration test. Add a deterministic repeated-call test that compares the public `Option` shape, event time, and all impact-state fields for every benchmark fixture.

### Baseline differential gate

Before implementation, serialize complete public results from the committed linear-hit, parallel-miss, and grazing-miss fixtures; the added grazing-hit and stationary floor; the curved control; representative ordinary rail impact; pocket mouth/back crossing scenarios already covered by tests; and side-pocket centerline crossing. Record `Option`/event discriminants and `f64::to_bits()` for time, positions, velocities, vertical velocity, and angular velocity.

On the same host/toolchain/configuration, candidate outputs must be **bitwise identical**. The optimization changes storage only and need not reassociate arithmetic. A bit difference indicates changed root formula, ordering, deduplication, or traversal and rejects the change. Platform-independent checked-in tests may continue to use established tolerances; the paired baseline artifact is the strict implementation gate.

## Statistical acceptance

Save a Criterion baseline from the unchanged code. Use the same release profile, Rust toolchain, feature set, host, power/thermal state, fixture data, and Criterion settings before and after. Run at least **three paired process-level comparisons**, interleaving baseline and candidate. Inspect outliers and rerun noisy or directionally inconsistent cells.

Use Criterion's bootstrap 95% confidence interval for the after/before time ratio:

- `collision_predictor_paths/linear_hit`: upper bound `<= 0.95`.
- added `collision_predictor_paths/grazing_hit`: upper bound `<= 0.95`.
- `collision_predictor_paths/grazing_miss`: upper bound `<= 0.95`.
- `throughput_functions/.../1000`: whole-workload guardrail upper bound `<= 1.03`.
- `stationary_miss`, `parallel_miss`, `curved_rolling_hit`, and the existing rail point: no regression greater than 3%; ratio upper bound `<= 1.03`.

The practical microbenchmark win is at least 5% on the three attributed root-using point cells. Allocation gates and exact equivalence are mandatory even if timings pass. Throughput is diagnostic for upside but must exclude a regression beyond the 3% guardrail; it is not a mandatory 3% win because the batch mixes branch shapes and other predictor costs.

## Risks and mitigations

- **Capacity error.** Degree bounds are explicit: three polynomial roots and five boundaries. Unit-test full capacity and keep bounds assertions.
- **Changed root order.** Preserve formula emission, consumer sort comparator, and earliest-first traversal; require bitwise public outputs.
- **Changed multiplicity.** Do not dedup inside root solvers. Only collision boundaries use the existing tolerance dedup.
- **NaN behavior accidentally hidden.** Keep `partial_cmp(...).expect(...)` at current consumer sorts; do not switch to `total_cmp` or filter before a sort that currently panics.
- **Stack copies erase the win.** Keep buffers `Copy`-small (three/five `f64`s plus length), pass mutable references within consumers, and inspect optimized assembly for avoidable whole-array copies.
- **Compiler already scalar-replaces vectors.** Allocator calls in source/assembly and allocation counts decide this. If optimized code has no allocation or public timing does not improve, reject rather than add complexity.
- **Common helper migration affects pockets/rails.** Migrate all call sites atomically, retain their exact filtering/clamping rules, and run focused suites.
- **Benchmark mislabeled as polynomial.** Keep nonzero-`wz` curved fixture explicitly as a control; do not use it as primary evidence.

## Stop criteria, rejection, and rollback

Reject or revert the implementation while keeping useful benchmark/tests if any of these occurs:

1. Any baseline public result differs bitwise on the paired host/toolchain, or any event/tie/root acceptance changes semantically.
2. Root length, multiplicity, sort order, boundary order, or tolerance dedup differs from current behavior.
3. Any allocator stack remains under the polynomial roots or boundary candidate path.
4. The attributed root-using point cells fail the `<= 0.95` upper-confidence-bound gate after three paired runs.
5. Throughput, stationary/parallel fast paths, curved control, rail prediction, or pocket correctness regresses beyond the stated guardrail.
6. The implementation requires a dependency, unsafe uninitialized storage, changed tolerances, approximate roots, or altered refinement/event semantics.
7. Optimized assembly shows array copies/spills that eliminate the intended mechanism and no public latency win.

Rollback restores the private vector-returning helpers and boundary vectors. No public API, serialized format, or persisted data changes, so rollback requires no compatibility shim or migration.

## arm64 SIMD and GPU decision

**Manual arm64 SIMD: reject.** The workload contains at most three scalar roots and five candidates with divergent discriminant branches, transcendental scalar functions in the three-root cubic, and earliest-root control flow. NEON setup and lane extraction would exceed useful work, while vectorized sorting/reduction could change operation order. Fixed arrays may improve scalar register allocation; rely on LLVM and verify assembly. Reconsider SIMD only for a demonstrated SoA batch API with many independent polynomials of the same degree and branch distribution, which is outside current event-driven simulation.

**GPU: reject.** A handful of roots are computed synchronously inside a latency-sensitive, sequential event search. Dispatch, transfer, synchronization, and divergent polynomial branches dominate any arithmetic. No GPU dependency, shader, or batching interface should be introduced.

## Candidate commit message

`perf(physics): store collision roots and boundaries inline`
