# Restore Causal Jaw/Capture Event Ordering

Date: 2026-07-10

Severity: **High**  
Priority: **P1 — correctness before further pocket-response tuning**

## Problem statement

The pocket-aware scheduler can deliberately choose a jaw impact that occurs after a terminal pocket capture. `prefer_explicit_jaw_over_nearby_capture_ref` gives a same-ball, same-pocket jaw candidate priority whenever the jaw and capture times differ by at most `0.005 s`, regardless of which event is earlier. This is a macroscopic event-order reversal, not floating-point tie handling.

A continuous-event simulator must advance to the minimum supported future event time. A later solid contact cannot prevent a terminal event that has already occurred. If a capture boundary overlaps the jaw/facing geometry and predicts capture too early, the fix is to make the geometry and its event surfaces mutually consistent—not to advance through the earlier event and resolve a later one.

This plan removes the five-millisecond override, makes pocket capture a terminal crossing of the canonical pocket free-space geometry, and reserves deterministic source priority for candidates that are numerically simultaneous at the same geometric configuration.

## Current behavior and impact

### Observed implementation

- `src/lib.rs:1904-1943`, `prefer_explicit_jaw_over_nearby_capture_ref`, checks the absolute jaw/capture time difference and always prefers the jaw for the same ball and pocket.
- `src/lib.rs:1945-1959`, `earlier_n_ball_pocket_aware_event_candidate_ref`, invokes that override before comparing event times.
- `src/lib.rs:10717-10720` defines `NEARBY_JAW_CAPTURE_ORDER_TOLERANCE_SECONDS = 0.005` and describes the capture predictor as a coarse gate whose result may be superseded by a nearby jaw.
- `src/lib.rs:2109-2257`, `PocketAwareEventCache::next_event`, applies the comparator to ball-ball, airborne diagnostic, jaw, capture, rail, table-bounce, and motion-transition candidates. It separately tracks the raw earliest time, so internal state can acknowledge an earlier event while `best` names a later event.
- The ordinary near-equality branch at `src/lib.rs:1954-1958` uses `1e-12 s`, while shared ball-ball simultaneity uses `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS = 1e-12` at `src/lib.rs:9009`. The five-millisecond rule is therefore seven to ten orders of magnitude larger than numerical roundoff at ordinary shot horizons.
- The capture zero-set overlaps solid geometry. `pocket_mouth_plane_gap_raw` (`src/lib.rs:7159-7171`) becomes satisfied when the ball's leading surface reaches the mouth plane. `pocket_capture_gap_during_current_phase_raw` (`src/lib.rs:8288-8310`) combines the signed target interval and entry-angle acceptance with mouth- and back-plane gates; `compute_next_ball_pocket_capture_on_table` also uses a capture-circle root as an analytic search seed (`src/lib.rs:8537-8579`). Independently, `compute_next_ball_jaw_impact_on_table` (`src/lib.rs:8397-8472`) finds roots against jaw circles. A path can therefore enter the capture region and still have a later jaw root.
- `compute_next_ball_pocket_capture_on_table` (`src/lib.rs:8491-8647`) treats the first accepted entry into that overlapping region as terminal. `resolve_n_ball_system_event_with_physics_and_pockets_on_table` (`src/lib.rs:10835-10867`) either applies a jaw response or changes the state immediately to `NBallSystemState::Pocketed`.

### Impact

- A selected event need not be the chronological minimum of the pending candidates.
- At common shot speeds, the scheduler can advance a visibly macroscopic distance past an earlier terminal event—up to approximately one inch at `200 in/s`.
- Results depend on a modeling workaround rather than on the table's geometry. Changes to jaw radius, mouth width, facing shape, or target formulas can silently create or remove an event-order reversal.
- `advance_*`, `simulate_*`, DSL traces, playback markers, and `shot_probe` all inherit the wrong event and elapsed time.
- The current tests `advancing_a_near_jaw_side_pocket_entry_resolves_the_explicit_jaw` and `a_near_jaw_entry_can_late_drop_on_the_same_jaw_impact_step` (`tests/n_ball_pockets.rs:925-1000`) protect the workaround but do not establish that the jaw is physically first.

## Affected files, symbols, and callers

### Primary implementation

- `src/lib.rs:1739-1764` — `NBallPocketAwareSystemEventSource`, whose total source order is the deterministic tie-break order.
- `src/lib.rs:1766-1902` — `NBallPocketAwareSystemEventCandidateRef::{source,time_seconds,to_event}`.
- `src/lib.rs:1904-1959` — `prefer_explicit_jaw_over_nearby_capture_ref` and `earlier_n_ball_pocket_aware_event_candidate_ref`.
- `src/lib.rs:1961-2257` — `PocketAwareEventCache::{build,refresh_ball,next_event}` and all candidate reduction.
- `src/lib.rs:2360-2439` — `NBallSystemEvent` and its relative `time()` contract.
- `src/lib.rs:6006-6056` — resolved jaw geometry used by the current jaw roots.
- `src/lib.rs:6941-6957` — `pocket_target_bounds_in_inches`.
- `src/lib.rs:7149-7181` — pocket acceptance, mouth-plane, and back-plane gaps.
- `src/lib.rs:8288-8395` — capture gap, bracketing, and refinement.
- `src/lib.rs:8397-8647` — public jaw and capture predictors.
- `src/lib.rs:9009-9012` — existing numerical simultaneity and zero-time safeguards.
- `src/lib.rs:10717-10735` — obsolete five-millisecond constant and public next-event entry point.
- `src/lib.rs:10835-11222` — pocket event resolution, one-step advance wrappers, and until-rest/event-limit simulation loops.
- `src/lib.rs:13889-13942` — `PocketShapeSpec` and `PocketSpec`, consumed by the canonical geometry work in the sibling plans.

### Direct and transitive callers

- Public prediction: `compute_next_n_ball_system_event_with_rails_and_pockets_on_table`.
- Public one-step execution: `advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table`, the rail-profile/config wrappers, and `advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table` (`src/lib.rs:10888-10988`).
- Public simulation: all `simulate_n_ball_system_*_and_pockets_*` and `simulate_n_balls_*_and_pockets_*` variants (`src/lib.rs:10990-11222`).
- DSL: `Scenario::{simulate_shot_system_with_physics_on_table_until_rest, simulate_shot_system_with_rails_and_pockets_on_table_until_rest, simulate_shot_trace_with_physics_on_table_until_rest, simulate_shot_trace_with_physics_on_table_until_event_limit, simulate_shot_trace_with_rails_and_pockets_on_table_until_rest, simulate_shot_trace_with_preferred_physics_on_table_until_*}` (`src/dsl.rs:340-565`).
- DSL absolute event times: `scenario_event_log_from_simulation` (`src/dsl.rs:1392-1413`) accumulates the selected relative event durations.
- CLI: `src/bin/shot_probe.rs:312-318` calls the DSL pocket-aware trace path.
- Focused tests: `tests/n_ball_pockets.rs`, especially the pocket-capture ordering coverage at `:667-699`, jaw/capture cases at `:873-1069`, and pocket-aware scheduling tests at `:1274-1424`; DSL trace coverage belongs in `tests/dsl.rs` near its existing pocket event-log tests.

## Physics and source evidence

### Continuous-event causality

Michael Greenspan et al., *Toward a Competitive Pool-Playing Robot*, PDF page 5 (printed article page 50), section “Physics simulation,” states that the simulator operates in the continuous domain by “predicting the times of pending events” such as collisions and motion transitions, solves collision time from a continuous separation function, and requires no discrete time step. The extracted corpus text is at `whitepapers/toward_a_competitive_pool_playing_robot.pdf:174-221`; the indexed source is `agent_knowledge/whitepapers_index.jsonl:289`, and its generated formula-candidate entry is `agent_knowledge/whitepapers_formula_candidates.txt:1886-1889`.

For the finite set of valid pending events $E$ from the current state, the causal scheduler contract is

$$
t_* = \min_{e\in E} t_e, \qquad e_* \in \operatorname*{arg\,min}_{e\in E} t_e,
$$

with $t_e\ge 0$. Source priority is permitted only to choose among representations of the same numerical time; it cannot redefine $t_*$. For a terminal capture at $t_c$ and jaw contact at $t_j>t_c$, choosing the jaw advances the state by an unphysical extra interval

$$
\Delta t=t_j-t_c, \qquad
\Delta s=\int_{t_c}^{t_j}\!v(t)\,dt
\approx v(t_c)\Delta t-\tfrac12 a\Delta t^2.
$$

### Why geometry must replace the workaround

- TP B.15, *Pocket geometry calculations*, identifies distinct mouth, throat, cushion-nose, facing, and depth-edge lines in its diagram and derives their facing geometry. See `whitepapers/tp_b_15_pocket_geometry_calculations.pdf:12-42` and Equations 1–10 at `:42-134`; generated anchors are `agent_knowledge/whitepapers_formula_candidates.txt:3291-3311`. In particular,

  $$
  \beta=180^\circ-\alpha,\qquad \theta=\alpha-135^\circ,
  $$

  and Equation 10 derives facing angle from mouth width $m$, throat width $t$, and facing reference length $c$. These are separate physical surfaces, not an instruction to reorder overlapping abstract events.
- TP 3.7, *Effective target sizes for fast shots into a side pocket at different angles*, models a near-point path and repeated inside-wall contacts before the ball reaches the hole rim. See `whitepapers/tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf:12-40,42-116`; generated parameters and piecewise target anchors are `agent_knowledge/whitepapers_formula_candidates.txt:2071-2103`. It explicitly uses ball radius $R=1.125\,\mathrm{in}$, mouth width $p$, wall angle $\alpha$, shelf depth $b$, and a maximum fast-entry angle $\theta_{\max}=50.688^\circ$.

TP B.15 is the geometric source for physical mouth/facing surfaces. TP 3.7 is an analytic target/outcome model under stated equal-angle and three-wall-rattle assumptions, not empirical validation of a `5 ms` timing preference. Greenspan et al. supports the event-prediction method, not pocket calibration. The implementation must preserve those source boundaries.

## Numeric reproducer

Use the default `50 in × 100 in` table, the center-right side pocket, ball radius $R=1.125\,\mathrm{in}$, nominal mouth width $p=5.000\,\mathrm{in}$, current rounded-nose radius $r_j=0.125\,\mathrm{in}$, and the focused test motion profile with rolling deceleration $a=5\,\mathrm{in/s^2}$.

A straight rolling path at lateral offset `1.300 in` has center $y=51.300\,\mathrm{in}$ and speed $v_0=200\,\mathrm{in/s}$ toward `+x`. Use initial center

$$
(x_0,y_0)=(40.07984,51.300)\,\mathrm{in},
$$

velocity $(200,0)\,\mathrm{in/s}$, and rolling angular velocity $(0,200/R,0)\,\mathrm{rad/s}$.

Under the current capture surface, the leading ball surface reaches the mouth plane when the center reaches

$$
x_c=50-R=48.875\,\mathrm{in}.
$$

With $x(t)=x_0+v_0t-\tfrac12at^2$, this occurs at exactly `t_capture = 0.044000 s` for the chosen $x_0$.

The current upper jaw center is $(50,52.5)\,\mathrm{in}$. Its centerline separation from the path is $1.2\,\mathrm{in}$ and its inflated contact radius is $R+r_j=1.25\,\mathrm{in}$, so its near-side root is

$$
x_j=50-\sqrt{1.25^2-1.20^2}=49.650\,\mathrm{in}.
$$

Ignoring the negligible speed loss over this interval gives the explicit comparator case

$$
t_c=0.044000\,\mathrm{s},\qquad
t_j=0.047875\,\mathrm{s},\qquad
\Delta t=0.003875\,\mathrm{s}.
$$

The exact jaw time under $5\,\mathrm{in/s^2}$ deceleration is approximately `0.0478794555 s`; either difference is below `0.005 s`, so the current override selects the later jaw. Between the specified comparator times, the ball travels `0.7741099609 in` under that deceleration (approximately `0.775 in` at constant speed). At the full override width, a `200 in/s` ball travels

$$
200\,\mathrm{in/s}\times0.005\,\mathrm{s}=1.000\,\mathrm{in}
$$

(or `0.9999375 in` with `5 in/s²` deceleration over the interval). This is visibly macroscopic and cannot be classified as roundoff.

The `0.044000/0.047875 s` pair is a required synthetic comparator regression. The end-to-end geometry regression must use the physical state above after the rounded-jaw and rail/facing geometry plans land; it must not preserve the current false jaw merely to preserve those old timestamps.

## Root cause

1. **Overlapping event surfaces:** capture begins at a leading-edge mouth-plane crossing while jaw circles/facings remain executable farther into the same path. The capture predicate is an outcome/acceptance envelope, but the resolver treats entry into it as an immediate terminal state.
2. **Policy substituted for topology:** instead of deriving first contact or terminal drop from one pocket-local free-space/solid model, the cache independently schedules capture and jaw predictors and applies a same-pocket exception after prediction.
3. **A physical-time constant is mislabeled as ordering tolerance:** `0.005 s` scales into distance with shot speed and is unrelated to root-solver error, machine precision, or geometric residual.
4. **Current near-tie comparison is not a complete tie contract:** it uses a literal `1e-12` pairwise test rather than one canonical tolerance anchored to the raw minimum. Pairwise “within epsilon” reduction can be traversal-order dependent for chains of candidates unless the raw minimum is found first.

## Target design and invariants

### Canonical nonoverlapping pocket event geometry

Consume the sibling-owned `ResolvedTableBoundaryGeometry { rail_faces, pocket_boundaries: [ResolvedPocketBoundaryGeometry; 6] }`; do not add a second jaw/facing/capture resolver. Each pocket boundary's single `capture_boundary` is the terminal capture segment described here:

- An inward unit axis $\mathbf n$ and tangent $\mathbf u$ derived from the resolved pocket boundary.
- `ResolvedPocketBoundaryGeometry::{mouth_tips,nose_arcs,facings,inner_walls,capture_boundary}` plus the sibling-owned `ResolvedPocketJawGeometry::{Point { mouth_tip }, RoundedArc { mouth_tip, center, nose_radius, rail_boundary_normal, facing_boundary_normal, sweep }}` in raw inches/unit vectors. Collision queries derive $S\oplus B_R$ from these canonical uninflated solids at query time; no radius-inflated geometry is persisted. Rounded-jaw collision expands only the bounded exposed arc, and mouth/capture construction uses the physical `mouth_tip`, not the curvature center.
- A terminal capture segment at the shelf/drop boundary, with inward coordinate

  $$
  q_{\mathrm{capture}}=q_{\mathrm{mouth}}+d,
  $$

  where $q=\mathbf n\cdot\mathbf x$ and $d$ is the pocket's configured shelf/depth measurement converted to inches.
- Lateral segment limits formed from ball-center free space after a ball-radius Minkowski offset of the canonical solid facings/walls. The signed TP target interval remains a separate outcome gate evaluated at the physical mouth crossing, where TP 3.5–3.8 define effective target size; do not incorrectly transplant that mouth interval to the deeper terminal plane.

For each solid surface $S_k$, define the ball-center gap

$$
g_k(\mathbf x)=\operatorname{dist}(\mathbf x,S_k)-R.
$$

The interior of the terminal capture segment must satisfy $g_k>\epsilon_x$ for every solid. It may meet a solid only at a shared endpoint/seam within geometric root tolerance. Therefore a path either encounters a solid root before reaching the terminal boundary, crosses the terminal boundary through free space, or reaches the shared seam as a genuine numerical tie. There is no interval over which an accepted terminal capture and a distinct later jaw/facing are both valid alternatives.

Change the capture predictor accordingly:

- Replace the current mouth-leading-edge terminal condition, back-plane slab gate, and capture-circle search seed with inward crossing of `ResolvedPocketBoundaryGeometry::capture_boundary`.
- Evaluate the signed TP target bounds at the trajectory's physical mouth crossing using the angle and speed there; evaluate the radius-offset free-space interval at the terminal crossing. Both conditions must hold for a terminal candidate.
- Preserve the existing within-motion-phase root/bracketing machinery; root enumeration improvements for curved paths are separate work.
- Keep `PredictedBallPocketCapture::state_at_capture` at the actual terminal boundary.
- Treat a ball already beyond the terminal boundary as zero-time capture only when it lies inside the free interval and is already irreversibly in the pocket region; do not capture arbitrary behind-plane states.

### Numerical tie semantics

Define one canonical, scale-aware time epsilon around the raw minimum:

$$
\epsilon_t(t_*)=\max\!\left(10^{-12}\,\mathrm{s},
64\,\epsilon_{\mathrm{mach}}\max(1\,\mathrm{s},|t_*|)\right).
$$

Selection is two-stage:

1. Visit all valid candidates and compute the raw finite nonnegative minimum $t_*$ using ordinary/total floating-point ordering.
2. Visit candidates again, retain only $|t_e-t_*|\le\epsilon_t(t_*)$, and choose the smallest existing `NBallPocketAwareSystemEventSource` as the deterministic representative.

This raw-minimum anchoring makes selection permutation-invariant and guarantees that source priority can move the reported time by at most numerical epsilon. The existing source order puts `BallJawImpact` before `BallPocketCapture`, so a true shared-boundary jaw/capture tie deterministically chooses solid contact. A jaw even slightly outside the numerical band loses to an earlier capture. Shared ball-ball contact batching continues to use its explicit contact-graph logic.

Required scheduler invariants are:

- `selected.time <= raw_minimum + epsilon_t(raw_minimum)`.
- No pending candidate has `time < selected.time - epsilon_t(raw_minimum)`.
- Reordering candidate visitation cannot change the selected source or time.
- Every reported step duration is finite and nonnegative.
- Cumulative event time $T_i=\sum_{k=0}^{i}t_k$ is nondecreasing; `simulation.elapsed` equals the final cumulative time.
- Once a ball receives a terminal `BallPocketCapture`, no later event names that ball.

## Explicit non-goals

- Do not retain a smaller jaw/capture-specific grace period, speed-dependent distance allowance, or compatibility flag.
- Do not special-case the `1.300 in` fixture in either predictor.
- Do not redesign rail or rounded-jaw placement in this plan; those are owned by `plans/pocket-rail-cutouts.md` and `plans/pocket-rounded-jaw-mouth-width.md`.
- Do not add a parallel pocket geometry type beside the canonical resolved geometry from those plans.
- Do not recalibrate `PocketSpec.depth`, TP 3.5–3.8 target curves, speed interpolation, restitution, or friction. This plan uses the configured depth as the existing shelf/drop measurement and fixes event topology/order.
- Do not redesign the post-impact `should_capture_after_jaw_impact` rattle/late-drop heuristic. It may run only after a jaw that was itself the causal earliest event; richer multi-wall dynamics remain separate work.
- Do not change ball-ball shared-contact resolution, airborne pocket geometry, or general curved-root completeness.
- Do not change public event variants or expose numerical tolerance as a user tuning parameter.

## API and data-model cutover

### Public API

Keep these contracts source-compatible:

- `PredictedBallJawImpact`, `PredictedBallPocketCapture`, and `NBallSystemEvent` variants remain unchanged.
- `NBallSystemEvent::time()` remains the relative duration from the current simulation state.
- Existing compute/advance/simulate and DSL methods keep their signatures.
- `NBallSystemSimulation.events` remains a sequence of relative events, and `ScenarioShotTrace.event_log` remains cumulative absolute time.

The behavioral cutover is clean: all callers immediately receive causal minimum-time selection. There is no legacy mode, alias, deprecation path, or shim.

### Internal model and migration

- Use the single canonical `ResolvedPocketBoundaryGeometry::capture_boundary` as the terminal segment at the configured shelf/drop depth. Keep the TP effective-target interval attached to the physical mouth-crossing predicate rather than embedding it in the deeper terminal segment; do not create another capture surface.
- Make `compute_next_ball_jaw_impact_on_table` and `compute_next_ball_pocket_capture_on_table` consume the same `ResolvedTableBoundaryGeometry`. Rounded-jaw queries must honor `RoundedArc::sweep` and its rail/facing boundary normals, and all ball-radius expansion is query-time. Public wrappers may resolve from `TableSpec`, while `PocketAwareEventCache` should reuse the resolved per-table geometry rather than constructing a competing representation.
- Remove `NEARBY_JAW_CAPTURE_ORDER_TOLERANCE_SECONDS`, its stale explanatory comment, and `prefer_explicit_jaw_over_nearby_capture_ref` completely.
- Replace `earlier_n_ball_pocket_aware_event_candidate_ref`'s pairwise policy with raw-minimum-anchored selection. Add a no-allocation `PocketAwareEventCache` candidate visitor and run it twice rather than allocating a `Vec` on every scheduling step.
- Reuse `SIMULTANEOUS_EVENT_TOLERANCE_SECONDS` as the absolute floor of the canonical time epsilon; remove the comparator's duplicate literal `1e-12`.
- Preserve `NBallPocketAwareSystemEventSource` ordering as the documented deterministic representative order. Document explicitly that source order applies only inside the numerical tie band.
- Rework the two old near-jaw tests at `tests/n_ball_pockets.rs:925-1000`: one fixture must geometrically hit the corrected jaw/facing before any terminal boundary; the clean `1.300 in` path must capture without a false jaw. Do not merely flip assertions on the same invalid geometry.

## Phased implementation plan

### Phase 1 — Lock down causal selection before changing the comparator

1. Add private unit fixtures in `src/lib.rs` that construct jaw and capture candidates with a shared state, ball index, and pocket.
2. Add a failing regression with capture `0.044000 s` and jaw `0.047875 s`; require `BallPocketCapture` and exact selected time `0.044000 s`.
3. Add boundary tests at one raw minimum $t_*$:
   - jaw and capture exactly equal: solid contact wins deterministically;
   - jaw at $t_*+0.5\epsilon_t$: solid contact may represent the numerical tie;
   - jaw at $t_*+2\epsilon_t$: the earlier capture wins;
   - an earlier jaw similarly beats a later capture.
4. Feed the same candidate set in multiple visitation orders, including a three-time chain spanning more than one epsilon, and require identical output anchored to the raw minimum.
5. Add an invariant/property-style table of candidate times and sources asserting the selected event is never more than $\epsilon_t$ after the raw minimum. Keep it deterministic; no random generator is needed.

### Phase 2 — Land and validate canonical nonoverlapping pocket geometry

1. Land the physical-mouth-width correction from `plans/pocket-rounded-jaw-mouth-width.md` and the finite rail-cutout/facing model from `plans/pocket-rail-cutouts.md` first.
2. Before changing capture code, add geometry regressions for the resolved center-right pocket and its center-left mirror:
   - the `1.300 in` straight path lies in ball-center free space and does not intersect a corrected jaw arc;
   - a path aimed into the exposed rounded arc has a solid root before the terminal capture segment;
   - a clean path crosses the terminal segment without any earlier solid root;
   - the only jaw/capture coincidence is the shared seam within geometric tolerance.
3. Populate and validate the canonical `ResolvedPocketBoundaryGeometry::capture_boundary` at the terminal capture coordinate/free interval derived from the mouth plane, configured depth, and uninflated facing/wall solids. Derive $S\oplus B_R$ only when querying for the current ball radius.
4. Replace the mouth-leading-edge/back-plane terminal gap and radial search seed in `pocket_capture_gap_during_current_phase_raw`/`compute_next_ball_pocket_capture_on_table` with the canonical `capture_boundary` crossing. Validate the TP target at the corresponding physical mouth crossing and the free interval at the terminal crossing. Delete capture-only geometry terms that no longer define a physical boundary rather than leaving both models active.
5. Keep the public predictor wrappers and migrate `PocketAwareEventCache::refresh_ball` to the canonical geometry path.

### Phase 3 — Remove the override and implement true tie selection

1. Add the scale-aware epsilon helper and document its units and role.
2. Add a no-allocation candidate visitor over every candidate category in `PocketAwareEventCache`.
3. Refactor `next_event` into raw-minimum and tie-representative passes. Use the same raw minimum for the existing shared ball-ball batching decision.
4. Delete `prefer_explicit_jaw_over_nearby_capture_ref`, `NEARBY_JAW_CAPTURE_ORDER_TOLERANCE_SECONDS`, and the stale “coarse gate” ordering rationale.
5. Retain source order only for the numerical tie set. Assert/debug-assert that accepted candidate times are finite and nonnegative at the selection boundary.
6. Run the Phase 1 tests before migrating integration expectations; the exact `0.044000/0.047875 s` case must now pass.

### Phase 4 — Migrate callers and prove end-to-end monotonicity

1. Replace the workaround-protecting tests at `tests/n_ball_pockets.rs:925-1000` with geometry-grounded clean-capture and true-jaw fixtures.
2. Add the full `200 in/s`, `(40.07984, 51.300) in` reproducer through `compute_next_n_ball_system_event_with_rails_and_pockets_on_table` and one-step advance. Under corrected geometry it must never report a later event while an earlier terminal event remains pending; it must either select the actual earliest solid root or the canonical terminal capture, with no five-millisecond exception.
3. Add a multi-step `simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit` test that accumulates every `event.time()`. Assert finite/nonnegative steps, nondecreasing cumulative times, equality with `simulation.elapsed`, and no event involving a ball after its capture.
4. Add a `tests/dsl.rs` trace regression for a pocket-aware scenario. Assert `ScenarioShotTrace.event_log` times are nondecreasing, each absolute timestamp equals the cumulative physics event durations, and no jaw/rail/motion event for the captured ball appears after its `BallPocketCapture` entry.
5. Confirm existing same-time ball-ball batching, zero-time jaw contact, centered clean pocket entry, shallow jaw rejection, and irrelevant-pocket parity tests remain unchanged in observable behavior.

### Phase 5 — Source comments and cleanup after behavior passes

1. Update `compute_next_ball_pocket_capture_on_table` rustdoc (`src/lib.rs:8474-8490`) to describe the canonical terminal capture boundary rather than a coarse first-pass region.
2. Update scheduler/event rustdoc to state raw-minimum selection and numerical-only deterministic tie order.
3. Remove comments and test names that describe a `5 ms` “nearby jaw” preference as valid behavior.
4. Do not rewrite the dated `physics_audits/2026-04-24-rail-pocket.md`; it is historical context and its earlier “safety valve” judgment is superseded by this implementation and regression evidence.
5. Do not edit `agent_knowledge/*`; those are generated corpus artifacts and the underlying whitepapers do not change. No generated artifact regeneration is required.
6. No README or DSL syntax change is required because public signatures and syntax remain stable.

## Regression and acceptance tests

The implementation is accepted only when all of the following observable contracts hold:

1. **Required comparator case:** capture at `0.044000 s` and same-ball/same-pocket jaw at `0.047875 s` selects capture at `0.044000 s`.
2. **Macroscopic-gap rejection:** no candidate `3.875 ms` or `5 ms` after the raw minimum can win through source priority, including at `200 in/s` where the old policy displaced the ball by approximately `0.775 in` or up to `1 in`.
3. **True tie:** equal or roundoff-scale (`≈1e-12 s`) shared-boundary jaw/capture roots deterministically select the solid contact; a difference outside the computed epsilon selects chronological minimum.
4. **Permutation invariance:** candidate visitation order does not change the selected source/time.
5. **Nonoverlapping geometry:** clean free-space entries have no earlier jaw/facing root; actual jaw/facing paths hit a solid before the terminal capture segment; seam contact is the only tolerated coincidence.
6. **Mirroring:** center-left and center-right fixtures produce mirrored event classes and equal times within numerical tolerance.
7. **One-step causality:** `advance.elapsed == event.time()` and no independently pending candidate is earlier than the reported event by more than numerical epsilon.
8. **Simulation monotonicity:** cumulative event times never decrease, may remain equal only for legitimate zero-time cascades, and end at `simulation.elapsed`.
9. **Terminality:** after `BallPocketCapture`, no later event or DSL log entry involves that ball.
10. **No regressions outside overlap:** shared ball-ball contacts, unrelated motion transitions, ordinary rail hits outside pocket mouths, immediate closing jaw contacts, and clean centered captures retain their existing contracts.

## Risks and edge cases

- **Root residual versus time tolerance:** two roots for the same seam can differ by more than `1e-12 s` if a predictor is inaccurate. Do not enlarge scheduler epsilon to hide this; improve the root or shared geometry so both predicates converge on the same configuration.
- **Nontransitive pairwise epsilon:** avoid folding with pairwise “near” comparisons. Always anchor the tie set to a separately computed raw minimum.
- **Long horizons:** the relative ULP term prevents a fixed absolute threshold from becoming smaller than floating precision without creating a millisecond-scale physical allowance.
- **Zero-time initial states:** a ball touching a closing jaw remains a valid zero-time solid event. A ball already beyond the capture plane is captured at zero only if it satisfies the canonical free-space/terminal predicate. Existing no-progress safeguards remain active.
- **Changed trace timestamps:** moving terminal capture from leading-edge mouth entry to the physical shelf/drop boundary will change capture states, elapsed times, path vertices, and DSL playback markers. Update expected values only when they are derived from the new boundary.
- **Post-jaw capture:** `should_capture_after_jaw_impact` can still produce a terminal state on the selected jaw step. Ensure no test confuses that resolution rule with permission to choose a later jaw.
- **Corner sign conventions and mirrors:** derive axes/tangents from canonical pocket geometry; do not hand-code per-pocket comparator exceptions.
- **Performance:** a two-pass cache traversal is acceptable, but it must not allocate candidate vectors in the scheduling hot path. Geometry should be resolved/reused rather than repeatedly converted from exact decimal units.
- **Dependency drift:** if either sibling plan chooses different internal names, reuse its canonical representation and preserve the invariants here; do not introduce duplicate geometry solely to match this plan's illustrative terminology.

## Dependencies and overlap

- **Depends on `plans/pocket-rounded-jaw-mouth-width.md` — “Preserve Physical Pocket Mouth Width with Rounded Jaw Arcs.”** That plan owns interpreting configured mouth width as physical tip-to-tip width and introduces the canonical `ResolvedPocketJawGeometry::{Point, RoundedArc}` representation with physical `mouth_tip`, arc center/radius, boundary normals, and oriented exposed sweep. Its correction removes the false jaw collision in the `1.300 in` offset reproducer. This plan consumes that representation and owns scheduler chronology, terminal capture topology, and numerical ties.
- **Depends on `plans/pocket-rail-cutouts.md` — “Clip Rail Solids at Pocket Mouths and Add Explicit Facings.”** That plan owns `ResolvedTableBoundaryGeometry` and `ResolvedPocketBoundaryGeometry` with finite `rail_faces`, `mouth_tips`, shared `nose_arcs`, `facings`, `inner_walls`, and one `capture_boundary`. This plan consumes that uninflated canonical model, gives the capture boundary terminal event semantics, derives $S\oplus B_R$ only at query time, and selects all resulting roots causally; it does not duplicate rail/facing construction.
- The plans should land in the order: rounded-jaw mouth invariant and rail/facing geometry, then terminal capture integration and comparator removal. The failing synthetic comparator tests may be added before either dependency because they do not require geometry.
- `PHYSICS_ENGINE_PLAN.md:779-811` broadly names made/jawed/rejected/crossed-face outcomes but does not specify event-time causality or replace this plan.
- `physics_audits/2026-04-24-rail-pocket.md:97-101` records the older heuristic model and historical safety-valve judgment. It overlaps context only and is not an implementation dependency.

## Verification commands

Run focused regressions first, then the relevant aggregate suites:

```bash
nix develop -c cargo test --lib pocket_aware_ordering_keeps_earlier_capture_over_jaw_3_875_ms_later
nix develop -c cargo test --lib pocket_aware_ordering
nix develop -c cargo test --test n_ball_pockets clean_offset_side_entry_uses_causal_pocket_geometry
nix develop -c cargo test --test n_ball_pockets pocket_aware_simulation_reports_monotonic_causal_event_times
nix develop -c cargo test --test dsl pocket_trace_event_times_are_monotonic_and_terminal
nix develop -c cargo test --test n_ball_pockets
nix develop -c cargo test --test dsl
nix develop -c cargo test --lib
nix develop -c cargo test
```

The first exact comparator regression and the geometry/end-to-end tests must fail against the old five-millisecond implementation and pass only after the override and overlap are removed.

## Candidate commit message

```text
Restore causal jaw and pocket event ordering

Remove the 5 ms later-jaw override, derive capture from canonical
nonoverlapping pocket geometry, and reserve source priority for true
numerical ties.
```
