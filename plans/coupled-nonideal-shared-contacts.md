# Couple non-Ideal shared ball contacts without creating energy

**Date:** 2026-07-10  
**Severity:** High  
**Priority:** P1

## Problem statement

A simultaneous ball-ball contact graph is one impact problem, not a set of independent complete two-ball impacts. The current executor recognizes that fact only for `CollisionModel::Ideal`. For `ThrowAware` and `SpinFriction`, it resolves every pair from the same pre-pass snapshot, sums those mutually incompatible pair deltas on shared balls, applies the sum, and repeats. A symmetric, frictionless, perfectly elastic double hit therefore creates 75% kinetic energy even though the same geometry already has a conservative coupled solution in the Ideal branch.

The implementation must use one unilateral coupled normal-contact solve for every collision model. Non-Ideal friction must be derived from each contact's coupled normal impulse and solved as a bounded contact-space problem. No execution path may retain the summed standalone-pair fallback. Until a required part of the generalized friction solve is available, the executor must stop at an explicit unsupported shared-contact diagnostic and leave the impact state unchanged rather than emit a knowingly nonphysical state.

This plan distinguishes current observations from proposed design. The equations below define the required behavior; the specific proposed Rust names may be adjusted during implementation if an existing nearby convention provides a better name, but the fail-closed and invariant contracts are not optional.

## Current behavior and impact

### Observed execution

`resolve_shared_zero_time_ball_ball_contacts_on_table` and `resolve_shared_zero_time_ball_ball_contacts_in_system_states` currently:

1. collect and canonicalize all zero-time touching/closing pairs;
2. use `coupled_ideal_shared_ball_ball_contact_deltas_from_state_refs` only when `collision_model == CollisionModel::Ideal`;
3. otherwise clone a snapshot;
4. call the complete binary `collide_ball_ball_on_table_with_radius_and_config` for every pair against that unchanged snapshot;
5. sum each pair's linear and angular changes into `OnTableKinematicDelta` for shared balls;
6. apply all summed deltas simultaneously and repeat for at most `SHARED_BALL_BALL_CONTACT_RESOLUTION_PASSES == 8`.

Each binary response assumes its pair receives the full impulse permitted by its pre-impact relative velocity. When one ball belongs to two contacts, both pair solves spend the same incoming momentum and kinetic energy. Later passes do not repair the first pass once every contact is separating.

The public event description acknowledges the approximation through `SharedBallBallContactResolution::CoupledIdealOrIterativePairwiseApproximation`, but disclosure is not a physical safeguard. Callers receive ordinary post-impact states and can neither reject nor distinguish an energy-creating non-Ideal result.

### Impact

- A rack or cluster can gain macroscopic translational and rotational energy at a zero-time batch.
- Break spread, rail arrival, pocket outcomes, and DSL shot previews can depend on ball indexing and pair iteration order rather than only on the physical state.
- `CollisionModel::ThrowAware` does not reduce to the correct Ideal solution when friction is configured to zero.
- The eight-pass cap is not a convergence criterion and does not establish complementarity, separation, conservation, or a Coulomb bound.
- Existing break tests can pass despite the defect because they assert only that a shared/collision event occurs and enough object balls move.

## Exact affected files, symbols, and callers

Line ranges describe the audited 2026-07-10 source and should be re-anchored immediately before implementation.

### Core data model and solvers

- `src/lib.rs:1635-1649` — `SharedBallBallContactResolution` and its stale `coupled_ideal_or_iterative_pairwise_approximation` public string.
- `src/lib.rs:1651-1714` — `NBallOnTableEvent::SharedBallBallContact`, `time`, and `primary_ball`; rustdoc currently promises an Ideal coupled solve and non-Ideal iterative approximation.
- `src/lib.rs:2359-2439` — `NBallSystemEvent::SharedBallBallContact` and `is_terminal_diagnostic`; only unsupported airborne ball-ball contact is currently terminal.
- `src/lib.rs:9009-9023` — simultaneous-event tolerances, the fixed eight-pass constant, and `shared_ball_ball_contact_resolution`.
- `src/lib.rs:9066-9114` — zero-time pair discovery and canonical pair ordering. This should remain contact-graph construction, not become an impulse solver.
- `src/lib.rs:9413-9512` — collision-delta comparison, `OnTableKinematicDelta`, snapshot-derived delta accumulation, and application.
- `src/lib.rs:9514-9567` — `SharedIdealBallBallContact`, contact extraction, and unit normal-impulse mapping.
- `src/lib.rs:9569-9623` — the current dense linear solver.
- `src/lib.rs:9625-9695` — `coupled_ideal_shared_ball_ball_contact_deltas_from_state_refs`, the existing coupled normal matrix solve and negative-impulse active-set gate.
- `src/lib.rs:9698-9802` — defective on-table non-Ideal fallback.
- `src/lib.rs:9804-9931` — identical defective richer-system fallback.
- `src/lib.rs:11578-11830` — binary Ideal and frictional response equations, including `contact_friction_impulse_per_mass`, the `|slip|/7` no-slip cap, `mu * J_n` cap, transferred spin, optional Kim table correction, and `CollisionDiagnostics`. Reuse the contact law and sign conventions through generalized impulse mappings; do not call the complete binary outcome once per edge of a shared graph.
- `src/lib.rs:11876-11965` — public direct binary collision APIs. Their one-contact observable behavior must remain covered and must agree with the coupled solver's one-contact limit.

### Ordinary N-ball execution and simulation

- `src/lib.rs:9933-10048` — `advance_to_next_n_ball_event_with_scheduler`; it invokes the shared resolver both for an explicit `SharedBallBallContact` and after an ordinary primary ball-ball collision exposes touching neighbors.
- `src/lib.rs:10053-10179` — public N-ball advance wrappers with default or explicit ball-ball/rail coefficients.
- `src/lib.rs:10522-10688` — until-rest loop and public `simulate_n_balls_*_until_rest` wrappers. The ordinary loop currently has no terminal-diagnostic shared-contact branch.

### Pocket-aware/richer-system execution

- `src/lib.rs:10742-10886` — `resolve_n_ball_system_event_with_physics_and_pockets_on_table`; it invokes the system shared resolver for explicit shared events and post-binary zero-time cascades but returns only states, so it cannot currently report resolution failure.
- `src/lib.rs:10888-10987` — public pocket-aware advance wrappers.
- `src/lib.rs:10990-11173` — pocket-aware/system simulation wrappers and event loop. `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit` already stops on `is_terminal_diagnostic`, providing the model for unsupported shared contact.
- `src/lib.rs:11155-11224` — on-table-to-system pocket-aware simulation wrappers.

### DSL, trace, and rack/break callers

- `src/dsl.rs:131-197` — named simulation physics selects `CollisionModel` and `BallBallCollisionConfig` used by traces.
- `src/dsl.rs:303-365` — shot states and `simulate_shot_system_with_physics_on_table_until_rest` route scenarios into the richer-system simulator.
- `src/dsl.rs:384-471` — shot-trace until-rest/event-limit entry points.
- `src/dsl.rs:655-687` — trace replay calls `resolve_n_ball_system_event_with_physics_and_pockets_on_table`; this caller must consume the resolver's actual resolved-or-unsupported event, not assume a state-only success.
- `src/dsl.rs:1032-1110` — `ScenarioShotTraceEventKind::SharedBallBallContact` and human formatting expose the obsolete resolution string.
- `src/dsl.rs:1415-1523` — system-event participation and trace-event conversion must add unsupported shared contact and carry the successful coupled strategy.
- `tests/break_shots.rs:99-155` — `nine_ball_break_examples_open_the_rack_after_shared_contact` directly advances frozen racks with `CollisionModel::ThrowAware` and `BallBallCollisionConfig::human_tuned`; it currently checks event presence and movement only.
- `tests/break_shots.rs:157-215` — default DSL break traces use the same ThrowAware/human-tuned path and assert rail hits and spread only.
- `examples/scenarios/nine_ball_break_head_rail.billiards` and `examples/scenarios/nine_ball_break_left_side_rail.billiards` — concrete rack/break inputs exercised by those callsites. `golden_break_cut_break.billiards` is also used to validate frozen geometry.

### Existing tests to migrate or extend

- `tests/n_ball_events.rs:253-289` — predicted shared-event geometry and the obsolete resolution string.
- `tests/n_ball_advance.rs:449-496` — slipping ThrowAware frozen line checks only that the TP B.29 special case is skipped; it does not assert conservation.
- `tests/n_ball_advance.rs:499-553` — coupled symmetric shared-contact coverage exists only for `CollisionModel::Ideal` and a future contact after cloth deceleration.
- `tests/n_ball_pockets.rs:57-133` — ordinary/system event equivalence includes `SharedBallBallContactResolution` and must cover the new unsupported variant/outcome.
- `tests/n_ball_simulation.rs:176-224` — TP B.29 frozen-line simulation coverage must remain valid.
- `tests/ball_collisions.rs` — existing binary friction/throw/spin tests protect the one-contact limit during extraction of shared contact-space helpers.

## Whitepaper and source evidence

### Impulse, restitution, and friction

Doménech, *Non-smooth modelling of billiard- and superbilliard-ball collisions*, International Journal of Mechanical Sciences 50 (2008), pp. 752-763:

- pp. 1-3, Introduction and §2.1, Eqs. (1)-(4): an impact is represented by instantaneous normal and tangential impulses equal to changes in linear/angular momentum.
- Eq. (7): normal restitution is the negative ratio of post- to pre-impact normal relative speed.
- Eq. (8): the normal impulse is proportional to `(1+e)` and the incoming normal component.
- The introduction states the continuous-sliding tangential-to-normal impulse ratio equals the friction coefficient, supporting a Coulomb bound rather than an unconstrained sum.
- Extracted corpus locations: `agent_knowledge/whitepapers_corpus.txt:35253-35299` identifies the peer-reviewed source and impulse/friction framing; `:35319-35397` contains §2.1 and momentum Eqs. (1)-(6); `:35398-35420` contains restitution Eqs. (7)-(8) and the vertical-contact limitation.
- Direct extracted source locations: `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf`, pp. 1-3, especially extracted lines 19-57 and 69-137.

This paper is a binary/sequential billiards formulation. It supports the impulse, restitution, momentum, and friction constraints used per contact; it is not evidence that independently summing binary outcomes solves an arbitrary N-contact graph.

### Frozen-contact force propagation

Alciatore, TP B.29, *Simulation of a CB striking two frozen OBs along their line of centers*:

- The worked simulation integrates the simultaneous forces `F12` and `F23` in one three-ball system rather than completing collision 1-2 before collision 2-3.
- Its displayed normalized final speeds are approximately `(-0.071, 0.076, 0.995)` and it explicitly checks `v1+v2+v3=1` and `v1^2+v2^2+v3^2=1`.
- Direct extracted source locations: `whitepapers/tp_b_29_simulation_of_a_cb_striking_two_frozen_obs_along_their_line_of_centers.pdf:118-264` for the simultaneous force/equation loop and `:313-363` for final speeds, momentum, energy, collision duration, and conclusions.
- The generated corpus does not contain the standalone TP body; it cross-references TP B.29 at `agent_knowledge/whitepapers_corpus.txt:31918-31921,31998-31999`. `agent_knowledge/whitepapers_index.jsonl:360` is the source index entry. The PDF extraction above is therefore the authoritative local text for this evidence.

TP B.29 is an Alciatore technical proof/worked Hertz-contact simulation for one elastic, collinear, frozen three-ball layout. It corroborates simultaneous force propagation and conservation in that layout; it does not prove a general rigid multicontact closure. Preserve the current TP B.29 special-case behavior unless a separately validated model deliberately supersedes it.

### Simulator-level conservation

Greenspan et al., *Toward a Competitive Pool-Playing Robot*, p. 5 “Physics simulation,” says the physics model conserves linear and angular momentum. Direct extracted source location: `whitepapers/toward_a_competitive_pool_playing_robot.pdf:174-221`, especially lines 178-180. This is a system-level requirement, not a derivation of a frictional multicontact algorithm.

## Governing equations and required invariants

For a set of equal-mass ball contacts, let generalized pre-impact velocity be `qdot^-`, let `G_n` map it to signed contact-normal relative velocities `u_n^- = G_n qdot^-` (negative means closing), and let `M` be the generalized mass/inertia matrix. In the current per-unit-mass representation,

`A_n = G_n M^-1 G_n^T`.

For a uniform Newton restitution `0 <= e <= 1`, the unilateral normal contact problem is

`w = A_n lambda_n + (1+e) u_n^-`,

with

`lambda_n >= 0`, `w >= 0`, and `lambda_n_i * w_i = 0` for every contact.

The generalized normal velocity update is

`Delta qdot_n = M^-1 G_n^T lambda_n`.

For an active contact, `w_i=0` gives `u_n_i^+ = -e u_n_i^-`. An inactive edge has no tensile impulse and must not remain closing beyond tolerance. A solver is successful only when primal, dual, and complementarity residuals are all within a scale-aware tolerance; “ran eight passes” is not success.

For each contact's two-component friction impulse `lambda_f_i = (lambda_t_i, lambda_z_i)`, require the Coulomb disk

`sqrt(lambda_t_i^2 + lambda_z_i^2) <= mu_i lambda_n_i`.

The friction impulse must oppose the current contact-slip vector and may stop slip, but must not reverse it merely to spend the entire Coulomb budget. In a coupled graph, shared-ball translation and spin make the friction effective-mass matrix non-diagonal. Therefore compute all friction impulses from the coupled `lambda_n`, apply them through generalized impulse Jacobians, and use a converged projected/block contact solve (including normal reprojection where normal/tangent cross-coupling exists). Do not calculate a complete binary friction outcome per edge and add its state delta.

For a solid sphere, use `I/m = 2R^2/5` in the generalized energy metric. The existing binary `|slip|/7` cap should emerge as the one-contact equal-sphere limit of the effective-mass solve, not be independently applied to every edge in a shared graph.

Every successful ball-only batch with external table coupling disabled must satisfy:

1. all state components and contact impulses are finite;
2. no tensile normal impulse: `lambda_n_i >= -tol`;
3. no unresolved closing active/touching contact beyond tolerance;
4. restitution/complementarity residuals are within the documented tolerance;
5. each friction impulse satisfies its Coulomb disk and opposes slip;
6. total linear momentum is conserved;
7. ball-contact angular impulse bookkeeping is equal-and-opposite about the common contact point;
8. total translational plus rotational kinetic energy does not increase for `0 <= e <= 1` and `mu >= 0`;
9. `mu=0` gives exactly the coupled normal solution for `Ideal`, `ThrowAware`, and `SpinFriction` within numerical tolerance;
10. positions are unchanged by the instantaneous impulse solve;
11. unpermuting physically identical inputs gives the same states, active contacts, and resolved/unsupported status.

If the centralized Kim-domain policy from `plans/kim-correction-domain.md` marks an object/table correction **applied**, the table supplies an external impulse and ball-only horizontal momentum is not an applicable invariant. Such a shared batch must use an explicitly validated table-coupled generalized model or return unsupported; it must not silently pass ball-only checks by weakening them. If that policy marks the correction **skipped** (for example, `SkippedObjectNotStationary`), retain the ordinary coupled ball-ball response with the Kim term exactly zero and carry the skip reason in diagnostics.

## Deterministic symmetric three-ball reproducer

Use touching pool balls with `R = 1.125 in`, `e = 1`, `mu = 0`, zero angular velocity, and no elapsed free-motion interval:

- shared/cue ball 0: center `(0, -sqrt(3) R) in = (0, -1.9485571585...) in`, velocity `(0, 10) in/s`;
- left object ball 1: center `(-R, 0) in = (-1.125, 0) in`, velocity `(0, 0) in/s`;
- right object ball 2: center `(R, 0) in = (1.125, 0) in`, velocity `(0, 0) in/s`;
- model: `CollisionModel::ThrowAware` (and separately `SpinFriction`);
- config: `BallBallCollisionConfig::new(Scale::from_f64(1.0), Scale::zero())`.

The contact normals from ball 0 to balls 1 and 2 are

`n_L = (-1/2, sqrt(3)/2)`, `n_R = (1/2, sqrt(3)/2)`.

Each standalone equal-mass pair solve sees incoming normal speed `5 sqrt(3) = 8.6602540378... in/s` and, for `e=1`, applies that full per-mass impulse. Summing both binary outcomes produces the currently observed erroneous velocities:

- cue: `(0, -5) in/s`;
- left: `(-5sqrt(3)/2, 15/2) = (-4.3301270189..., 7.5) in/s`;
- right: `(5sqrt(3)/2, 15/2) = (4.3301270189..., 7.5) in/s`.

For equal masses, use `K* = 2K/m = sum_i |v_i|^2`, with units `(in/s)^2`. Before impact,

`K*^- = 10^2 = 100 (in/s)^2`.

After the current fallback,

`K*^+ = 5^2 + 2[(5sqrt(3)/2)^2 + (15/2)^2] = 175 (in/s)^2`.

Momentum remains `(0, 10)m in/s`, but kinetic energy increases by `(175-100)/100 = 75%`. Both contacts then separate, so the remaining fixed passes keep the bad result.

### Exact required Ideal-limit solution

The coupled normal equations in per-mass units are

`[[2, 1/2], [1/2, 2]] [lambda_L, lambda_R]^T = [10sqrt(3), 10sqrt(3)]^T`.

Thus

`lambda_L = lambda_R = 4sqrt(3) = 6.9282032303... in/s`,

and the exact post-impact velocities are

- cue: `(0, -2) in/s`;
- left: `(-2sqrt(3), 6) = (-3.4641016151..., 6) in/s`;
- right: `(2sqrt(3), 6) = (3.4641016151..., 6) in/s`.

They give momentum `(0,10)m in/s` and

`K*^+ = (-2)^2 + 2[(-2sqrt(3))^2 + 6^2] = 4 + 48 + 48 = 100 (in/s)^2`.

For the same symmetric fixture at general restitution `e`, the exact zero-friction coupled solution is

- `lambda_L = lambda_R = 2sqrt(3)(1+e) in/s`;
- cue `v_y^+ = 4 - 6e in/s`;
- left/right `v_x^+ = +/-sqrt(3)(1+e) in/s`, `v_y^+ = 3(1+e) in/s`;
- `K*^+ = 40 + 60e^2 (in/s)^2 <= 100 (in/s)^2` for `0 <= e <= 1`.

These exact values are the zero-friction acceptance oracle. A different multicontact restitution law must not be substituted silently; changing the law requires an explicit model/API decision and new source justification.

## Explicit non-goals

- Do not redesign continuous collision timing or curved rolling trajectories; `plans/curved-rolling-event-detection.md` owns future-contact path consistency. This plan consumes its zero-time contact set.
- Do not add positional depenetration or accept grossly overlapping input. `plans/reject-overlapping-ball-states.md` owns public `g < 0` rejection and tiny bounded roundoff recovery. This plan owns admissible touching graphs at `g = 0`.
- Do not generalize the TP B.29 collinear compliant three-ball special case into evidence for arbitrary racks. Preserve its currently tested, tightly gated domain.
- Do not solve simultaneous ball-rail, ball-jaw, or ball-pocket constraint graphs in this change. If such a mixed graph is detected, preserve current event ordering or diagnose it explicitly; do not claim the ball-only solver covers it.
- Do not recalibrate restitution, the Marlow friction curve, cloth coefficients, or break presets.
- Do not hide solver failure by clipping output energy, renormalizing velocities, increasing the pass cap, or falling back to pairwise impulses.
- Do not hand-edit `agent_knowledge/*`; it is generated corpus/index data.

## API and data-model cutover

Make a clean cutover; leave no alias for the obsolete approximation string or fallback.

1. Replace `SharedBallBallContactResolution::CoupledIdealOrIterativePairwiseApproximation` with explicit states that distinguish prediction from completed execution:
   - `PendingCoupledSolve` for a contact graph returned by a pure predictor that has no collision model/config;
   - `CoupledNormal` for a successful zero-friction solve;
   - `CoupledNormalWithBoundedFriction { iterations, max_residual }` for a successful projected friction solve.
   The exact diagnostics may use a small struct rather than enum fields, but successful advances and simulation logs must never report `PendingCoupledSolve`.
2. Add `SharedBallBallContactImpulse` diagnostics aligned with `ball_ball_pairs`, containing at least the normal and two friction impulse-per-mass components plus the effective friction coefficient. This makes Coulomb bounds and permutation behavior observable without reconstructing an underdetermined rack impulse vector from ball deltas.
3. Add `UnsupportedSharedBallBallContact { time_until_contact, ball_indices, ball_ball_pairs, reason }` to both `NBallOnTableEvent` and `NBallSystemEvent`. Define a stable reason enum including unsupported reduced vertical contact, unsupported table-coupled friction, singular/infeasible normal solve, and non-convergence. Add `time`, `primary_ball`, and `is_terminal_diagnostic` handling.
4. Change `resolve_n_ball_system_event_with_physics_and_pockets_on_table` from a state-only `Vec<NBallSystemState>` return to the existing `NBallSystemAdvance` shape (or an equivalently explicit resolved-event outcome) so a supplied predicted shared event can become either a successful resolved event with diagnostics or an unsupported terminal event. Return the advanced-to-contact but otherwise unmodified states on unsupported contact.
5. Make `advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table` return that resolver outcome directly. Make both simulation loops record the actual resolved event and stop on unsupported shared contact. Add `NBallOnTableEvent::is_terminal_diagnostic` to mirror the richer system.
6. Update `ScenarioShotTraceEventKind` with an unsupported shared-contact variant and successful coupled resolution/impulse diagnostics. Migrate `scenario_event_involves_ball`, `scenario_event_kind_from_system_event`, `format_human`, trace replay, and event-equivalence tests. Human output must say either `coupled normal`, `coupled normal + bounded friction`, or `unsupported shared contact: <reason>`; remove the old approximation label.
7. Rename private Ideal-only symbols to model-independent contact terms, for example `SharedBallBallContact`, `shared_ball_ball_contact_from_state_refs`, and `solve_coupled_shared_ball_ball_contacts`. Replace the two duplicated state-shape resolvers with one shared contact solve and thin on-table/system application adapters.
8. Remove `SHARED_BALL_BALL_CONTACT_RESOLUTION_PASSES`, the snapshot/pairwise loops, and `OnTableKinematicDelta::add_from_collision_pair_state` after the coupled path passes focused smoke tests. Do not retain them as a fallback.
9. The generalized solver delta must include vertical center-of-mass velocity and all angular components once `plans/ball-collision-vertical-impulse.md` lands. It must not squeeze the vertical impulse back through the current five-component `OnTableKinematicDelta`. Before that dependency is complete, any shared graph with material vertical contact slip must produce `UnsupportedSharedBallBallContact::UnsupportedReducedVerticalContact`.
10. Consume the centralized Kim eligibility/outcome helper from `plans/kim-correction-domain.md`; do not duplicate its stationary-object predicate in multicontact code. A skipped Kim correction proceeds through the coupled ball-only solve with the Kim term zero and reports the skip diagnostic. An applied Kim correction remains unsupported for shared graphs until its external table impulse is represented in the same generalized solve with validated invariants. Binary collision APIs keep the Kim plan's contract.

All public callsites in `src/lib.rs`, `src/dsl.rs`, `tests/n_ball_pockets.rs`, `tests/break_shots.rs`, CLI/preview trace construction, and any example-facing trace formatting must migrate in the same cutover. Do not leave compatibility aliases or silently discard the resolver's updated event.

## Phased implementation plan

### Phase 1 — Lock the failure and the fail-closed contract with tests

1. Add the exact time-zero three-ball fixture to `tests/n_ball_advance.rs` using `R = 1.125 in`, `ThrowAware`, `e=1`, `mu=0`, and zero spin.
2. Assert a `SharedBallBallContact`, elapsed `0 s`, pair set `{(0,1),(0,2)}`, exact velocities `(0,-2)`, `(-2sqrt(3),6)`, `(2sqrt(3),6)`, momentum, symmetry, separation, and `K*=100 (in/s)^2`. The test must fail against the current `K*=175` result.
3. Exercise the same fixture through `advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table` and assert identical on-table states and event diagnostics.
4. Add a temporary-to-permanent unsupported contract test for a non-Ideal shared graph whose vertical friction or **eligible/applied** table-coupled correction cannot yet be represented: event time is `0`, states are unchanged at contact, the reason is explicit, and both until-rest loops terminate instead of selecting the same zero-time event forever. Add the complementary Kim-domain case where the centralized policy reports `SkippedObjectNotStationary`: the Kim term is zero, the coupled ball-only solve proceeds, and diagnostics retain the skip reason.
5. Extend the event prediction test so pure prediction reports only graph geometry plus `PendingCoupledSolve`; successful advance must replace pending status.

### Phase 2 — Generalize and harden the coupled normal solve

1. Build one immutable contact array from the sorted/deduplicated zero-time pair set. Store indices, normal basis, tangent basis, pre-impact normal speed, and pre-impact two-component contact slip once; stop recomputing pair collisions from cloned states on every pass.
2. Generalize `coupled_ideal_shared_ball_ball_contact_deltas_from_state_refs` to every `CollisionModel`. The normal law depends on `collision_config.normal_restitution`, not on whether throw/spin friction is requested.
3. Solve the unilateral LCP/convex contact problem with active-set or rank-aware projected methods. Check both negative active impulses and closing/infeasible inactive contacts; the current “remove the most negative impulse” loop alone is insufficient for singular or redundant rack graphs.
4. Base success on scale-aware complementarity residuals, not an iteration count. For singular but physically redundant contact matrices, compute a feasible minimum-norm impulse or otherwise a unique generalized velocity update; if feasibility/convergence cannot be established, return unsupported without mutation.
5. Apply the generalized normal impulse once to each ball. Route `Ideal`, `ThrowAware`, and `SpinFriction` with `mu=0` through this exact same path.
6. Add focused solver/unit tests for one contact, disjoint contacts, the symmetric graph, an inactive unilateral edge, a redundant/singular graph, and non-convergence/failure conversion to the unsupported public event.

### Phase 3 — Add jointly bounded friction

1. Factor the binary contact-slip coefficient selection from `BallBallFrictionModel` so the shared solver can evaluate `mu_i` from each contact's pre-impact slip without constructing a complete binary outcome.
2. Construct generalized tangent/vertical Jacobians and their effective-mass/cross-coupling blocks using `I/m = 2R^2/5`. Warm-start from the coupled normal solution.
3. Solve friction contact blocks jointly with per-contact projection onto `||lambda_f_i|| <= mu_i lambda_n_i`. Reproject/re-solve normal blocks when friction cross-coupling changes normal residuals. Stop only when normal complementarity, Coulomb, slip-direction, and state-update residuals all converge.
4. Preserve the binary one-contact limit: the projected solve must match the existing planar throw direction and `min(mu J_n, |slip|/7)` magnitude when the vertical/full-state model is within its supported domain.
5. Consume the full-state vertical impulse/delta contract from `plans/ball-collision-vertical-impulse.md`. Until available, fail closed for material vertical slip; after migration, include `Delta v_A,z = -j_z`, `Delta v_B,z = +j_z` and the corresponding contact torques rather than discarding vertical COM motion.
6. Consume the centralized Kim-domain decision from `plans/kim-correction-domain.md`. Do not reimplement its predicate: proceed with a zero Kim term when it reports a skipped correction, and return explicit unsupported only when it reports an applied external table correction that the shared generalized solve cannot yet represent.
7. Emit per-contact impulse diagnostics only after convergence and apply the final generalized delta once. Validate finite states, energy, momentum where applicable, and all residuals before committing mutation.

### Phase 4 — Integrate both executors and migrate all callsites

1. Make both current resolver adapters call the same coupled core and differ only in how on-table entries are read/applied among richer system states.
2. Rewrite returned predicted shared events with the successful resolution and impulse diagnostics, or with `UnsupportedSharedBallBallContact`; never return `PendingCoupledSolve` from an advance.
3. Migrate pocket-aware advance/simulation, ordinary until-rest simulation, DSL shot simulation, DSL trace replay, scenario event conversion/formatting, and event-equivalence helpers to the resolved-event API.
4. Preserve disjoint-pair batching and the narrowly gated TP B.29 collinear result. Confirm they do not enter the new unsupported path.
5. Smoke-test the exact fixture through ordinary advance, system advance, ordinary simulation, system simulation, and DSL trace formatting before cleanup.

### Phase 5 — Rack, permutation, and boundary regression coverage

1. For `e in {0, 0.5, 0.95, 1}`, run the symmetric zero-friction fixture and assert the exact general solution above, `K*=40+60e^2`, momentum, complementarity, and no closing contact.
2. Run all six permutations of the three physical balls for each collision model in `{Ideal, ThrowAware, SpinFriction}` at `mu=0`. Unpermute by physical identity/initial position and compare post-impact states, active physical pair set, impulses mapped by pair identity, energy, and event status. Also reverse/mirror the x geometry to detect handedness.
3. Add a nonzero planar-slip/spin fixture. Assert per-contact Coulomb disks, friction opposing slip, total translational-plus-rotational energy non-increase, ball-only momentum, torque bookkeeping, finite states, convergence diagnostics, and all six input permutations. Use an unsupported expectation instead if the full generalized path is not yet available; never accept pairwise output.
4. Add one-contact equivalence against `collide_ball_ball_detailed_on_table_with_radius_and_config`, plus a disjoint-pair case proving independent batches retain existing results.
5. Strengthen `tests/break_shots.rs` to measure generalized system kinetic energy immediately before and after every zero-time `SharedBallBallContact`. With ball-only human-tuned physics, prohibit gain beyond a scale-aware roundoff tolerance and require no pending resolution. If a requested external/vertical model is unsupported, assert the explicit terminal event rather than rack spread.
6. Retain the existing rack movement/rail/spread assertions only after the conservation check. A lively break is not evidence of a correct impact solve.
7. Add an event-limit/trace replay test showing the event recorded by simulation and replayed by DSL has the same resolved strategy, impulses, and states.

### Phase 6 — Cleanup and source documentation after focused smoke passes

1. Delete both snapshot/delta fallback loops, the fixed pass constant, obsolete pair-delta accumulation, and stale approximation comments/string.
2. Update rustdoc for `CollisionModel`, `SharedBallBallContactResolution`, both N-ball event enums, resolver/advance APIs, binary detailed collision APIs, and DSL human event text to state the coupled normal/friction and unsupported contracts.
3. Update `PHYSICS_TODOS.md` only if it is being used as the completion ledger at implementation time; record completion only after all acceptance tests pass. Preserve historical audits as historical records rather than rewriting their old observations.
4. No whitepaper files or generated `agent_knowledge/*` artifacts need changes: the implementation cites already indexed local sources and introduces no source document. If source metadata is changed independently, regenerate through the repository's owning corpus workflow rather than hand-editing generated files.

## Regression and acceptance tests

The implementation is accepted only when all of the following are observable:

- The exact `ThrowAware`, `e=1`, `mu=0` symmetric fixture returns elapsed `0 s`, shared pairs `(0,1)` and `(0,2)`, the exact coupled velocities, and `K*=100 (in/s)^2`; the old `175` result is impossible.
- The richer-system executor returns the same states and contact outcome as the ordinary executor.
- `ThrowAware` and `SpinFriction` at `mu=0` agree with `Ideal` for the same restitution and contact graph.
- Restitution fixtures `e={0,0.5,0.95,1}` match the analytic symmetric solution and never gain energy.
- All six input permutations, after mapping back to physical identities, agree within documented numerical tolerance in states, pair impulses, active contacts, and status.
- A nonzero-friction supported fixture satisfies `sqrt(j_t^2+j_z^2) <= mu j_n`, does non-positive friction work, conserves applicable ball-only momentum/angular bookkeeping, and does not increase total translational-plus-rotational energy.
- A nonzero-friction configuration outside the implemented generalized domain returns an explicit unsupported terminal event with impact states unmodified.
- No successful advance or simulation log contains `PendingCoupledSolve`.
- A failed/singular/non-converged solve never falls back to standalone pair impulses and never returns partially mutated states.
- A successful batch leaves no touching contact closing beyond tolerance and reports bounded complementarity/convergence residuals.
- Existing one-contact, disjoint simultaneous pair, TP B.29 frozen-line, pocket-aware parity, break geometry, and break trace behavior remain valid within their documented domains.
- Break tests evaluate energy at every zero-time shared batch before evaluating spread.

Tests must use scale-aware absolute/relative tolerances tied to incoming speed and impulse magnitude. They must not compare only event strings, count moved balls, normalize output energy, or inspect source text.

## Risks and edge cases

- **Restitution-law ambiguity:** simultaneous Newton restitution is a model choice. This plan preserves the law already implemented by the Ideal matrix path and makes it common to all collision models. Any replacement requires an explicit API/model decision and separate evidence.
- **Redundant rack constraints:** dense frozen racks can produce singular contact matrices and non-unique contact impulses. Prefer a rank-aware feasible/minimum-norm contact solution and test physical velocity/status permutation invariance. Fail closed if residuals cannot establish a solution.
- **Iteration-order bias:** projected friction solvers can appear deterministic while retaining index-order error. Converge on residuals, canonicalize graph inputs, and require all permutation tests; a fixed sweep count is insufficient.
- **Normal/friction cross-coupling:** applying friction after normal impulses without revisiting normal constraints can reopen or reclose neighboring contacts. Use block coupling/reprojection and check final normal residuals.
- **Energy accounting:** friction transfers energy between translation and rotation. Tests must include both, with `I/m=2R^2/5`; translational energy alone is insufficient once friction/spin is active.
- **Vertical state:** the current reduced on-table delta omits vertical COM velocity. Material vertical contact slip must remain unsupported until the full-state contract in `plans/ball-collision-vertical-impulse.md` is available.
- **External table impulse:** an eligible/applied Kim object/table correction intentionally breaks ball-only momentum. Use the centralized decision and diagnostics from `plans/kim-correction-domain.md`; a skipped correction proceeds with a zero Kim term, while an applied correction needs validated external-impulse accounting or an unsupported result.
- **Zero-time progress:** an unsupported event must be terminal in both simulation loops. A successful event must change velocity or conclusively satisfy complementarity so it is not selected forever.
- **Numerical scale:** contact tolerances must scale with ball radius, incoming speed, and impulse magnitude while retaining a small absolute floor for rest states.
- **Performance:** build contact geometry/Jacobians once and avoid the current per-pass full-state clones and repeated binary collision prediction. Correctness and residual diagnostics take priority over micro-optimization.

## Dependencies and overlap

- **Depends on / coordinates with `plans/ball-collision-vertical-impulse.md`:** that plan owns correct full-state pair vertical impulses and event routing. This plan owns the coupled shared-contact graph. The shared solver must consume/produce its generalized vertical COM deltas; until then, material vertical-slip shared contacts are explicitly unsupported.
- **Depends on `plans/kim-correction-domain.md`:** that plan owns the single eligibility predicate and `Applied`/`SkippedObjectNotStationary`-style diagnostics for Kim's object/table correction. This plan consumes that decision per contact, never duplicates it, solves skipped contacts with a zero Kim term, and fails closed only when an applied external table correction is not represented by the shared solver.
- **Boundary with `plans/reject-overlapping-ball-states.md`:** that plan owns invalid initial `g<0` rejection and tiny roundoff recovery. This plan begins from admissible `g=0` touching/closing graphs and owns their velocity-level impulse resolution.
- **Boundary with `plans/curved-rolling-event-detection.md`:** that plan owns canonical trajectories and future root detection. This plan consumes the contact graph at the selected event time and does not duplicate CCD work.
- **Historical overlap:** `physics_audits/2026-04-24-event-geometry-multiball.md:78-95` requested explicit shared-contact handling, and `PHYSICS_AUDIT_2026-04-14.md:91-97` warned that dense multicontact remained approximate. The current issue is narrower and newly quantified: a real Ideal coupled path now exists, while only non-Ideal shared graphs still take an energy-creating fallback.

## Verification commands

Run focused tests first:

```sh
cargo test --test n_ball_advance symmetric
cargo test --test n_ball_advance shared
cargo test --test n_ball_pockets shared
cargo test --test n_ball_events shared
cargo test --test ball_collisions
cargo test --test break_shots
```

Then run the relevant aggregate suite:

```sh
cargo test --test n_ball_events --test n_ball_advance --test n_ball_pockets --test n_ball_simulation --test ball_collisions --test break_shots
```

Finally run the crate suite after focused physics and rack coverage is green:

```sh
cargo test
```

## Candidate commit message

```text
Couple non-ideal shared-contact impulses

Solve every shared graph with unilateral coupled normal impulses and bounded friction, expose per-contact diagnostics, and fail closed when a requested shared-contact model is unsupported or does not converge. Remove the summed standalone-pair fallback and lock the symmetric three-ball ideal limit, conservation, rack energy, and permutation invariants.
```
