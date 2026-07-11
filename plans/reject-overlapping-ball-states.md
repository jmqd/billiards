# Reject overlapping ball states at N-ball construction boundaries

- **Date:** 2026-07-10
- **Severity:** Medium
- **Priority:** P2
- **Audit basis:** `agent://EventSchedulerAudit`, Finding 3

## Problem statement

The public N-ball schedulers accept each `OnTableBallState` independently but never validate the geometry of the collection. Two rigid balls can therefore begin with a negative center gap,

\[
g_{ij}=\lVert \mathbf{x}_j-\mathbf{x}_i\rVert-(R_i+R_j)<0,
\]

receive a velocity-level impulse at the penetrated positions, and later be reported as a terminal system while their centers remain interpenetrating. The DSL has the same hole because it converts every placed ball independently before constructing `Vec<NBallSystemState>`.

The implementation must make aggregate nonpenetration a public N-ball input invariant. Gross overlap must return an indexed error before event prediction or impulse resolution. Only a fixed, extremely small construction-arithmetic tolerance may be recovered positionally; that recovery must preserve the pair midpoint, velocities, angular velocities, indices, and exact frozen contacts.

## Current behavior and impact

### Observed behavior

- `OnTableBallState::try_new` and `try_new_with_thresholds` (`src/lib.rs:2912-2963`) validate height and vertical velocity for one ball. They cannot enforce pairwise separation.
- `NBallSystemState` and its conversions (`src/lib.rs:2307-2357`) are likewise per-ball representations with no aggregate constructor.
- `compute_next_ball_ball_collision_on_table` (`src/lib.rs:5687-5736`) treats `quadratic_c <= 0` as a zero-time collision only when relative motion is closing; stationary or separating penetrated pairs return `None`.
- `compute_next_ball_ball_collision_during_current_phases_on_table` (`src/lib.rs:5769-5832`) has the same policy at `initial_gap <= 0`: it emits a zero-time event for closing motion and returns `None` otherwise.
- `select_earliest_n_ball_event_from_states` (`src/lib.rs:8834-8895`) calls the pair predictor for all pairs without first establishing aggregate nonpenetration. `compute_next_n_ball_event_on_table` and `compute_next_n_ball_event_with_rails_on_table` expose that selection publicly (`src/lib.rs:8902-8922`).
- `advance_to_next_n_ball_event_with_scheduler` (`src/lib.rs:9933-10048`) and the richer system resolver (`src/lib.rs:10742-10886`) advance positions to an event and then resolve collision velocities/spins. Neither performs positional depenetration.
- The until-rest loop (`src/lib.rs:10522-10577`) stops when an advance returns `event: None`. Its public documentation says it stops with balls “resting and separated” (`src/lib.rs:10579-10597`), but an initially stationary overlapped pair immediately reaches that exit.
- `PocketAwareEventCache::build`/`refresh_ball`/`next_event` (`src/lib.rs:1972-2256`) and the richer system simulation loop (`src/lib.rs:11015-11087`) inherit the same invalid-state behavior.
- `DslScenario::initial_shot_system_states_on_table` (`src/dsl.rs:303-338`) converts and pushes each placement independently. Its simulation and trace methods call the public system simulators at `src/dsl.rs:340-465`, while trace replay calls the public resolver at `src/dsl.rs:601-695`; none can currently report invalid pair geometry.

### Impact

- Invalid rigid-body configurations are indistinguishable from valid “no event” states.
- A closing penetrated pair receives an impulse at the wrong geometry. Because the impulse changes velocity but not position, the next scheduler call can see a separating penetrated pair and stop permanently.
- A stationary penetrated pair can terminate immediately with no diagnostic.
- DSL typos or malformed externally constructed state arrays can silently contaminate shot previews, search trees, and downstream event logs.
- The error is systemic: adding a special case to one collision response cannot establish the invariant for every N-ball/public system path.

## Exact affected files, symbols, and callers

### Core state and validation model

- `src/lib.rs:2272-2305`: `NBallOnTableAdvance`, `NBallOnTableSimulation` return state collections but cannot currently return geometry errors.
- `src/lib.rs:2307-2357`: `NBallSystemState`, `From<OnTableBallState>`, `From<BallState>`.
- `src/lib.rs:2750-2768`: existing per-ball `OnTableStateError` and `RestingOnTableStateError`; the new aggregate error belongs beside these but must remain a distinct type.
- `src/lib.rs:2912-3073`: `OnTableBallState` and `RestingOnTableBallState` validation. These types remain per-ball and must not be burdened with collection geometry.

### Prediction and execution internals

- `src/lib.rs:1972-2256`: `PocketAwareEventCache::{build, refresh_ball, next_event}`.
- `src/lib.rs:5687-5736`: `compute_next_ball_ball_collision_on_table` overlap branch.
- `src/lib.rs:5739-5749`: `raw_ball_ball_contact_is_closing`.
- `src/lib.rs:5769-5832`: `compute_next_ball_ball_collision_during_current_phases_on_table` overlap branch.
- `src/lib.rs:8834-8922`: `select_earliest_n_ball_event_from_states`, `compute_next_n_ball_event_on_table`, `compute_next_n_ball_event_with_rails_on_table`.
- `src/lib.rs:9933-10179`: `advance_to_next_n_ball_event_with_scheduler` and all public N-ball advance variants.
- `src/lib.rs:10522-10689`: `simulate_n_ball_system_on_table_until_rest` and all public on-table N-ball until-rest variants.
- `src/lib.rs:10691-10886`: `advance_n_ball_system_without_event`, public richer-system prediction, and `resolve_n_ball_system_event_with_physics_and_pockets_on_table`.
- `src/lib.rs:10888-11222`: all richer-system and pocket-aware N-ball advance/simulation variants.

### Public APIs requiring a clean `Result` cutover

The following state-collection seams must stop returning an unqualified success value and return `Result<ExistingSuccessType, NBallGeometryError>` (or `Result<Option<_>, NBallGeometryError>` for pure event queries):

- `compute_next_n_ball_event_on_table`
- `compute_next_n_ball_event_with_rails_on_table`
- `advance_to_next_n_ball_event_on_table`
- `advance_to_next_n_ball_event_with_physics_on_table`
- `advance_to_next_n_ball_event_with_rail_profile_on_table`
- `advance_to_next_n_ball_event_with_physics_and_rail_profile_on_table`
- `advance_to_next_n_ball_event_with_rail_config_on_table`
- `advance_to_next_n_ball_event_with_rails_on_table`
- `simulate_n_balls_on_table_until_rest`
- `simulate_n_balls_with_physics_on_table_until_rest`
- `simulate_n_balls_with_rail_profile_on_table_until_rest`
- `simulate_n_balls_with_rail_config_on_table_until_rest`
- `simulate_n_balls_with_rails_on_table_until_rest`
- `compute_next_n_ball_system_event_with_rails_and_pockets_on_table`
- `resolve_n_ball_system_event_with_physics_and_pockets_on_table`
- `advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table`
- `advance_to_next_n_ball_system_event_with_rail_profile_and_pockets_on_table`
- `advance_to_next_n_ball_system_event_with_rail_config_and_pockets_on_table`
- `advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table`
- `simulate_n_ball_system_with_physics_and_pockets_on_table_until_rest`
- `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit`
- `simulate_n_ball_system_with_rail_profile_and_pockets_on_table_until_rest`
- `simulate_n_ball_system_with_rail_config_and_pockets_on_table_until_rest`
- `simulate_n_ball_system_with_rails_and_pockets_on_table_until_rest`
- `simulate_n_balls_with_physics_and_pockets_on_table_until_rest`
- `simulate_n_balls_with_rail_profile_and_pockets_on_table_until_rest`
- `simulate_n_balls_with_rails_and_pockets_on_table_until_rest`

The two-ball compatibility event/advance/simulation wrappers at `src/lib.rs:8928-8971` and `src/lib.rs:10185-10519` delegate to these N-ball paths. They must propagate the same error as `Result<ExistingSuccessType, NBallGeometryError>` rather than unwrap it or preserve a second unchecked convention.

Direct binary timing/response primitives remain expert-level pair operations; aggregate construction and collection validity must not be reimplemented independently inside every pair impulse model.

### DSL and repository callsites

- `src/dsl.rs:303-338`: `DslScenario::initial_shot_system_states_on_table` is the DSL construction seam and must map indexed geometry failure to `DslBuildError`.
- `src/dsl.rs:340-465`: system simulation and trace entry points must use `?` to propagate the core result.
- `src/dsl.rs:601-695`: `ball_traces_from_simulation` must return a result and propagate replay-resolution failure rather than assuming replay is infallible.
- `src/dsl.rs:1826-2053`: `DslBuildError` and its `Display`/`Error` implementation need the new invalid-layout case.
- Compile-time callsite migration is required in `tests/n_ball_events.rs`, `tests/n_ball_advance.rs`, `tests/n_ball_simulation.rs`, `tests/n_ball_pockets.rs`, `tests/break_shots.rs`, `tests/dsl.rs`, `tests/advance_two_ball_events.rs`, `tests/two_ball_simulation.rs`, `tests/rail_event_scheduling.rs`, and `tests/rail_event_execution.rs`.
- Benchmark callers must explicitly unwrap known-valid fixtures in `benches/physics.rs` and `benches/throughput.rs`; an ignored `Result` must not accidentally become the measured workload.

## Whitepaper evidence and reliability boundaries

### Rigid admissibility and first contact

For equal balls in this repository, rigid admissibility is

\[
g_{ij}=\lVert\mathbf{x}_j-\mathbf{x}_i\rVert-2R\ge 0.
\]

`whitepapers/toward_a_competitive_pool_playing_robot.pdf`, p. 5, “Physics simulation” (extracted corpus lines 174-221), describes a continuous-domain event simulator that parameterizes the **separation of two moving balls** as a function of time and solves the resulting quartic for collision time, with “no discrete time step.” This supports collision at the first separation satisfying contact, not acceptance of an already-interpenetrating terminal configuration.

Reliability boundary: the article describes the Deep Green simulator at a system level and does not prescribe this repository’s error type or numerical tolerance. It supports separation-based first-contact semantics only.

### An impulse cannot repair a positional overlap

`whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf`, pp. 1-3, §1 and §2.1 (extracted corpus lines 19-29 and 69-135), models rigid-sphere impact as instantaneous forces/impulses at a single contact point. Equations (1)-(6) express impulse through changes in linear momentum, Eq. (7) defines restitution from pre/post normal relative velocity, and Eq. (8) gives the normal impulse. In compact form,

\[
\mathbf{p}^{+}-\mathbf{p}^{-}=\mathbf{J},
\qquad
 e=-\frac{u_n^{+}}{u_n^{-}}.
\]

Those are velocity/momentum-level laws. They do not provide a positional depenetration operation, so applying the current collision response at `g<0` cannot make the configuration admissible.

Reliability boundary: Doménech is a peer-reviewed binary collision formulation with simplifying assumptions about discontinuous impact and constant coefficients. It supports the distinction between impulse and position correction; it is not evidence for a general N-contact projection algorithm.

### Physical-compression scale and frozen spacing

`whitepapers/tp_b_29_simulation_of_a_cb_striking_two_frozen_obs_along_their_line_of_centers.pdf` gives `R = 1.125 in` and cites experimental full compression `δ = 0.2 mm` for a `1 m/s` two-ball collision (extracted corpus lines 44-64). Its three-ball worked simulation initializes frozen centers at `D = 2R` (lines 118-126) and computes compliant force only while compression is positive (lines 222-230).

This evidence gives two useful scales:

- rigid frozen construction is center spacing `D=2R`;
- `0.2 mm` is already a real finite-impact compression example, not a license for a rigid event solver to accept millimeter-scale overlap.

Reliability boundary: TP B.29 is an Alciatore technical worked example, not a peer-reviewed universal material law. The `0.2 mm` value is the cited full-compression example at `1 m/s`, not a universal maximum. The implementation tolerance below is therefore a numerical software policy chosen orders of magnitude smaller than that example, not a fitted compliance parameter.

## Numeric reproducer

Use the repository’s standard pool-ball radius:

- `R = 1.125 in = 28.575 mm`;
- required center spacing `2R = 2.25 in = 57.15 mm`;
- ball A position `(0, 0) in`, velocity `(10, 0) in/s`;
- ball B position `(2.0, 0) in`, velocity `(0, 0) in/s`;
- both angular velocities zero.

The signed gap is

\[
g=2.0\text{ in}-2.25\text{ in}=-0.25\text{ in}=-6.35\text{ mm}.
\]

This overlap is `6.35/0.2 = 31.75` times TP B.29’s cited `0.2 mm` full-compression example. In the current squared-gap code,

\[
\texttt{initial\_gap}=2.0^2-2.25^2=-1.0625\text{ in}^2.
\]

For the closing fixture, `r·v_rel = (2,0)·(-10,0) = -20 in²/s`, so the current predictor emits a collision at `t=0`. An ideal equal-mass response transfers the x velocity to B but leaves centers at `(0,0)` and `(2,0)`. The next relative radial velocity is separating, so the predictor returns `None` and the negative positional gap survives. With both velocities initially zero, the first query already returns `None`.

The planned recovery tolerance is exactly

\[
\tau=10^{-6}\text{ in}=0.0000254\text{ mm}.
\]

Thus the counterexample penetration is `250,000 τ`, and the TP B.29 compression example is about `7,874 τ`. Neither can enter the numerical-recovery path.

## Root cause

1. The type boundary is per-ball: `OnTableBallState` establishes cloth-contact kinematics, not collection geometry.
2. Public N-ball functions accept raw slices instead of a validated aggregate and return unconditional success/`Option`, so malformed geometry has no error channel.
3. The overlap branches conflate exact touching, tiny floating-point penetration, and gross overlap under one `<= 0` comparison, then decide solely from relative velocity.
4. Collision execution is correctly velocity-level for rigid impact, but the scheduler implicitly treats it as if it also repaired positions.
5. DSL construction preserves each placement but never validates the completed layout before striking or simulation.

## Proposed design

Everything in this section is proposed; the names and policies do not exist in current source.

### Aggregate error contract

Add a public `NBallGeometryError` beside the existing state-domain errors in `src/lib.rs`:

```rust
pub enum NBallGeometryError {
    OverlappingOnTableBalls {
        first_ball_index: usize,
        second_ball_index: usize,
        center_distance: Inches,
        required_center_distance: Inches,
        penetration: Inches,
        recovery_tolerance: Inches,
    },
}
```

Requirements:

- Pair indices are canonical (`first_ball_index < second_ball_index`) and refer to the caller’s original order.
- Distances and penetration remain unit-bearing types; `penetration` is positive in the error.
- `Display` includes both indices and values in inches so CLI/DSL diagnostics are actionable.
- The error derives `Clone`, `Debug`, and `PartialEq` consistently with neighboring errors and implements `std::error::Error`.
- A coincident-center pair is grossly invalid for any positive radius and returns this error; recovery never invents a normal direction.
- Non-finite separation must not pass as valid. If existing scalar constructors permit a non-finite position, represent it with a dedicated non-finite variant in the same enum rather than allowing `NaN` comparisons to bypass overlap checks.

In `src/dsl.rs`, add `DslBuildError::InvalidNBallGeometry` carrying the core error plus the two `BallType`/ball references looked up from the reported indices. Its display text must name both DSL balls and retain the indexed/unit-bearing core details.

### One canonical validator/recovery helper

Add one internal helper for contiguous on-table slices and one thin adapter for `NBallSystemState`:

- Iterate every canonical on-table pair exactly once and calculate `distance = dx.hypot(dy)`, `required = 2R`, and `penetration = required - distance`. Use the distance gap, not the squared gap, for a unit-correct tolerance comparison.
- `penetration <= 0`: accept without changing either position. In particular, exact `distance == 2R` frozen contacts are bit-for-bit unchanged.
- `0 < penetration <= τ`: recover positionally.
- `penetration > τ`: return `NBallGeometryError` before event prediction, advancement, or impulse calculation.
- For `NBallSystemState`, validate/project only pairs whose current variants are both `OnTable`. `Pocketed` balls are inert. Airborne geometry remains outside this plan, but once a table bounce converts a state to `OnTable`, the post-event invariant check applies before that state can be returned or scheduled again.

For one recoverable pair with `d>0`, use the contact normal

\[
\mathbf{n}=\frac{\mathbf{x}_j-\mathbf{x}_i}{d}
\]

and equal/opposite position correction

\[
\mathbf{x}'_i=\mathbf{x}_i-\frac{c}{2}\mathbf{n},
\qquad
\mathbf{x}'_j=\mathbf{x}_j+\frac{c}{2}\mathbf{n},
\]

where `c` is the measured penetration plus only the few ulps needed for the recomputed `hypot` to satisfy `distance >= required`. This preserves the pair midpoint exactly up to arithmetic and leaves linear/angular velocity untouched.

For a small connected graph of recoverable overlaps, use deterministic canonical-pair position iterations, accumulating equal/opposite corrections from one iteration snapshot before applying them. Bound the process by all of the following:

- every original and intermediate penetration must remain `<= τ`;
- each ball’s cumulative displacement must remain `<= τ`;
- use a fixed small iteration cap (16 is sufficient as the initial implementation bound);
- after the final iteration, recompute every pair and require `distance >= 2R` under the same `hypot` calculation used by validation;
- if any bound or convergence check fails, return the canonical most-penetrated pair as `OverlappingOnTableBalls` rather than broadening the tolerance or returning a partially projected state.

This is a roundoff/construction-arithmetic repair, not a physical compliance solver. Do not change velocity to match the positional correction, consume simulation time, emit an event, or resolve an impulse merely because recovery occurred.

### Boundary placement and invariant lifetime

- Validate/recover once at every public N-ball prediction, advance, resolve, and simulation entry point before building candidates or a cache.
- State-producing APIs return the recovered snapshot even when there is no event, so a recoverable stationary pair cannot be returned still penetrated.
- Pure public event queries return `Result<Option<Event>, NBallGeometryError>` and perform prediction against the same recovered local snapshot. Their rustdoc must state that they do not mutate caller-owned inputs; callers requiring the recovered state should use the advance/simulation seam. Gross overlap is always `Err`, never `Ok(None)`.
- Internal simulation loops operate on their owned `Vec` and run the same bounded validation after each resolved event and before rebuilding candidates/cache. This catches and narrowly repairs event-generated floating residue while converting any material solver-created penetration into an error.
- `resolve_n_ball_system_event_with_physics_and_pockets_on_table` validates both the supplied pre-event system and the resolved output. A caller cannot bypass the invariant by manually replaying an event.
- Keep private “already validated” prediction/resolution helpers so a simulation does not repeat O(N²) validation through nested public wrappers on the same state. Public entry validates once; each newly produced event boundary validates once.
- Update the `initial_gap <= 0` comments/guards in both pair predictors to distinguish exact/recovered contact from invalid overlap. From validated N-ball paths, those branches may see contact and floating residue only; they must not be presented as gross-overlap handling.

### API and data-model cutover

This is a clean breaking cutover, with no `_checked` duplicate APIs, panicking compatibility shims, aliases, or deprecated unchecked paths.

- N-ball prediction APIs: `Option<Event>` becomes `Result<Option<Event>, NBallGeometryError>`.
- N-ball advance APIs: `NBallOnTableAdvance`/`NBallSystemAdvance` becomes `Result<..., NBallGeometryError>`.
- N-ball simulation APIs: `NBallOnTableSimulation`/`NBallSystemSimulation` becomes `Result<..., NBallGeometryError>`.
- Public system event resolution: `Vec<NBallSystemState>` becomes `Result<Vec<NBallSystemState>, NBallGeometryError>`.
- Delegating two-ball event, advance, and simulation wrappers propagate `Result`; they must not call `expect` because a two-element slice is still an aggregate geometry input.
- Existing success structs and event enums remain unchanged. Geometry failure is an input/state invariant error, not a synthetic `NBallOnTableEvent` or terminal diagnostic event.
- `DslScenario::initial_shot_system_states_on_table` already returns `Result`; preserve that outer shape, but build all resting states, validate/recover the full geometry, and only then apply the cue strike to the selected ball at its recovered position.
- DSL simulation/trace methods preserve their existing `Result<Option<_>, DslBuildError>` signatures and map core errors with `?`.
- Update every repository caller in one commit. Do not leave a caller discarding a result with `let _ =`, `unwrap_or_default`, or an empty simulation fallback.

## Explicit non-goals

- Do not model finite Hertzian ball compression, contact duration, compliant force propagation, or soft-body overlap. The solver remains rigid and event-driven.
- Do not depenetrate the `0.25 in / 6.35 mm` fixture, regardless of whether its pair is closing, separating, or stationary.
- Do not make the tolerance configurable through `OnTableMotionConfig`, playing conditions, or DSL. A user-configurable “slop” could silently become a compliance parameter and defeat rejection.
- Do not alter restitution, friction, spin transfer, or any binary collision impulse equation.
- Do not solve valid simultaneous-contact impulse graphs. `plans/coupled-nonideal-shared-contacts.md` owns admissible shared graphs at `g=0`.
- Do not change curved-path first-contact timing. `plans/curved-rolling-event-detection.md` owns future contact roots for valid `g>0` states.
- Do not add general airborne ball-ball, ball-jaw, rail, table-boundary, or pocket-occupancy validation. This plan validates current on-table ball pairs; unsupported airborne geometry remains separately scoped.
- Do not reorder balls, assign DSL identities in the physics core, or alter rack generation coordinates.
- Do not silently clamp gross overlap, drop one of the balls, perturb velocity, or return a placeholder event.

## Phased implementation plan

### Phase 1: Establish failing observable contracts

1. Add focused tests using the intended `Result` signatures before implementing recovery behavior.
2. In `tests/n_ball_simulation.rs`, add the stationary and closing `0.25 in` fixtures. Assert `Err(NBallGeometryError::OverlappingOnTableBalls { first_ball_index: 0, second_ball_index: 1, penetration: 0.25 in, .. })`, zero successful events, and no impulse-produced state.
3. In `tests/n_ball_events.rs`, prove both public N-ball event-query variants return the indexed error rather than `Ok(None)`/a zero-time collision for gross overlap.
4. In `tests/n_ball_pockets.rs`, exercise the richer `NBallSystemState` compute, advance, resolve, and simulation seams with the same on-table pair and require the same error.
5. In `tests/dsl.rs`, create two explicitly placed overlapping balls, attach a shot, and assert `DslBuildError::InvalidNBallGeometry` names both balls and reports `0.25 in` penetration.
6. Add exact/tolerance/frozen tests described in the acceptance section below before implementing the helper so the correction policy cannot drift during API migration.

### Phase 2: Implement aggregate validation and bounded recovery

1. Add `NBallGeometryError`, its trait implementations, and the private `τ = 1e-6 in` policy constant in `src/lib.rs`.
2. Implement the on-table aggregate validator/recovery using `hypot`, canonical pairs, midpoint-preserving equal/opposite correction, cumulative per-ball bounds, 16-iteration cap, and full final postcondition scan.
3. Implement the `NBallSystemState` adapter over on-table indices without losing original indices.
4. Unit-test single-pair projection math: midpoint, center distance, velocity, angular velocity, state ordering, and correction bound.
5. Tighten pair-predictor overlap comments and internal assertions so exact touching is not described as generic overlap handling.

### Phase 3: Cut public core APIs to `Result`

1. Convert the public event-query functions, then their two-ball compatibility wrappers, to propagate geometry errors.
2. Convert N-ball advance functions and `advance_to_next_n_ball_event_with_scheduler`. Return recovered states even when no event exists.
3. Convert on-table until-rest functions and normalize the owned initial vector once. Validate/recover exactly once after every event before the next loop iteration.
4. Convert richer-system compute/resolve/advance/simulation functions. Build `PocketAwareEventCache` only from a validated snapshot and revalidate after each resolution before cache rebuild.
5. Ensure manual resolver calls validate supplied state/event compatibility far enough to prevent an invalid input state from being advanced; this plan does not otherwise redesign event identity or stale-event semantics.
6. Introduce private validated helpers where needed so delegating wrappers do not perform duplicate O(N²) scans.
7. Migrate internal callsites with `?`; do not convert invariant errors to assertions.

### Phase 4: Validate DSL construction and propagate identities

1. Refactor `initial_shot_system_states_on_table` to collect all resting states first, run aggregate validation/recovery, and then strike the selected target without changing its recovered center.
2. Add `DslBuildError::InvalidNBallGeometry` and map pair indices to the corresponding placed ball identities while retaining the core error.
3. Propagate simulator errors through all DSL simulation/trace methods.
4. Change `ball_traces_from_simulation` to return `Result<Vec<ScenarioBallTrace>, DslBuildError>` and propagate resolver failures through callers.
5. Verify that malformed DSL fails during initial state construction, before an event log, trace segment, or rendered path is produced.

### Phase 5: Complete callsite migration and invariant cleanup

1. Update all affected integration tests to unwrap only known-valid construction results and to match errors explicitly in invalid-geometry tests.
2. Update `benches/physics.rs` and `benches/throughput.rs` so benchmark setup validates once where possible and measured calls consume the success value rather than benchmark an unconsumed `Result`.
3. Update public rustdoc to state the nonpenetration precondition, `1e-6 in` recovery policy, indexed error, and guaranteed nonpenetrating state output.
4. Remove any obsolete comments claiming `initial_gap <= 0` itself is a supported overlap-resolution strategy.
5. Search all migrated symbol callsites and require explicit propagation/handling; leave no old return signature or unchecked wrapper.

## Regression and acceptance tests

### Gross-overlap rejection

- **Stationary counterexample:** A `(0,0) in`, B `(2,0) in`, both resting, `R=1.125 in`. Every public N-ball prediction/advance/simulation family returns indexed penetration `0.25 in`; no success value contains `event: None` with the original geometry.
- **Closing counterexample:** same positions, A velocity `(10,0) in/s`, B resting. It returns the same geometry error before collision response. Assert there is no zero-time event and no velocity transfer.
- **Separating counterexample:** reverse the closing velocity and require the identical geometry error. Rejection depends on configuration, not relative normal velocity.
- **Indexing:** add an unrelated valid ball at index 0 and place the invalid pair at indices 1 and 2; assert the error reports `(1,2)` in canonical order.
- **Coincident centers:** require a deterministic indexed error without division by zero or non-finite corrected coordinates.

### Tolerance boundary and recovery invariants

Let `τ = 1e-6 in` and `D=2R`.

- `distance = D - 0.5τ`: success; returned state-producing API has recomputed `distance >= D`, unchanged midpoint within floating tolerance, unchanged velocities/spins, unchanged order, and each center moves no more than `0.25τ` plus the documented ulp guard.
- `distance = D - τ`: success at the inclusive boundary and the same output invariants.
- `distance = D - 100τ`: indexed error; no projection. This is the required `2R-ε` versus `2R-100ε` distinction with `ε=τ`.
- `distance = D + 0.5τ`: unchanged success; recovery must never pull separated balls together.
- Exact `distance = D`: bit-for-bit unchanged positions and normal scheduler semantics (closing contact may be a zero-time event; stationary/separating contact is valid geometry and may have no event).
- Dense recoverable graph: assert every final on-table pair satisfies `distance >= D`, the full system center-of-position sum is preserved within arithmetic tolerance, and no ball exceeds the cumulative `τ` displacement budget. A graph that cannot satisfy those bounds returns an error rather than a partially corrected success.
- Run the same tolerance cases through ordinary on-table and richer `NBallSystemState` paths.

### Frozen-rack preservation

- Keep `tests/rack_and_displacement.rs:87-130` passing: the generated nine-ball triangle has no overlap and exactly 16 touching pairs.
- Add a validation assertion around `racked_ball_positions()` proving the exact generated rack is accepted and unchanged, including all 16 touching relationships.
- Keep `tests/break_shots.rs:42-97` passing for the three named break scenarios. Their intended frozen contacts must not be rejected as gross overlap.
- Strengthen a break construction test to compare ball identities/order and contact graph before and after DSL N-ball validation. If decimal conversion produces sub-`τ` negative residue, recovery may move centers only within its bound and must retain all 16 intended touching contacts.
- Keep `tests/break_shots.rs:100-154` reaching the shared-contact path. Validation must not reinterpret valid `g=0` frozen contacts as an input error.

### End-to-end invariant

For every successful state-producing public N-ball call, inspect all pairs that are simultaneously `OnTable` and assert

\[
\lVert\mathbf{x}_j-\mathbf{x}_i\rVert \ge 2R
\]

under the same `hypot` computation as the validator. Apply the assertion to initial no-event returns, post-collision returns, every event boundary in until-rest simulation, and final terminal states. Tests must fail on the plausible regression where only closing overlaps are rejected or where an impulse is expected to repair position.

## Risks and edge cases

- **Frozen-contact false positives:** squared-gap cancellation and decimal DSL conversions can make a mathematically exact contact slightly negative. Comparing a unit-bearing distance penetration against `τ`, then recovering only within that bound, avoids rejecting intended racks without accepting physical overlap.
- **Projection order in contact graphs:** sequential pair mutation is order-dependent and can open or create neighboring overlaps. Snapshot-accumulated canonical-pair iterations plus a global final scan avoid returning a pairwise-only “fixed” graph.
- **Tolerance creep:** reusing motion/event time epsilons or making `τ` configurable would mix dimensions and semantics. Keep one position tolerance in inches, document it, and test both acceptance and rejection boundaries.
- **Hidden large cumulative correction:** many individually tiny overlaps could move one ball materially. The per-ball cumulative `τ` cap and convergence failure-to-error rule prevent this.
- **Zero separation:** normalization has no defined contact normal. Reject before division.
- **Query versus state-producing APIs:** event queries operate on a recovered local snapshot but cannot mutate caller-owned states. Rustdoc must direct callers that need normalized states to an advance/simulation result; gross overlap remains an error in both families.
- **Airborne transition:** airborne pairs are not projected. If a table bounce creates an on-table overlap, the post-resolution invariant check either repairs only sub-`τ` residue or returns an error. Full airborne collision geometry is not smuggled into this plan.
- **API break breadth:** changing foundational functions to `Result` will expose every ignored failure. Complete the cutover atomically across source, tests, DSL, and benches; do not add temporary unwrap shims.
- **Performance:** validation is O(N²), matching pair candidate generation. Validate once per public state boundary and avoid nested duplicate scans. Ordinary pool ball counts make this negligible relative to candidate construction.

## Source, documentation, and generated-artifact updates

- Update rustdoc on every changed public N-ball/two-ball wrapper to document `NBallGeometryError`, exact-contact acceptance, `1e-6 in` bounded recovery, and the nonpenetrating output invariant.
- Update `DslBuildError` display text so human-facing parse/build failures name both balls, pair indices, penetration, required distance, and recovery tolerance with units.
- The whitepapers are evidence and must not be edited.
- No `agent_knowledge` or other generated artifact is an implementation source of truth for this change; do not edit generated audit artifacts.
- No rack coordinates, example scenario files, or physics calibration constants should change. If an existing fixture exceeds `τ`, treat that as a real invalid fixture and diagnose it rather than widening tolerance.

## Verification commands

Run focused behavior first:

```sh
cargo test --test n_ball_simulation overlap
cargo test --test n_ball_events overlap
cargo test --test n_ball_advance roundoff
cargo test --test n_ball_pockets overlap
cargo test --test dsl overlap
cargo test --test rack_and_displacement given_racked_ball_positions_when_checked_then_the_triangle_is_frozen_without_gaps_or_overlaps
cargo test --test break_shots nine_ball_break_examples_use_frozen_rack_geometry
cargo test --test break_shots nine_ball_break_examples_open_the_rack_after_shared_contact
```

Then run all directly affected integration targets and compile migrated benchmarks:

```sh
cargo test --test n_ball_events --test n_ball_advance --test n_ball_simulation --test n_ball_pockets --test dsl --test advance_two_ball_events --test two_ball_simulation --test rail_event_scheduling --test rail_event_execution --test rack_and_displacement --test break_shots
cargo check --benches
```

Finally run the relevant aggregate suite:

```sh
cargo test
```

## Dependencies and overlap with sibling plans

- **No direct dependency:** this plan can land independently because it establishes state admissibility before collision timing or response.
- `plans/coupled-nonideal-shared-contacts.md` is related but distinct. That plan owns impulse resolution for valid simultaneous touching graphs at `g=0`; this plan owns invalid `g<0` inputs and bounded position-only numerical recovery. This plan must not reject or perturb exact frozen contacts needed by the coupled solver.
- `plans/curved-rolling-event-detection.md` is related but distinct. That plan owns first-contact timing from valid separated states (`g>0`); this plan guarantees it is not asked to schedule from materially penetrated initial geometry.
- If all three land together, apply this plan’s `Result` cutover to the final sibling API signatures rather than introducing intermediate compatibility wrappers. The physical ownership boundaries remain: admissibility here, valid-path timing in the curved-event plan, and valid-contact impulses in the shared-contact plan.

## Candidate commit message

```text
Reject overlapping N-ball states at public boundaries

Return indexed geometry errors for material penetration, recover only sub-microinch positional residue, and preserve exact frozen-rack contacts across core and DSL construction paths.
```
