# Spin-aware rail scalar-invariant plan

**Status:** Accepted only for benchmark-first investigation; production implementation remains explicitly rejection-gated on setup-controlled direct timing and shipped-configuration assembly evidence.

**Priority:** Medium. The kernel is expensive and directly observable, but the proposed win is deliberately lower confidence than algorithmic hot-path work because release LLVM may already hoist the loop invariants.

**Confidence:** Medium for reducing duplicated post-solve work; low-to-medium for improving the impulse loop itself until optimized arm64 output proves that invariant arithmetic remains in the loop.

**Dependencies and order:** The committed 216-impact direct group supplies an initial current-tree scale measurement, but it clones the owned exact-unit radius inside the timed loop and has no scalar point. Correct that attribution issue, add the missing scalar and SpinAware trace guard, save paired baselines, and inspect unchanged optimized output before any production edit. Do not combine this work with a rail-model change or another performance candidate.

## Goal

Reduce scalar work in `RailModel::SpinAware` rail resolution without changing the impulse discretization, friction or restitution equations, branch boundaries, floating-point operation order, output bits, or event behavior.

This is a benchmark-gated micro-optimization, not a mandate to add a context type. If LLVM already performs the useful loop-invariant motion, or if three paired measurements cannot distinguish the candidate from noise at the thresholds below, retain the benchmark/equivalence coverage and reject the production refactor.

## Current source map and evidence

### Exact production call chain

The direct straight-rail path is:

```text
collide_ball_rail_on_table_with_radius_and_profile
  src/lib.rs:13949-13983
  -> profile.for_rail(rail)
  -> RailModel::SpinAware
  -> spin_aware_ball_rail_collision_on_table
     src/lib.rs:13847-13869
  -> rail_collision_basis(rail)
  -> spin_aware_ball_cushion_collision_on_table_from_basis
     src/lib.rs:13718-13845
  -> project the incoming state into the local cushion frame
  -> solve_spin_aware_rail_impact_in_frame
     src/lib.rs:13474-13511
  -> solve_rail_impact_compression_phase
     src/lib.rs:13309-13386
  -> advance_rail_impact_frame_by_impulse_step
     src/lib.rs:13264-13298
  -> rail_impact_frame_slip_directions
     src/lib.rs:13236-13262
  -> rail_impact_contact_slip_direction twice
     src/lib.rs:13216-13234
  -> solve_rail_impact_restitution_phase
     src/lib.rs:13388-13472
  -> the same step/slip chain
  -> reconstruct world velocity and spin
  -> apply entry-dependent post-solve guardrails
  -> build_on_table_ball_state
```

The public wrapper chain remains part of the observable surface and must not change:

- `collide_ball_rail_on_table_with_radius_and_config` at `src/lib.rs:13987-14001` constructs a uniform profile and calls the profile API.
- `collide_ball_rail_on_table_with_radius` at `src/lib.rs:14008-14021` supplies the default profile.
- `collide_ball_rail_on_table` at `src/lib.rs:14028-14034` supplies the default radius and profile.

The profile API is used by three real event/trace executors, not only by tests:

- `advance_to_next_n_ball_event_with_scheduler`, straight-rail event arm at `src/lib.rs:10600-10610`;
- `resolve_n_ball_system_event_with_physics_and_pockets_on_table`, straight-rail event arm at `src/lib.rs:11461-11469`;
- single-ball path tracing, rail-impact arm at `src/lib.rs:11945-11972`.

The same local-basis resolver also handles pocket jaws through `collide_ball_jaw_on_table_with_radius_and_profile` at `src/lib.rs:13871-13921`, whose SpinAware arm calls `spin_aware_ball_cushion_collision_on_table_from_basis` at lines 13909-13920. Any internal signature cutover must migrate both the straight-rail and jaw call sites; do not leave duplicate coefficient-building conventions.

### Current repeated work

Measured facts from the current tree:

- The committed `rail_resolution/spin_aware_216_impacts` quick range is **11.657–11.933 ms per 216-impact batch**, or approximately **18.10–18.53 thousand impacts/s**.
- The identical-loop control ranges are **402.0–416.2 us/batch** for `restitution_only_control_216_impacts` and **329.0–335.5 us/batch** for `mirror_control_216_impacts`.
- These direct fixtures establish that SpinAware resolution dominates the public call on this matrix, but they are not clean solver baselines because each timed call clones the owned `Inches` radius.
- There is still no committed scalar direct-resolution point.
- `core_functions/compute_next_ball_rail_impact_on_table` benchmarks rail-impact **prediction**, which does not execute the response solver.
- The existing one-second bank trace uses `RailModel::Mirror`, so it also does not measure SpinAware resolution.
- `benches/throughput.rs` uses SpinAware for whole single-ball traces, but motion prediction and trace construction obscure the resolution cost.
- The DSL/preparsed three-ball path uses SpinAware and is only an end-to-end guardrail.
Source-grounded facts in the solver:

- `RAIL_IMPACT_NOMINAL_IMPULSE_STEPS` is 1,000 and `RAIL_IMPACT_ROOT_REFINEMENT_STEPS` is 48 (`src/lib.rs:13179-13181`). Both compression and restitution repeatedly call the scalar step function; the terminal partial step invokes it in each refinement iteration.
- Every step computes `5.0 / (2.0 * ball_radius)` at `src/lib.rs:13280` and receives the same radius, `sin_theta`, `cos_theta`, table friction, and cushion friction for the entire impact.
- Every step computes two state-dependent slip norms and normalizations through `hypot` and division at `src/lib.rs:13216-13233`. Those operations are not invariant and are out of scope.
- `sin_theta` and `cos_theta` are already computed once in `solve_spin_aware_rail_impact_in_frame` at `src/lib.rs:13485-13486`; claiming their helper functions as a loop win would be incorrect.
- The expressions at `src/lib.rs:13245-13253` include left-associated products such as `angular_normal * ball_radius * sin_theta`. Precomputing `ball_radius * sin_theta` would change binary64 grouping and is therefore not permitted by this plan.
- After the solve, incoming speed, rolling proximity, side-spin ratio, and incoming cloth slip are recalculated across `rail_rolling_proximity`, `rail_running_english_generation_scale`, `rail_rebound_horizontal_spin_blend`, `rail_rebound_outgoing_cloth_slip_ratio_limit`, and `clamp_rail_rebound_horizontal_spin_to_slip_limit` (`src/lib.rs:13546-13607` and `13666-13716`). The call sites are at `src/lib.rs:13790-13792`, `13811`, and `13830-13837`. This duplication is visible in source, but release-code duplication still must be confirmed before editing.

Existing correctness coverage is substantial and must remain green:

- `tests/rail_collisions.rs:107-130` covers frictionless restitution across restitution and speed values.
- `tests/rail_collisions.rs:245-315` covers cushion-friction strength and partial slip.
- `tests/rail_collisions.rs:317-493` covers exact and near-zero cushion/cloth slip continuity.
- `tests/rail_collisions.rs:495-607` covers topspin, rolling-versus-sliding entry, and carried side spin.
- `tests/rail_collisions.rs:609-652` covers the high-side-spin Mathavan qualitative case.
- `tests/rail_collisions.rs:673-780` covers outgoing cloth-slip and vertical-plane-spin guardrails.
- `tests/rail_event_execution.rs:178-235` verifies event execution uses the configured SpinAware response; lines 237-283 protect zero-restitution zero-time behavior.
- `tests/lag_calibration.rs:172-216` protects the tuned rolling-lag and settled-speed behavior.

The quick direct batch proves workload scale but not a candidate gain. No unchanged-code arm64 instruction count has yet established removable dynamic work, and the in-loop owned-radius clone must be excluded before a percentage is attributed to the solver. Any statement that a coefficient context is faster remains a hypothesis until the corrected benchmark and disassembly stages below pass.

## Scope

### In scope

1. Add isolated direct SpinAware rail-resolution latency and batch benchmarks.
2. Add a deterministic fixture constructor and before/after bitwise output gate.
3. Inspect optimized Apple arm64 output for the unchanged and candidate implementations.
4. If and only if the unchanged output contains repeated invariant work, introduce a small internal `Copy` coefficient context for exactly those values.
5. Consolidate source-proven, still-present post-solve recomputation into one `Copy` entry-metrics value if isolated measurements attribute a meaningful win to it.
6. Preserve all public APIs and migrate both straight-rail and jaw internal callers in one cutover.

### Explicit non-goals

- Do not reduce the nominal 1,000 impulse steps or the 48 refinement iterations.
- Do not add a convergence tolerance, early exit, approximate root, lookup table, or alternative integrator.
- Do not change normal restitution, cushion friction, cloth friction, adherence scaling, TP 7.3 geometry, rolling blend, outgoing cloth-slip limits, or their constants.
- Do not reorder, reassociate, fuse, or otherwise alter floating-point expressions. Do not enable fast-math.
- Do not replace exact-unit public inputs or outputs.
- Do not optimize rail-impact prediction; `compute_next_ball_rail_impact_on_table` is an adjacent but different operation.
- Do not introduce a SIMD batch API, GPU path, parallel scheduler, or structure-of-arrays layout.
- Do not add caches with lifetime beyond one impact. Coefficients are impact-local and stack-resident.

## Stage 1: correct and complete direct benchmarks before production edits

### What is already committed

`benches/physics.rs::bench_rail_resolution` is registered in the Criterion group as `rail_resolution`, with ten-second measurement time, 30 samples, and `Throughput::Elements(216)`. `rail_resolution_matrix` constructs this exact ordered Cartesian product:

| Axis | Values | Count |
| --- | --- | ---: |
| Rail | `Left`, `Right`, `Bottom`, `Top` | 4 |
| Total planar speed | 10, 60, 120 in/s | 3 |
| Tangent/normal ratio | 0.0, 0.5, 1.0 | 3 |
| Vertical-spin factor | -1.0, 0.0, 1.0 | 3 |
| Horizontal-spin entry | sliding, exact cloth rolling | 2 |

For each total speed $v$ and ratio $q$, the constructor uses `normal_speed = v / sqrt(1 + q²)` and `tangent_speed = q * normal_speed`; 10/60/120 are therefore **not** normal-speed values. It maps those local components to world velocity with the explicit per-rail match, uses `wx = -vy/R, wy = vx/R` for rolling and zero horizontal spin for sliding, and sets `wz = vertical_spin_factor * v / R`. The current factors are not labeled or asserted as “running” and “reverse,” and they are not the previously proposed normalized ±0.5 side-spin cases. Preserve this exact fixture definition for the paired baseline rather than silently substituting a different matrix.

The committed IDs are:

- `rail_resolution/spin_aware_216_impacts`
- `rail_resolution/restitution_only_control_216_impacts`
- `rail_resolution/mirror_control_216_impacts`

The group uses `RailCollisionProfile::default()`, not an explicitly constructed `human_tuned()` profile. It prebuilds states/profile/radius and black-boxes complete outputs, but calls `ball_radius.clone()` once per impact inside the measured loop. The quick numbers above include that clone and cannot be used as the primary after/before baseline for an internal scalar hoist.

### Required benchmark correction and missing fixtures

Before touching `src/lib.rs`:

1. Exclude owned-radius cloning/setup from the measured routine without changing the public API. Use `iter_batched_ref`: setup clones a 216-element radius vector; the timed routine drains it to pass each owned `Inches` into the public call; Criterion drops only the now-empty backing vector outside the measured routine. Destruction of each consumed `Inches` remains timed because the by-value public call owns it until return; treat that as identical unavoidable API overhead on baseline, candidate, and controls. Apply the identical setup and loop shape to all three models, and set `Throughput::Elements(216)` explicitly for each batch point. Do not subtract a separately measured clone or destructor estimate from results.
2. Outside timing, run the full matrix once for each model; assert all outputs are finite and black-box each complete `OnTableBallState` in timing.
3. Add `rail_resolution/spin_aware_scalar` for one fixed existing matrix element. Use `iter_batched_ref` setup with `Some(ball_radius.clone())`, then `take()` the owned radius in the timed routine so cloning remains outside timing; the consumed value's destructor remains timed as unavoidable by-value API overhead. Use `Top`, total speed 60 in/s, ratio 0.5, vertical-spin factor `+1.0`, and exact rolling horizontal spin. Name the factor, not a presumed running/reverse interpretation. Set `Throughput::Elements(1)` before registering this point so it does not inherit the batch's 216-impact throughput metadata.
4. Add `core_functions/trace_ball_path_with_rails_on_table/bank_duration_1s_spin_aware` beside the unchanged Mirror point. This trace is an end-to-end guardrail, not primary acceptance evidence.

Do not add a second 216-case matrix with different rail order, normal-speed semantics, side-spin normalization, or profile defaults. Boundary semantics for running/reverse signs, adherence, per-rail profiles, and radii belong in correctness tests, not a silently changed timing corpus.

### Baseline protocol

After the benchmark correction but before production edits:

1. Run the focused direct group with the same stable Rust toolchain, release Criterion profile, feature set, `RUSTFLAGS`, and power configuration intended for the candidate.
2. Save a distinct Criterion baseline for each of three pairs. Compare each candidate run with its matching baseline and alternate unchanged/candidate processes rather than collecting all unchanged runs in one thermal block.
3. Use at least 30 samples, ten seconds measurement time, and Criterion's bootstrap 95% confidence interval. Record scalar and 216-impact batch separately; never average models or fixture strata.
4. Use the corrected direct batch with Criterion profiling mode for a macOS `sample` capture. SpinAware resolution and its impulse loop must dominate the isolated workload before coefficient plumbing is justified.

Focused filters:

```text
cargo bench --bench physics -- 'rail_resolution'
cargo bench --bench physics -- 'core_functions/trace_ball_path_with_rails_on_table/bank_duration_1s_spin_aware'
```

Use Criterion's `--save-baseline <name>` on unchanged runs and `--baseline <name>` on the matching candidate runs. The already observed quick ranges remain context only because they include in-loop radius cloning.

## Stage 2: establish equivalence gates before refactoring

### Bitwise gate

In `tests/rail_collisions.rs`, reuse the exact deterministic ordering and value definitions from the committed 216-case matrix above. Before the optimization, compute a compact baseline fingerprint for the implementation host/toolchain over, in order:

1. rail and the four fixture-axis discriminants/indices;
2. output position `x` and `y` `f64::to_bits()` values;
3. output velocity `x` and `y` bits;
4. output angular velocity `x`, `y`, and `z` bits.

Use a tiny explicitly specified integer fold (for example FNV-1a over each `u64`'s little-endian bytes), with no new dependency. Generate and review the expected constant on the unchanged implementation before editing production code. A checked-in constant must be target/toolchain-gated unless portability is proven; platform-independent CI keeps the established semantic rail assertions. Also compare two same-process executions element by element so a mismatch reports the exact fixture and component rather than only the aggregate fingerprint.

The candidate must reproduce the unchanged same-host fingerprint and every component bit. A new finite tolerance is not an acceptable substitute for this micro-optimization: the permitted transformation does not require changing arithmetic grouping.

### Numerical and branch-boundary gate

Add focused tests that fail on plausible context-lifetime or coefficient-wiring bugs:

- all four rails produce basis-rotated equivalent local-frame outputs for one nonzero value of every velocity/spin component;
- two unequal per-rail profile entries prove `profile.for_rail(rail)` is still selected before context construction;
- radius values on both sides of the typical radius prove no context leaks between impacts;
- normal restitution 0 and 1, cushion friction 0 and 1, and impact cloth friction 0 and 1 prove the fields are not swapped;
- cushion-contact and cloth-contact slip magnitudes immediately below, exactly at, and immediately above the 0.02 in/s adherence scale retain the existing branch and continuity behavior;
- an entry with `normal_speed_toward_cushion <= f64::EPSILON` retains the solver's unchanged early return;
- rolling, sliding, zero-side-spin, running, reverse, and overspin cases retain their guardrail behavior.

For independent physics checks, retain the existing repository tolerances rather than replacing them with the fingerprint: frictionless normal restitution remains within `1e-7` (`tests/rail_collisions.rs:125`), exact/no-slip component checks remain at `1e-9`, no-slip continuity keeps its existing `1e-2`/`1e-3` bounds, the outgoing rolling low-English cloth-slip ratio remains at most `0.8 + 1e-9`, and the lag settled-speed ratio remains `0.5 ± 0.04`.

Run only the focused suites during implementation:

```text
cargo test --test rail_collisions
cargo test --test rail_event_execution
cargo test --test lag_calibration
```

Any bitwise mismatch rejects the optimization even if all looser numerical assertions pass.

## Stage 3: inspect optimized Apple arm64 output

Inspect the unchanged release artifact before designing fields. Use the benchmark's exact `RUSTFLAGS`; on Apple arm64 include `-C target-cpu=native` only if it is also used for both timing sides. Force one codegen unit for readable inspection without changing the timed configuration silently:

```text
cargo rustc --release --lib -- -C codegen-units=1 --emit=asm,llvm-ir
```

If native CPU flags are part of the benchmark protocol, append `-C target-cpu=native` to the command above and use the same flag for baseline and candidate benchmarks. Keep assembly/IR snapshots outside the repository (for example under `/tmp`) and compare artifacts emitted under `target/release/deps`; do not commit generated research artifacts.

Inspect `solve_spin_aware_rail_impact_in_frame`, both phase back-edges, and the inlined body of `advance_rail_impact_frame_by_impulse_step`. Inlining may remove private symbol boundaries, so follow the call from the public profile entry and identify the loops by their normal-speed/work comparisons and back-edges rather than assuming every Rust function has a symbol.

Record for unchanged and candidate output:

- whether the `5.0 / (2.0 * ball_radius)` division is inside or outside each hot back-edge;
- dynamic scalar `fdiv` sites, distinguishing the two required state-dependent slip normalizations from an invariant angular-scale division;
- repeated radius/angle/friction multiplications in the back-edge;
- coefficient loads and stack spills introduced or removed;
- calls implementing `hypot`, which must remain;
- code size and branch shape of compression, restitution, and terminal refinement paths;
- whether post-solve incoming speed, cloth-slip, rolling-proximity, and side-spin calculations are duplicated after inlining.

Do not infer a win merely because Rust source has fewer expressions. The required corroboration is a smaller relevant dynamic instruction shape without new spills, followed by the timing gate.

## Stage 4: conditional implementation

### 4.1 Loop coefficient context

Only if unchanged arm64 output leaves invariant arithmetic in a hot back-edge, add an internal type near `RailImpactFrameState`:

```rust
#[derive(Clone, Copy, Debug)]
struct RailImpactCoefficients {
    ball_radius: f64,
    sin_theta: f64,
    cos_theta: f64,
    angular_scale: f64,
    normal_restitution: f64,
    table_friction: f64,
    cushion_friction: f64,
    nominal_delta_p: f64,
}
```

Construct it once in `solve_spin_aware_rail_impact_in_frame`, after the existing non-approaching early return. Compute every field with the exact current expression and grouping. Pass `&RailImpactCoefficients` through compression, restitution, and step functions; the type is `Copy` for cheap local snapshots but borrowing avoids a large-by-value ABI and satisfies the repository's `large_types_passed_by_value` lint.

Use `coefficients.angular_scale` at the current use sites. Keep all state-dependent formulas textually ordered as they are now. In particular:

- keep `state.angular_normal_toward_cushion * ball_radius * sin_theta` left-associated;
- keep `state.angular_vertical * ball_radius * cos_theta` left-associated;
- keep coupling, tangential impulse, normal impulse, and all five state updates in their current operation order;
- keep work accumulation and midpoint comparisons unchanged;
- keep `nominal_delta_p` and the 1,000/48 iteration controls unchanged.

Do **not** add `radius_sin`, `radius_cos`, precombined friction-angle terms, or fused multiply-add expressions merely because they look invariant. They regroup current arithmetic and violate the bitwise contract. If the only apparent loop opportunity requires such reassociation, reject the loop part of the candidate.

If LLVM already keeps `angular_scale` and the other fields outside the loop in registers, do not add `RailImpactCoefficients` for aesthetics. Extra pointer loads or spills are a regression, not an abstraction benefit.

### 4.2 Post-solve entry metrics

If the unchanged optimized output/profile still repeats the incoming-state conversions, add a separate small internal `Copy` value, for example:

```rust
#[derive(Clone, Copy, Debug)]
struct RailEntryMetrics {
    speed: f64,
    rolling_proximity: f64,
    explicit_side_spin_scale: f64,
}
```

Construct it exactly once from `state_ref` after the impulse solve and before `outgoing_wz` is computed. Preserve the current `speed <= f64::EPSILON` branch semantics. For nonzero speed:

1. call `ball_speed` once;
2. call `cloth_contact_velocity_on_table` once and compute the same `hypot / speed` rolling-proximity expression;
3. compute the same `abs(radius * wz) / speed.max(f64::EPSILON)` side-spin ratio and clamp.

Expose methods or free helpers taking `RailEntryMetrics` for the three derived values currently returned by:

- `rail_running_english_generation_scale`;
- `rail_rebound_horizontal_spin_blend`;
- `rail_rebound_outgoing_cloth_slip_ratio_limit`.

Change `clamp_rail_rebound_horizontal_spin_to_slip_limit` to receive the entry metrics rather than recomputing rolling proximity and the outgoing limit from `state_before`. Keep its **outgoing** speed and outgoing cloth-slip calculations local: those depend on `velocity_after` and provisional outgoing horizontal spin and must not be confused with incoming metrics.

Delete the superseded internal helper paths after migrating all local call sites; do not retain aliases or parallel conventions. Keep `rail_entry_is_overspinning_relative_to_cloth_rolling` separate because its projections and sign checks are different data, not duplicated entry metrics.

The context must remain stack-only, immutable after construction, impact-local, and private. It must not be stored in `RailCollisionProfile`, cached globally, or exposed through a public API.

### 4.3 Clean cutover

Migrate the straight-rail route and the jaw route to the same internal constructor/signatures. Public functions, profile selection, exact-unit arguments, and return types remain unchanged. No compatibility shim is needed for private functions, and no old scalar-argument overload should remain.

## Acceptance gates

Correctness gates run before performance is considered.

### Mandatory correctness

- The ordered 216-fixture output fingerprint is identical to the unchanged implementation.
- Position, velocity, and angular-velocity components are bitwise identical for every fixture.
- All new boundary/profile/radius tests and the existing focused rail suites pass.
- Straight-rail event kind, elapsed time, and no-repeat behavior remain unchanged.
- Jaw resolution remains on the shared basis solver and passes existing pocket/jaw tests affected by the signature cutover.
- No expression reassociation, fast-math, or step/refinement-count change is present.

### Statistical performance

Use three paired baseline/candidate process runs on the same Apple arm64 host, stable Rust toolchain, release configuration, feature set, `RUSTFLAGS`, power source, and thermal conditions. Alternate the processes within each pair. Use Criterion's bootstrap 95% confidence interval for candidate/baseline time ratio.

Primary acceptance requires all of the following:

- `rail_resolution/spin_aware_216_impacts`: the upper bound of the 95% CI is at most **0.95** in each paired run;
- `rail_resolution/spin_aware_scalar`: the upper bound is at most **0.97** in each paired run;
- all three pair point estimates agree in direction; no cherry-picking of the best pair;
- candidate arm64 output removes the expected invariant/repeated instructions and introduces no compensating hot-loop spills;
- the Copy contexts cause no heap allocation.

Guardrails:

- `rail_resolution/mirror_control_216_impacts` and `rail_resolution/restitution_only_control_216_impacts` candidate/baseline 95% CI upper bounds must be at most **1.02**;
- the SpinAware one-second bank trace upper bound must be at most **1.02**;
- no individual fixture stratum exposed by a diagnostic split may regress by more than 2% at the 95% CI upper bound;
- end-to-end DSL/trace results are corroboration only and cannot rescue a failed direct benchmark.

A gain visible only after averaging SpinAware with the much cheaper Mirror or RestitutionOnly paths is invalid.

## Rejection and stop conditions

Reject or narrow the production change, while retaining useful direct benchmark/test coverage, if any of these holds:

1. LLVM already hoists the angular-scale division and other legal invariants outside both phase loops, leaving no smaller instruction shape available without reassociation.
2. The coefficient pointer causes additional loads, register pressure, spills, code growth, or a slower refinement path that offsets the removed arithmetic.
3. Post-solve helpers are already common-subexpression-eliminated in optimized output, or their isolated contribution is below measurement noise.
4. Any result bit changes, even if an existing tolerance test still passes.
5. Any adherence, zero-restitution, overspin, profile-selection, radius, rail-orientation, jaw, or event-order test changes behavior.
6. The batch does not clear 5%, the scalar point does not clear 3%, any of the three paired runs disagrees, or a primary 95% CI crosses its threshold.
7. A gain appears only under `target-cpu=native` when the shipped/timed configuration does not use that setting.
8. A proposed field requires changing multiplication grouping, adding FMA, approximating `hypot`, or sharing incoming values with outgoing guardrail calculations.

Do not respond to rejection by reducing integration steps, relaxing tolerances, changing friction/restitution math, or expanding into a batch API. Those are different physics/API proposals.

## Risks and mitigations

- **Floating-point regrouping:** Precomputed products can change bits. Mitigation: hoist only identically grouped expressions, forbid radius-angle products, and require full-grid bit identity.
- **Wrong coefficient lifetime:** Radius and profile values differ per impact/rail. Mitigation: construct once per call after `profile.for_rail`, never cache across calls, and test unequal rails and radii.
- **Incoming/outgoing conflation:** Post-solve guardrails use both incoming metrics and outgoing velocity/spin. Mitigation: `RailEntryMetrics` contains only incoming quantities; outgoing speed/slip remain local to the clamp.
- **Register pressure:** A context pointer can be worse than scalar SSA values. Mitigation: compare arm64 spills/loads and reject if the instruction shape worsens.
- **Benchmark overfitting:** A single head-on bank misses tangent and spin branches. Mitigation: use the full 216-case matrix plus a separately reported scalar point and controls.
- **Jaw divergence:** The jaw route shares the basis solver but not the straight-rail entry function. Mitigation: migrate it in the same cutover and run focused jaw/pocket coverage.
- **Thermal noise on macOS:** Long paired runs can drift. Mitigation: alternate baseline/candidate processes, use three pairs and bootstrap CIs, and reject inconsistent direction.

## SIMD and GPU decision

Explicit arm64 SIMD is not justified for this scalar solver. Compression, restitution, and each terminal refinement are recurrences: impulse state at step `n + 1` depends on step `n`, so time steps cannot be vectorized without changing the algorithm. The two slip vectors within a step are small, branch through zero/adherence handling, feed different scalar equations, and would require packing/unpacking; manual NEON would add overhead and likely change operation order. Apple arm64 already has strong scalar floating-point scheduling, so the correct action is to let LLVM schedule independent scalar operations and verify the result in assembly.

Vectorizing across impacts would require a new structure-of-arrays batch API. The simulator normally resolves one selected event at a time, and independent impacts have divergent compression/refinement iteration behavior. There is no demonstrated production batch consumer, and such an API is an explicit non-goal here.

A GPU path is still less appropriate: one rail event provides far too little parallel work to amortize dispatch and transfer, simulation state is CPU-resident, divergent recurrent loops underutilize GPU lanes, and different floating-point behavior threatens the bitwise determinism gate. No SIMD or GPU speedup is claimed or required by this plan.

## Rollback

Keep the direct benchmarks and correctness fixtures if they are useful, but roll back the production contexts as one self-contained change if any correctness, assembly, or statistical gate fails. Restore the original private scalar signatures and post-solve helpers; public APIs and serialized data require no migration because this plan never changes them. There is no persistent cache, schema, feature flag, or compatibility layer to unwind.

## Candidate commit message

```text
perf(physics): hoist proven spin-aware rail invariants
```
