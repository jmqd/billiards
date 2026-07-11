# Use canonical curved rolling trajectories for continuous event detection

Date: 2026-07-10

Severity: **High**  
Priority: **P1**

## Problem statement

The on-table integrator and the event predictors currently use different trajectories for a rolling ball with active TP B.2 side-spin turn. `raw_advance_within_phase_on_table` advances the ball along the repository's analytic curved rolling path, but ball-ball, rail, and jaw timing continue to solve a straight, fixed-direction quadratic path. The scheduler can therefore miss a real contact (tunneling) or schedule an impact whose canonically advanced states are not touching (a ghost event).

The fix must establish one private, phase-local trajectory representation as the source of truth for both state advancement and event-gap evaluation. True quadratic segments must retain their existing analytic root paths. Curved rolling segments require a continuous, adaptively bracketed entry solve with conservative exclusion bounds; an evenly spaced or fixed-count time scan is not an acceptable replacement.

This plan covers the one confirmed root issue across ball-ball circles, rail planes, and jaw circles. Pocket capture remains an audit/consistency follow-up because the current capture candidates are revalidated against the real advanced state and fall back to a real-trajectory scan; the audit did not substantiate a deterministic pocket-capture miss.

## Observed current behavior and impact

### The two trajectory definitions

- `raw_advance_within_phase_on_table` (`src/lib.rs:4646-4799`) is the actual state integrator. Its rolling branch (`:4692-4789`) can call `rolling_side_spin_curved_displacement` and rotate the translational heading while vertical-axis spin decays.
- `rolling_side_spin_turn_coefficient` and `rolling_side_spin_curved_displacement` (`src/lib.rs:12018-12073`) implement the current analytic TP B.2 turn and displacement.
- `raw_planar_acceleration_during_phase` (`src/lib.rs:5305-5342`) instead represents every rolling trajectory with one acceleration vector antiparallel to the initial velocity. That is valid for straight rolling but omits the lateral acceleration and rotating heading of an active TP B.2 turn.
- `RelativeQuadraticMotion` and its root machinery (`src/lib.rs:5344-5528`) correctly solve the path they are given: a quadratic relative center path, quartic squared-distance gap, cubic critical points, and bisection of the first entering monotonic interval. The defect is not in those polynomial roots; the defect is applying them to a non-quadratic path.
- `first_fixed_circle_entry_time_for_raw_motion` (`src/lib.rs:5530-5561`) makes the same fixed-direction assumption for contact with a stationary circle.

### Confirmed affected predictors

- **Ball-ball:** `compute_next_ball_ball_collision_during_current_phases_on_table` (`src/lib.rs:5769-5831`) builds a `RelativeQuadraticMotion` from the straight surrogate at `:5809-5820`, then constructs `a_at_impact` and `b_at_impact` by running the curved integrator at `:5822-5825`. Its computed root and returned states can therefore disagree geometrically.
- **Rails:** `rail_collision_gap_quadratic_coefficients`, `first_rail_collision_time_during_current_phase_raw`, and `compute_next_ball_rail_impact_on_table` (`src/lib.rs:5834-5966`) solve only a quadratic signed plane gap, then advance the returned state with the curved integrator.
- **Jaws:** `compute_next_ball_jaw_impact_on_table` (`src/lib.rs:8397-8472`) delegates every rounded- or point-jaw circle to `first_fixed_circle_entry_time_for_raw_motion`, then advances the returned state with the curved integrator.

### Direct callers and propagation

- `PocketAwareEventCache::refresh_ball` (`src/lib.rs:1995-2107`) caches all three affected event types, including pair entries at `:2076-2089` and jaw/rail entries at `:2058-2064`.
- `select_earliest_n_ball_event_from_states` (`src/lib.rs:8834-8895`) consumes the ball-ball and rail predictions; its public prediction wrappers are `compute_next_n_ball_event_on_table`, `compute_next_n_ball_event_with_rails_on_table`, and the two-ball compatibility wrappers (`:8897-8972`).
- `advance_to_next_n_ball_event_with_scheduler` and public on-table advance wrappers (`src/lib.rs:9933-10304`) advance all balls to the selected time and resolve the reported impact, magnifying a bad root into a missed or spurious impulse.
- `compute_next_n_ball_system_event_with_rails_and_pockets_on_table`, `resolve_n_ball_system_event_with_physics_and_pockets_on_table`, and the pocket-aware advance/simulation loop (`src/lib.rs:10728-11086`) consume cached ball-ball, rail, and jaw predictions.
- `PostContactContinuation::{next_collision_against_ball,next_collision_from_struck_ball_against_ball,next_event_against_ball,next_event_from_struck_ball_against_ball,next_rail_impact,next_event_against_ball_with_rails}` (`src/lib.rs:12257-12330`) expose the affected predictors after a collision.

A missed event lets a ball pass through another ball, cushion plane, or jaw. A ghost event applies an impulse at a state that does not satisfy its own contact geometry. Both corrupt event ordering, cached candidates, downstream collision response, and final shot traces.

## Physics and source evidence

### Governing equations adopted by the current integrator

TP B.2 gives the turning force, turn rate, and their relation as

$$
F_t=\frac{m v^2}{\rho},\qquad \Omega_t=\frac{v}{\rho},\qquad F_t=m v\Omega_t.
$$

Its rolling-turn model then gives

$$
\Omega_t(v)=\frac{T_s\sin\theta}{m v R\left(\frac25+\cos\theta\right)},
\qquad v(t)=v_0-a_r t,
$$

and integrates the heading through

$$
\psi(t)-\psi_0=\int_0^t\Omega_t(v(\tau))\,d\tau
=\operatorname{sgn}(\omega_z)\frac{K}{a_r}
\ln\!\left(\frac{v_0}{v(t)}\right)
$$

for the coefficient $K$ represented by `rolling_side_spin_turn_coefficient`. Position is the integral of $v(t)(\sin\psi(t),\cos\psi(t))$ in the repository's north-based heading convention. It is not $p_0+v_0t+\tfrac12a_0t^2$ while the heading turns.

For a ball-ball event, the required entry condition is

$$
g_{bb}(t)=\|p_B(t)-p_A(t)\|-(R_A+R_B)=0,
\qquad
\dot g_{bb}(t)<0.
$$

For a jaw centered at $c_j$ with nose radius $R_j$,

$$
g_j(t)=\|p(t)-c_j\|-(R+R_j)=0,
\qquad
\dot g_j(t)<0.
$$

For a rail with inward unit normal $n$ and legal center plane offset $d$,

$$
g_r(t)=n\cdot p(t)-d=0
$$

under the helper's sign convention, with the derivative directed into the rail. The exact sign representation may remain rail-specific, but positive values must mean separated and an accepted future root must be entering.

### Corpus and document locations

- `whitepapers/tp_b_2_rolling_resistance_spin_resistance_and_ball_turn.pdf:275-295` contains Eqs. 21-23; `:331-347` derives the turn rate in Eq. 30; `:367-417` gives its speed dependence, $v(t)=v_0-at$, and the Eq. 35 heading integral. The source's two examples appear at `:427-443`: 2 mph over 8 ft gives $0.305^\circ$ and 0.217 in lateral error, while 5 mph over 3 ft gives $0.012^\circ$ and 0.003811 in.
- The same document is explicit that the pressure-center and spin-down-torque construction is assumed (`:221-241`). This limits the model's claim to universality, but not the internal-consistency requirement: event timing must follow the model already selected by the integrator.
- `agent_knowledge/whitepapers_formula_candidates.txt:3626-3668` indexes the TP B.2 rolling coefficient/deceleration, spin-down torque, lateral force, and $F_t=mv\Omega_t$ evidence. This generated index is evidence only and is not an edit target.
- Cross qualitatively observes that a near-vertical-axis spinning ball curves with spin direction and that greater initial spin increases curvature (`agent_knowledge/whitepapers_corpus.txt:42593-42630`). Cross's measurements use a 22 mm golf ball on low-pile carpet, with calibration values at `:42663-42674`; they must not be used to recalibrate pool cloth in this change.
- `whitepapers/toward_a_competitive_pool_playing_robot.pdf:174-221` describes continuous-domain event prediction, an analytic quartic separation equation for the polynomial case, and the benefit of requiring no discrete time step. It supports preserving analytic event roots where the trajectory is actually polynomial; it does not support applying a quartic surrogate after the motion model becomes curved.

## Exact deterministic reproducers

All three confirmed fixtures use:

- ball radius $R=1.125\ \mathrm{in}$;
- rolling deceleration $a_r=5\ \mathrm{in/s^2}$;
- vertical-axis spin deceleration $\alpha_z=2\ \mathrm{rad/s^2}$;
- initial velocity $v=(0,10)\ \mathrm{in/s}$;
- no-slip horizontal spin $\omega_x=-10/R=-8.888888888\ldots\ \mathrm{rad/s}$ and $\omega_y=0$;
- vertical spin $\omega_z=+2\ \mathrm{rad/s}$.

The existing observable contract in `tests/advance_ball_state.rs:622-648` establishes that a ball starting at $(10,20)\ \mathrm{in}$ reaches

$$
p(1\ \mathrm{s})=(10.004702077725524,\ 27.499997782973136)\ \mathrm{in}
$$

with heading change $+0.09257711527464842^\circ$ and speed $5\ \mathrm{in/s}$.

### 1. Ball-ball tunneling fixture

- Ball A starts at $(10,20)\ \mathrm{in}$ with the common velocity and angular velocity above.
- Resting ball B is centered at exactly $(12.253702077725524,\ 27.499997782973136)\ \mathrm{in}$.
- At 1 s, the canonical curved separation is exactly $2.249\ \mathrm{in}<2R=2.25\ \mathrm{in}$, so continuity requires a first entering root before 1 s.
- The straight surrogate keeps $x_A=10\ \mathrm{in}$; its minimum possible horizontal separation is $2.253702077725524\ \mathrm{in}>2.25\ \mathrm{in}$ and it returns `None`.
- Evaluating the existing analytic curved equation gives a diagnostic reference root near $t=0.9873657442089809\ \mathrm{s}$. The regression must primarily assert the contact and closing invariants rather than make this many digits an algorithm-dependent contract.

### 2. Right-rail tunneling fixture

- On the default 50 in wide table, the right-rail legal center plane is $x=50-R=48.875\ \mathrm{in}$.
- Start the same rolling state at exactly $(48.871,20)\ \mathrm{in}$.
- The canonical curved state reaches $x(1)=48.875702077725524\ \mathrm{in}$ and has crossed the right-rail plane.
- The straight surrogate holds $x=48.871\ \mathrm{in}$ and finds no right-rail root.
- The existing analytic curved equation gives a diagnostic reference root near $t=0.9111862889917761\ \mathrm{s}$.

### 3. Center-right jaw tunneling fixture

- The default `TableSpec` is the 9 ft Brunswick GC IV (`src/lib.rs:14015-14018,14048-14068`). `pocket_jaw_reference_point_in_inches` places `Pocket::CenterRight/PocketJaw::First` at $(50,52.5)\ \mathrm{in}$ (`src/lib.rs:6013-6037`), and the default rounded nose radius is $0.125\ \mathrm{in}$ (`src/lib.rs:14139-14145`). The center-to-jaw contact radius is therefore $1.125+0.125=1.25\ \mathrm{in}$.
- Start the same rolling state at exactly $(48.746297922274476,\ 45.000002217026864)\ \mathrm{in}$.
- At 1 s the canonical state is exactly $(48.751,52.5)\ \mathrm{in}$, only $1.249\ \mathrm{in}$ from the jaw center, so a first entering root occurred before 1 s.
- The fixed-direction circle solver keeps $x=48.746297922274476\ \mathrm{in}$ and its minimum separation is $1.253702077725524\ \mathrm{in}>1.25\ \mathrm{in}$, so it returns `None`.
- The existing analytic curved equation gives a diagnostic reference root near $t=0.9904405018373671\ \mathrm{s}$.

### Ghost-event fixture

Add a complementary ball-ball regression derived from the same canonical curve, not from an enlarged collision radius:

- Keep ball A at $(10,20)\ \mathrm{in}$ with $\omega_z=+2\ \mathrm{rad/s}$.
- Put resting B at $(7.7505,\ 27.499997782973136)\ \mathrm{in}$.
- The straight surrogate enters the $2.25\ \mathrm{in}$ contact circle because its fixed horizontal offset is $2.2495\ \mathrm{in}$.
- The canonical curve bends A to the right, away from B; the curved path's closest center distance over $[0,1]\ \mathrm{s}$ is approximately $2.2541991383\ \mathrm{in}>2.25\ \mathrm{in}$, so the predictor must return `None`.

This fixture prevents a nominal tunneling fix from accepting every ambiguous interval or merely inflating contact geometry.

## Root cause

The implementation added TP B.2 curvature to advancement without introducing a trajectory abstraction shared by event prediction. Event code continued to infer the entire phase path from the initial acceleration vector and documented the relative path as unconditionally quadratic. That duplicated representation became false for curved rolling. The later polynomial critical-point fix removed the old 512-sample grazing miss for genuinely quadratic paths, but it cannot repair a polynomial built for the wrong path.

The mismatch is especially dangerous because the predictors calculate time on the surrogate and then build returned states with `raw_advance_within_phase_on_table`. There is no final geometry/closing postcondition before the event reaches the cache or impulse resolver.

## Non-goals

- Do not change the free-motion lifetime gate that currently suppresses TP B.2 curvature when vertical-axis spin outlives translation. That separate issue belongs to `plans/rolling-side-spin-lifetime.md`. This plan makes event timing follow whichever trajectory the canonical trajectory builder selects; it must not reproduce the lifetime decision in event code.
- Do not recalibrate rolling resistance, spin decay, center-of-pressure angle, or the TP B.2 turn coefficient.
- Do not add massé/swerve, airborne motion, vertical collision response, or a full cloth-contact model.
- Do not change ball-ball, rail, or jaw impulse laws, restitution, friction, shared-contact resolution, overlap recovery, or event tie priority.
- Do not replace continuous detection with a fixed number of samples, a fixed time step, or collision-radius/tolerance inflation.
- Do not claim a pocket-capture bug without a deterministic failing fixture. Pocket candidate consistency is audited only after the three confirmed event types work.
- Do not combine this correctness change with the broad raw-geometry/cache/performance work in `plans/performance_engineering.md`.

## Proposed private data model and cutover

Names in this section are proposed internal names; public API names and return types remain unchanged.

Introduce a private phase-local trajectory representation, for example:

```rust
enum RawPhaseTrajectorySegment {
    Stationary { /* initial state and duration */ },
    Quadratic { /* initial state, constant planar acceleration, duration */ },
    CurvedRolling { /* initial state, TP B.2 parameters, duration */ },
}
```

The representation must provide, without allocation in an individual evaluation:

- `state_at(local_time) -> RawOnTableBallState`, including position, velocity, horizontal no-slip spin, and decayed vertical spin;
- a finite segment duration and absolute phase-time offset;
- a conservative planar speed bound over any subinterval, used to prove contact exclusion;
- a discriminator that permits analytic polynomial roots only for `Quadratic`/`Stationary` combinations;
- equation boundaries inside a qualitative phase. In particular, a rolling phase whose current canonical motion curves until vertical spin stops and then rolls straight must be exposed as a curved segment followed by a quadratic segment, even though `MotionPhase` remains `Rolling`.

Build the segment list once from `RawOnTableBallState`, `MotionPhase`, radius, and `OnTableMotionConfig`. `raw_advance_within_phase_on_table` must delegate to the same segment evaluator used by event gaps rather than maintain a second TP B.2 formula branch. Event code must never independently decide whether rolling curvature is active. This makes the lifetime-gate plan change one canonical builder later instead of requiring another scheduler migration.

No public data-model migration is required:

- Preserve `PredictedBallBallCollision`, `PredictedBallRailImpact`, `PredictedBallJawImpact`, `NBallOnTableEvent`, and `NBallSystemEvent` shapes.
- Preserve all public predictor, advance, simulation, cache, DSL, and `PostContactContinuation` signatures.
- Atomically migrate the three predictors and their internal helpers to the canonical trajectory representation; do not leave a deprecated straight-path alias.
- Keep `RelativeQuadraticMotion`, `real_roots_quadratic`, `real_roots_cubic`, and the existing critical-point/refinement path for true quadratic segments.
- Any acceleration helper retained for pocket analytic candidates must be named/documented as a quadratic candidate approximation and must not be callable by the migrated ball-ball, rail, or jaw paths for a curved segment.

## Continuous curved-entry solver contract

Implement one private first-entry primitive used by curved or mixed curved/quadratic ball-ball, rail, and jaw gaps. Geometry-specific adapters supply signed gap, gap derivative, and conservative gap-rate/position bounds; they must all evaluate the canonical trajectory.

The solver must:

1. Search segment intervals in chronological order, splitting at every trajectory equation boundary from either participant. For ball-ball motion, use the sorted union of A and B boundaries.
2. Dispatch to the existing analytic solver when all trajectories in an interval are quadratic. A straight post-spin remainder must therefore regain the polynomial fast path.
3. For an interval containing curved rolling, use left-to-right adaptive interval subdivision with a conservative exclusion bound. For example, the distance-to-contact gap is Lipschitz-bounded by the sum of participant speed bounds; if a midpoint gap minus that rate bound times the half-width is still positive beyond spatial tolerance, the whole interval is proven contact-free. Rail and fixed-circle adapters use the corresponding projection/radial bounds.
4. Subdivide only intervals that cannot be excluded. This is adaptive continuous collision detection, not a disguised evenly spaced scan. There must be no constant such as `CURVED_EVENT_SCAN_STEPS`, no fixed sampling grid, and no correctness dependence on a chosen sample count.
5. Once an entering sign change is bracketed, refine the earliest root with safeguarded bisection or Brent-style refinement. Search the left child before the right child so a later root cannot mask an earlier one.
6. Treat an exact tangent minimum without inward normal speed as no impulse event, preserving the current ball-ball rolling-stop behavior. Preserve existing zero-time semantics: ball-ball requires inward relative normal velocity; rail/jaw may accept the already-supported acceleration-inward case when instantaneous normal speed is zero.
7. Never return `None` merely because an interval remains numerically unresolved. Continue conservative subdivision until it is excluded, bracketed, or reduced to the documented dimensional spatial/time tolerance where no representable inward interval exists. Do not manufacture a root from an unresolved interval.
8. Validate every candidate against fresh canonical states before constructing a public prediction. A failed postcondition is an internal solver defect, not a candidate to send to the impulse resolver.

Use dimensional tolerances: inches for contact gap, inches per second for normal speed, and seconds for root refinement. Do not compare squared-inch gaps directly to linear-inch tolerances.

## Required root postconditions

Every returned future ball-ball, rail, or jaw root must satisfy all applicable conditions after recomputing states through the canonical trajectory evaluator:

1. **Finite and bounded time:** `t.is_finite()`, $t\ge0$, and $t$ is no later than the current common event horizon/equation segment end within time tolerance.
2. **Canonical state identity:** the public `*_at_impact` state is exactly the canonical trajectory evaluation used by the gap solver at the reported time; independently advancing the initial state through the public motion API to the same time agrees within the configured numeric tolerance.
3. **Contact geometry:** ball-ball center distance is $R_A+R_B$; jaw center distance is $R+R_j$; rail center projection lies on its legal plane. Each residual is within a dimensional spatial tolerance tied to geometry scale.
4. **Entering contact:** for positive-time ball-ball roots, $(p_B-p_A)\cdot(v_B-v_A)<0$ beyond normal-speed tolerance. For jaw roots, $(p-c_j)\cdot v<0$. For rail roots, the signed plane-gap derivative points into the cushion. Existing documented zero-time acceleration-inward cases remain the only exception.
5. **First contact:** all earlier trajectory segments are certified contact-free, and the interval immediately before a positive root is nonpenetrating within tolerance. Multiple crossings return the earliest entering root.
6. **No ghost response:** if canonical geometry or entering direction fails, the predictor returns no event for that candidate and does not populate the event cache or invoke an impulse response.

These postconditions are both implementation assertions and observable regression-test invariants.

## Implementation phases

### Phase 1 — Add failing behavioral regressions first

1. In `tests/ball_collision_timing.rs`, add the exact ball-ball tunneling fixture and ghost fixture above. Assert `Some` and `None`, respectively; for the real contact assert $0<t<1\ \mathrm{s}$, center distance $2.25\ \mathrm{in}$, inward relative normal velocity, and agreement with independent public advancement.
2. Add the mirrored tunneling case with $\omega_z=-2\ \mathrm{rad/s}$ and B reflected about $x=10\ \mathrm{in}$ to prove handedness symmetry.
3. In `tests/rail_event_scheduling.rs`, add the exact right-rail fixture. Assert `Rail::Right`, $0<t<1\ \mathrm{s}$, $x=48.875\ \mathrm{in}$ at impact, inward signed rail velocity, and public-advancement agreement. Add the mirrored/opposite-spin curve-away case and require no right-rail event.
4. In `tests/n_ball_pockets.rs`, add the exact `Pocket::CenterRight/PocketJaw::First` fixture. Assert the jaw identity, $0<t<1\ \mathrm{s}$, center-to-jaw distance $1.25\ \mathrm{in}$, inward radial velocity, and public-advancement agreement. Add an opposite-spin curve-away near miss.
5. In `tests/n_ball_events.rs`, route the exact ball-ball fixture through `compute_next_n_ball_event_on_table` and `advance_to_next_n_ball_event_on_table`; assert that pair `(0,1)` is selected and resolved before the rolling transition. This proves the corrected root reaches the scheduler, not only the direct helper.
6. Keep `the_phase_aware_predictor_finds_a_grazing_collision_between_fixed_scan_samples` (`tests/ball_collision_timing.rs:270-303`) unchanged as a no-fixed-step/analytic-quadratic regression.

The three positive fixtures must fail against the current straight surrogate before implementation. The ghost fixture must catch accepting a surrogate root whose canonically advanced states are separated.

### Phase 2 — Extract the canonical phase trajectory

1. Extract construction and evaluation of stationary, quadratic, and curved-rolling segments from `raw_advance_within_phase_on_table` without changing its public behavior.
2. Make the existing integrator delegate to this evaluator. Preserve the exact endpoint and heading asserted by `tests/advance_ball_state.rs:622-648` before touching event roots.
3. Expose rolling vertical-spin-stop as an internal equation boundary when the current canonical trajectory changes there; preserve the qualitative `MotionPhase` transition horizon from `raw_compute_next_transition_on_table` (`src/lib.rs:4801-4880`).
4. Add private unit coverage for segment composition: evaluating the whole trajectory at $t$ must equal evaluating to an internal boundary and then evaluating the remainder; exact-boundary evaluation must be finite and continuous in position and velocity.
5. Do not alter the free-motion lifetime gate in this phase.

### Phase 3 — Implement semi-analytic first-entry detection

1. Add the shared gap/root adapter and adaptive exclusion/refinement primitive with the continuous-solver contract above.
2. Preserve `RelativeQuadraticMotion` as the analytic implementation for quadratic/quadratic ball-ball and fixed-circle intervals. Preserve quadratic rail roots for quadratic segments.
3. Add internal solver tests for an interval whose contact window is much narrower than a coarse 1/512 horizon division, for multiple crossings where the earliest must win, for an exact non-closing tangent, and for a curve-away interval that conservative bounds eventually exclude.
4. Centralize final candidate validation so ball-ball, rail, and jaw cannot diverge in finite-time, contact-residual, or entering-direction enforcement.

### Phase 4 — Migrate all three confirmed event families

1. Refactor `compute_next_ball_ball_collision_during_current_phases_on_table` to build canonical trajectories for both balls, merge their equation boundaries, dispatch each interval to analytic or curved entry detection, and construct both impact states from the accepted root evaluation.
2. Refactor `first_rail_collision_time_during_current_phase_raw`/`compute_next_ball_rail_impact_on_table` to use the same trajectory segments and signed plane-gap adapter. Continue comparing all four rail candidates by earliest accepted time.
3. Refactor `compute_next_ball_jaw_impact_on_table` to use the same trajectory segments and fixed-circle adapter for every pocket/jaw. Continue comparing all jaw candidates by earliest accepted time and preserve current zero-time closing/acceleration handling.
4. Remove or narrow helpers whose names/contracts still imply every phase is quadratic. Update the predictor rustdocs at `src/lib.rs:5754-5768` and rail docs at `:5908-5913`; they currently overstate that the within-phase relative path is always quadratic or that timing already uses the same motion model.
5. Exercise both scheduler paths: direct N-ball selection/execution and `PocketAwareEventCache` selection/execution. No cache schema change is necessary because the public predicted-event values remain the same.

### Phase 5 — Pocket-capture consistency audit, documentation, and cleanup

Only after ball-ball, rail, and jaw tests pass:

1. Audit `first_pocket_mouth_plane_crossing_time_during_current_phase_raw` and `first_pocket_back_plane_crossing_time_during_current_phase_raw` (`src/lib.rs:7183-7256`), `pocket_capture_gap_during_current_phase_raw` and the fallback scan (`:8288-8395`), and `compute_next_ball_pocket_capture_on_table` (`:8491-8647`). Record the observed distinction in code comments: radial/mouth/back candidate times can use the quadratic approximation, but candidate validation and fallback advancement use the actual trajectory.
2. Add a curved rolling pocket consistency test only if it protects an existing observable contract. If a deterministic capture miss or ghost is demonstrated, open a separate pocket-capture correctness plan with its own fixture; do not silently expand this implementation into pocket target/capture redesign.
3. Do not replace, tune, or optimize the 512-step pocket fallback here. That work overlaps `plans/performance_engineering.md` and is not evidence for the confirmed ball-ball/rail/jaw defect.
4. Update the completion note at `PHYSICS_TODOS.md:56-64` to state that actual motion and the three confirmed continuous event consumers now share the TP B.2 trajectory. Do not edit whitepaper PDFs, `agent_knowledge/whitepapers_corpus.txt`, or `agent_knowledge/whitepapers_formula_candidates.txt`; those are source/generated evidence rather than implementation artifacts.

## Regression and acceptance criteria

The change is accepted only when all of the following are observable:

- The three exact historical fixtures produce a ball-ball collision, `Rail::Right` impact, and `Pocket::CenterRight/PocketJaw::First` impact before 1 s.
- Every positive fixture satisfies the full root postconditions, including canonical contact geometry and inward normal motion.
- The exact ball-ball ghost fixture returns `None`; curve-away rail and jaw near misses also return no event.
- Mirroring $\omega_z$ and lateral geometry mirrors the event result and root time within tolerance.
- Straight rolling, sliding, rest/spinning, and straight post-spin subsegments continue using analytic polynomial roots. The existing narrow grazing collision between former scan samples remains exact and passing.
- No correctness path introduces a fixed step, fixed sample count, radius inflation, or tolerance inflation. Adaptive subdivision is driven by conservative interval exclusion and root bracketing.
- Direct predictors, N-ball selection/advance, and `PocketAwareEventCache` return geometrically consistent states at the same accepted time.
- A root at a trajectory equation boundary is returned once, with deterministic existing tie behavior; a non-closing tangent does not generate a zero-impulse event.
- Existing public APIs and event payloads remain source-compatible.

## Risks and edge cases

- **Internal rolling boundaries:** vertical spin can stop before the rolling-to-rest transition. Missing or double-searching this internal boundary can skip the straight remainder or duplicate a boundary root.
- **Two moving curved balls:** ball-ball intervals must use the union of both balls' boundaries and a relative speed bound, not assume only one participant curves.
- **Tangent versus entry:** an exact tangent has zero normal speed and must not trigger an impulse; an extremely shallow but genuinely entering contact must not be pruned as tangent. Use derivative and dimensional tolerances separately from gap tolerance.
- **Zero-time contacts:** preserve current ball-ball closing-only behavior and the existing rail/jaw acceleration-inward exception. The canonical refactor must not alter overlap/shared-contact policy.
- **Near rolling stop:** the TP B.2 logarithmic heading is singular as $v\to0$. Under the current lifetime gate, confirmed curved segments end at side-spin stop before translation stop. `plans/rolling-side-spin-lifetime.md` owns the free-motion endpoint policy; this solver must consume the canonical finite boundary and never independently evaluate beyond it.
- **Multiple entry/exit windows:** adaptive traversal must be chronological and conservative so a later, easier root cannot hide an earlier narrow entry.
- **Floating-point scale:** gap, derivative, and time tolerances have different units. Scale them by ball/table geometry and local speed instead of reusing one dimensionless epsilon.
- **Performance:** curved solving is more expensive than polynomial roots. Restrict it to intervals actually marked curved, retain analytic roots everywhere else, compute trajectory parameters once per candidate, and use conservative bounds to prune non-contact intervals. Correctness must not be traded for a fixed scan.
- **Cache/event ordering:** corrected times can change which event wins. Existing deterministic tie tolerances and jaw-versus-capture precedence remain authoritative after candidates satisfy their own geometry.
- **Pocket scope:** the pocket fallback is already expensive and only partially analytic. Do not conflate its performance/consistency questions with the substantiated jaw-circle miss.

## Dependencies and overlap

- **`plans/rolling-side-spin-lifetime.md`:** owns the free-motion branch where side spin survives until translation stops and the near-stop TP B.2 policy. This plan owns event-root consistency with the canonical path. Implementing the canonical trajectory abstraction first reduces duplication, but neither plan should copy the other's gate logic.
- **`plans/performance_engineering.md`:** overlaps pocket scan performance, raw geometry, and cache rebuilding. This correctness plan retains analytic fast paths and audits pocket consistency but does not optimize or redesign pocket capture.
- **`PHYSICS_TODOS.md:56-64`:** the completed TP B.2 item migrated free advancement but named rail and future ball-collision prediction as consumers. This plan completes that event-consumer cutover and adds jaw coverage.
- **`physics_audits/2026-04-24-event-geometry-multiball.md:54-76`:** the historical 512-sample grazing defect is already fixed for quadratic paths by cubic critical points and quartic-gap refinement. This plan must preserve that fix; it addresses the newer mismatch between a curved integrator and a straight polynomial surrogate.
- Shared-contact, overlap-validation, rail-response, pocket-geometry, and airborne-geometry plans may consume the corrected event states but must not duplicate the canonical trajectory/root work here.

## Verification commands

Run focused failing/passing regressions first, using the final test-name filters chosen during implementation:

```sh
cargo test --test ball_collision_timing curved_rolling
cargo test --test rail_event_scheduling curved_rolling
cargo test --test n_ball_pockets curved_rolling_jaw
cargo test --test n_ball_events curved_rolling
```

Then run the complete affected integration-test binaries:

```sh
cargo test --test advance_ball_state
cargo test --test ball_collision_timing
cargo test --test rail_event_scheduling
cargo test --test n_ball_events
cargo test --test n_ball_pockets
```

Finally run the relevant aggregate suites:

```sh
cargo test --lib
cargo test
```

## Candidate commit message

```text
Fix curved rolling event roots

Unify within-phase trajectory evaluation across advancement and ball-ball,
rail, and jaw timing. Preserve analytic roots for quadratic segments and
enforce canonical contact postconditions on adaptively bracketed curved roots.
```
