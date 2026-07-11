# Correct SpinAware rail energetic restitution integration

- **Date:** 2026-07-10
- **Severity:** Medium
- **Priority:** P1 — correct before further rail-response calibration or bank-path tuning
- **Scope owner:** on-table `RailModel::SpinAware` impact integration and its restitution invariants

## Problem statement

**Observed:** The Mathavan-style solver does not integrate cushion-normal work as specified by Mathavan, Jackson, and Parkin. It uses endpoint rectangles instead of equation (16a)'s trapezoid, derives the restitution impulse increment from work even though the two quantities have different dimensions, stops compression short of the zero-normal-speed root, and computes the final restitution increment by assuming constant speed over that increment.

As a result, `RailModel::SpinAware` does not honor its configured energetic coefficient of restitution even in the frictionless limit. A nominally perfectly elastic, frictionless 90 in/s rail hit returns about 88.23 in/s instead of 90 in/s.

**Proposed:** Keep impulse as the independent variable, but use one dimensionally correct mass-normalized impulse increment for both phases, integrate signed work with pre/post contact-normal speeds, solve the compression endpoint at zero relative normal speed, and solve the last restitution increment against the trapezoidal work equation. Retain bounded convergence diagnostics so the energetic contract is directly testable.

## Current behavior and impact

### Observed implementation behavior

The private solver represents impulse divided by ball mass, so `delta_p` has velocity units (`in/s`), while accumulated mass-specific work has velocity-squared units (`(in/s)^2`). The current paths are:

1. `rail_impact_work_increment` computes

   \[
   \Delta w_{\text{code}}=\Delta p\,|q|\cos\theta,
   \]

   using one endpoint, where `q` is `normal_speed_toward_cushion`.
2. Compression advances the state first and accumulates a right-endpoint rectangle. When a full step crosses zero, eight dyadic refinements return the last still-positive `q`; the actual compression root and the work through that root are omitted.
3. Restitution sets

   \[
   \Delta p_{\text{code}}=\frac{w_{\text{target}}}{N},
   \]

   assigning a quantity with units `(in/s)^2` to a mass-normalized impulse step that must have units `in/s`.
4. Restitution checks overshoot with a pre-step endpoint work estimate but accumulates accepted work with a post-step endpoint estimate.
5. The final partial increment is computed as

   \[
   \Delta p_{\text{partial}}=\frac{w_{\text{remaining}}}{|q_n|\cos\theta},
   \]

   even though applying that increment changes `q` and therefore changes the work integrand over the same interval.
6. At `e = 0`, restitution exits immediately from the last still-positive compression state. That residual cushion-directed speed can make the ball remain an incoming zero-time rail candidate.

### User-visible and simulation impact

- `RailCollisionConfig::normal_restitution` is not the energetic restitution actually delivered by `RailModel::SpinAware`.
- The error varies with impact speed because restitution resolution is selected from a work scale proportional to speed squared. Calibration at one speed can therefore hide a solver error that reappears at another speed.
- Every SpinAware rail and pocket-jaw response inherits the error because both use `spin_aware_ball_cushion_collision_on_table_from_basis`.
- Rail-aware event execution inherits the incorrect rebound. At the `e = 0` boundary, residual incoming motion can also prevent clean event progress.
- Multiple-bank traces apply the bias repeatedly, so a per-impact normal-speed deficit compounds along a bank path.
- `Mirror` and `RestitutionOnly` are not affected.

## Exact affected files, symbols, and callers

Line ranges are the current 2026-07-10 ranges and must be re-located by symbol if surrounding edits move them.

### Core source: `src/lib.rs`

- `RAIL_IMPACT_SOLVE_MAX_STEPS`, `RAIL_IMPACT_MIN_IMPULSE_STEP`, and `RAIL_IMPACT_REFINEMENT_STEPS` (`12514-12516`): currently mix nominal resolution, an unnamed native-unit floor, and a fixed refinement count.
- `RailImpactFrameState` (`12529-12535`): internal state whose `normal_speed_toward_cushion` supplies the contact-normal work integrand after multiplication by `cos_theta`.
- `advance_rail_impact_frame_by_impulse_step` (`12599-12633`): advances translational and angular state using mass-normalized normal impulse.
- `rail_impact_work_increment` (`12635-12641`): endpoint-rectangle work calculation; change its contract to accept both pre- and post-step normal speeds.
- `solve_rail_impact_compression_phase` (`12643-12707`): right-endpoint accumulation and incomplete compression root.
- `solve_rail_impact_restitution_phase` (`12709-12765`): work-derived impulse step, inconsistent endpoint accumulation, and constant-speed final partial step.
- `solve_spin_aware_rail_impact_in_frame` (`12767-12799`): phase orchestration and the appropriate boundary for returning solve diagnostics.
- `spin_aware_ball_cushion_collision_on_table_from_basis` (`13006-13133`): sole internal consumer of the frame solve; migrate it to consume the final state from the new solve result.
- `spin_aware_ball_rail_collision_on_table` (`13135-13157`) and `collide_ball_jaw_on_table_with_radius_and_profile` (`13159-13209`): straight-rail and pocket-jaw dispatch to the shared basis solve.
- Public rail entry points `collide_ball_rail_on_table_with_radius_and_profile`, `collide_ball_rail_on_table_with_radius_and_config`, `collide_ball_rail_on_table_with_radius`, and `collide_ball_rail_on_table` (`13237-13321`): signatures remain stable, but SpinAware outputs change to the corrected energetic result.
- On-table event resolution (`10029-10039`), pocket-aware jaw and rail resolution (`10835-10857`, `10874-10883`), and bank-path rail resolution (`11259-11385`): indirect callers that inherit the corrected response. No signature change is required at these sites.

### Tests

- `tests/rail_collisions.rs`: reuse `assert_close_with_tolerance`, `rail_state_from_local_frame`, and `rail_local_frame_components` (`19-105`); add the frictionless speed/restitution sweep alongside the existing SpinAware response tests (`220-1035`). Existing Mathavan calibration and local-frame invariance tests at `791-824` and `989-1035` remain regression coverage.
- `tests/rail_event_execution.rs`: reuse the configured SpinAware event path demonstrated by `advancing_to_a_spin_aware_rail_impact_uses_the_configured_restitution_and_spin_response` (`176-231`) to cover `e = 0` event progress.
- `src/lib.rs`: add a narrowly scoped `#[cfg(test)]` module adjacent to the private rail-impact solver so internal work, impulse, root, step-count, and refinement diagnostics can be asserted without publishing a diagnostic API.

### Source-facing documentation

- `whitepapers/rail_rebound.md:145-157`: replace the open question about compression/restitution work tracing with the implemented internal diagnostic contract and document equation (16a)'s pre/post trapezoid.
- The primary PDF and generated corpus are evidence, not edit targets:
  - `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf`
  - `agent_knowledge/whitepapers_corpus.txt`

## Whitepaper evidence and governing equations

Primary source: S. Mathavan, M. R. Jackson, and R. M. Parkin, *A theoretical analysis of billiard ball dynamics under cushion impacts*, §3.3-3.4, journal pages 1868-1869.

- `agent_knowledge/whitepapers_corpus.txt:2501-2506`: the energetic coefficient `e_e` is independent of friction/slip, and `e_e^2` is the negative ratio of restitution work to compression work.
- `agent_knowledge/whitepapers_corpus.txt:2511-2529`, equation (16a), gives the discrete work increment explicitly as

  \[
  \Delta W_{ZI}=\Delta P_I\frac{(\dot z_I)_{n+1}+(\dot z_I)_n}{2}.
  \]

  This is trapezoidal quadrature over impulse, not an endpoint rectangle.
- `agent_knowledge/whitepapers_corpus.txt:2533-2558`, equations (16b)-(16c), requires

  \[
  W_{ZI}(P_{If})=(1-e_e^2)W_{ZI}(P_{Ic}),
  \qquad
  \dot z_I(P_{Ic})=0.
  \]

  Equivalently, when positive magnitudes are stored per phase,

  \[
  w_r=e_e^2 w_c.
  \]
- `agent_knowledge/whitepapers_corpus.txt:2546-2578`: the numerical algorithm stops at zero normal relative speed, records compression work, resumes impulse integration, and terminates when the equation (16b) work target is reached.
- `agent_knowledge/whitepapers_corpus.txt:2585-2595`: for approximately `N` iterations, the paper estimates

  \[
  \Delta P_I\approx\frac{(1+e_e)M V_n}{N}.
  \]

  Dividing by mass gives a step `delta_p = Delta P_I / M` with velocity units. The paper reports `N = 5000` as satisfactory for its MATLAB calculation; this repository may retain its production resolution of 1000 only if the convergence contract below passes.
- `agent_knowledge/whitepapers_corpus.txt:2268-2277`: in the frictionless two-dimensional limit, normal rebound speed is `e_e` times incident normal speed and tangential speed is unchanged.
- `agent_knowledge/whitepapers_corpus.txt:2602-2615`: the rigid-cushion analysis is intended for oblique impacts whose normal component is below `2.5 m/s`; the fitted values reported there are `e_e = 0.98` and `mu_w = 0.14`.

For the current mass-normalized implementation, define

- `p = P_I/M`, in `in/s`;
- `q = normal_speed_toward_cushion`, in `in/s`;
- `u_I = q cos(theta)`, the signed cushion-contact normal relative speed, in `in/s`;
- `w = W/M`, in `(in/s)^2`.

The source equation becomes

\[
\Delta w=\Delta p\frac{u_{I,n}+u_{I,n+1}}{2}.
\]

Compression increments are positive. Restitution increments are negative; diagnostics should expose the positive released-work magnitude `w_r = -sum(Delta w_restitution)` and compare it directly with `e_e^2 w_c`.

## Numeric reproducer with units

**Observed audit reproduction:** Use a direct top-rail impact with:

- local tangential speed `v_t = 0 in/s`;
- local cushion-directed normal speed `q_in = 90 in/s = 2.286 m/s`;
- angular velocity `(omega_t, omega_n, omega_z) = (0, 0, 0) rad/s`;
- radius `R = 1.125 in`;
- `normal_restitution = e_e = 1`;
- cushion friction `mu_w = 0`;
- impact cloth friction `mu_s = 0`;
- effective contact-height add-on `a/R = 0`;
- `RailModel::SpinAware` through `collide_ball_rail_on_table_with_radius_and_config`.

The 90 in/s normal component is below the paper's 2.5 m/s (`98.4252 in/s`) rigid-cushion limit. With no friction or spin coupling,

\[
q_{out}=-e_e q_{in}=-90\ \text{in/s},
\]

while tangential speed and all spin components remain zero. With `sin(theta)=2/5` and `cos(theta)=sqrt(21)/5`, the analytic mass-specific compression work is

\[
w_c=\int_0^{p_c}q(p)\cos\theta\,dp
=\tfrac12 q_{in}^2
=4050\ (\text{in/s})^2.
\]

A deterministic evaluation of the current loops gives:

- current compression work: `4046.2884036 (in/s)^2`;
- current restitution step: `4.0462884 in/s`;
- normal-speed change per full restitution step: about `3.70848 in/s`;
- 23 full restitution steps followed by a constant-speed partial step of about `3.20470 in/s`;
- resulting `q_out = -88.2320121 in/s`, conventionally reported as **88.23 in/s rebound speed**.

The output is short by about `1.7680 in/s`, a `1.97%` loss, despite `e_e = 1`. This number was derived from the displayed implementation during the audit; no current repository regression test exercises it.

## Root cause

1. **Wrong quadrature:** Work depends on contact-normal speed throughout an impulse interval. One endpoint does not implement equation (16a), and the compression/restitution paths do not even use the same endpoint convention.
2. **Dimensional mismatch:** `target_work / N` has units of `(in/s)^2`, not the `in/s` required by the mass-normalized impulse integrator. It also makes step size scale quadratically with impact speed.
3. **Compression endpoint is not the source-defined root:** Fixed-count halving retains a small positive incoming speed instead of applying the partial impulse that reaches `u_I = 0`.
4. **Final restitution work assumes a frozen integrand:** Dividing remaining work by current speed applies a rectangle precisely where the algorithm must close the trapezoidal energetic target.
5. **Insufficient observability:** Phase work, accumulated impulse, final residuals, and step counts are local scalars and discarded, so tests cannot distinguish work closure from a coincidentally plausible rebound.

## Explicit non-goals

- Do not change airborne rail eligibility, three-dimensional rail-envelope geometry, or airborne state preservation. Those belong to `plans/airborne-table-boundary-geometry.md`.
- Do not add cushion compliance/deformation or extend Mathavan's rigid-cushion model above the stated 2.5 m/s normal-speed range.
- Do not recalibrate human-tuned restitution, cushion friction, impact-cloth friction, or effective contact-height defaults.
- Do not change `RailModel::Mirror` or `RailModel::RestitutionOnly`.
- Do not redesign the existing cushion/table slip equations, adherence band, TP 7.3 geometric term, or rolling-entry spin guardrails. Only feed those paths the state produced by the corrected energetic integration.
- Do not change pocket-jaw geometry or event geometry. Jaws receive the correction only because they already share the basis solver.
- Do not publish a second rail-collision API solely for diagnostics. Keep diagnostics private and stack-valued; public collision signatures continue returning `OnTableBallState`.
- Do not directly edit `agent_knowledge/whitepapers_corpus.txt` or other generated agent-knowledge artifacts.

## Proposed solver and data-model design

### 1. Make native units explicit

- Rename the production resolution constant so it clearly means nominal impulse intervals rather than a convergence cap, for example `RAIL_IMPACT_NOMINAL_IMPULSE_STEPS`.
- Rename the native-unit floor to encode mass-normalized impulse and units, for example `RAIL_IMPACT_MIN_MASS_NORMALIZED_IMPULSE_STEP_INCHES_PER_SECOND`.
- Compute one base step from the incident normal speed:

  \[
  \Delta p_{nominal}=\max\left(\frac{(1+e_e)q_{in}}{N},\Delta p_{min}\right).
  \]

  Pass the same `delta_p_nominal` into compression and restitution. Never derive an impulse interval from work.
- Separate `N = 1000` from a hard phase cap of `10N`, and separate both from bounded scalar-root iterations. The production setting is accepted only with the convergence tests below.

### 2. Replace endpoint work with a signed trapezoid

Change `rail_impact_work_increment` to accept pre- and post-step normal speeds and compute

\[
\Delta w=\Delta p\,\cos\theta\,\frac{q_n+q_{n+1}}{2}.
\]

Do not use `abs` inside the source-equation helper. Maintain the phase sign explicitly:

- compression requires `q_n >= 0`, accumulates `Delta w >= 0`, and reports `w_c`;
- restitution requires `q_n <= 0`, accumulates `-Delta w >= 0`, and reports released work `w_r`.

This prevents a step from silently crossing the compression/restitution boundary while hiding the sign change inside `abs`.

### 3. Solve the compression root and include its work

For every compression increment:

1. Save `before`.
2. Evaluate a full `delta_p_nominal` candidate.
3. If the candidate remains cushion-directed, accept it and add the pre/post trapezoid.
4. If it crosses zero, solve `candidate(delta_p).normal_speed_toward_cushion = 0` for `delta_p` in `[0, delta_p_nominal]` with bounded bisection (or an equivalently bounded monotone scalar solve).
5. Advance once with the solved partial impulse, include that partial step's trapezoidal work, record its impulse and step, normalize a residual within the solver tolerance to exact zero, and terminate compression.

The scalar solve must evaluate candidates from the same pre-step state; it must not repeatedly mutate the state while refining. This preserves the existing explicit impulse update's slip direction over one interval and avoids allocation.

### 4. Close restitution with a trapezoidal partial step

Set `target_restitution_work = e_e^2 * compression_work`.

- For `e_e = 0`, return the zero-normal-speed compression-root state with zero restitution work and zero restitution impulse.
- Otherwise, evaluate each full candidate from the current restitution state and compute its positive released-work magnitude from the signed pre/post trapezoid.
- Accept full steps while they do not exceed the remaining target.
- When a full step would overshoot, solve in `[0, delta_p_nominal]` for the partial impulse whose trapezoidal released work equals the exact remaining work. Use the same bounded scalar-root machinery rather than `remaining_work/current_speed`.
- Apply that partial step once, include it in state, impulse, work, and step diagnostics, and terminate only when the scaled work residual is within tolerance.

For a frictionless interval, state is linear in partial impulse and the work equation is quadratic. A bounded root solve handles that case and the friction-coupled state uniformly without introducing a separate analytic branch.

### 5. Add private, allocation-free diagnostics

Introduce private stack-valued data structures near the solver:

- `RailImpactSolveSettings`: nominal impulse-step count plus hard phase/root bounds used by production and varied by internal convergence tests.
- `RailImpactPhaseDiagnostics`: accepted full/partial step count, accumulated mass-normalized impulse (`in/s`), signed or phase-normalized work (`(in/s)^2`), and final partial impulse (`in/s`).
- `RailImpactSolveDiagnostics`: nominal impulse step, compression and restitution phase diagnostics, compression normal-speed residual, restitution target work, and final work residual.
- `RailImpactSolveResult`: final `RailImpactFrameState` plus `RailImpactSolveDiagnostics`.

`solve_spin_aware_rail_impact_in_frame` returns `RailImpactSolveResult`. `spin_aware_ball_cushion_collision_on_table_from_basis` consumes `.state`; the adjacent private test module consumes `.diagnostics`. No heap trace or per-step vector is required.

The solver must assert finite progress, the expected phase sign, a bracketed partial root, and the hard step bound. Diagnostics are emitted only for a converged solve; non-convergence remains a deterministic failure rather than a plausible-looking partial rebound.

## API/data-model cutover and caller migration

- **Private cutover:** Replace the tuple return from compression and bare-state return from restitution with phase results that carry state and diagnostics. Replace the bare-state return from `solve_spin_aware_rail_impact_in_frame` with `RailImpactSolveResult`; do not retain parallel old/new private solve paths.
- **Immediate caller migration:** Update `spin_aware_ball_cushion_collision_on_table_from_basis` to use the result's final state for frame reconstruction. Keep diagnostics available to internal tests without cloning or allocating.
- **Public API stability:** Keep the signatures and return types of all four `collide_ball_rail_on_table*` functions unchanged. `RailCollisionConfig` is also unchanged.
- **Inherited callers:** Straight rails, pocket jaws, two-/N-ball event resolution, and bank tracing already converge on the shared basis solver. They require no alternate call path or compatibility shim; their SpinAware behavior changes atomically when the shared solver is replaced.
- **No mixed semantics:** Remove the old endpoint work helper, work-derived restitution step, fixed eight-step compression refinement, and constant-speed final-step calculation in the same change.

## Phased implementation plan

### Phase 1 — Add failing observable regressions

1. In `tests/rail_collisions.rs`, add `spin_aware_frictionless_restitution_matches_speed_and_e_sweep` using `rail_state_from_local_frame` and an explicit config with both friction coefficients and `a/R` set to zero.
2. Include the exact 90 in/s, `e = 1` reproducer first so the current approximately `-88.232 in/s` result is visible in the failure against `-90 in/s`.
3. Extend the same test over `q_in = [1, 30, 90] in/s` and `e_e = [0, 0.68, 1]`, asserting `q_out = -e_e q_in`, zero tangential speed, and unchanged zero angular velocity.
4. In `tests/rail_event_execution.rs`, add `spin_aware_zero_restitution_does_not_repeat_a_zero_time_rail_impact`: execute an immediate top-rail SpinAware impact at `e = 0`, verify the resolved local normal speed is not cushion-directed, then ask the scheduler for the next event and reject another zero-time impact for the same ball/rail.

### Phase 2 — Introduce diagnostic contracts and convergence tests

1. Add the private settings/result/diagnostic structs; do not yet expose them through public collision APIs.
2. Add an adjacent `#[cfg(test)] mod rail_impact_energetic_restitution_tests` with direct access to the private frame solve.
3. Lock down the dimensional step invariant: for fixed `e`, nominal step at 90 in/s is three times the step at 30 in/s, not nine times, and both are reported in mass-normalized impulse units.
4. Lock down equation (16): compression reaches the zero-speed root, `w_c > 0`, `w_r >= 0`, and `|w_r - e_e^2 w_c|` satisfies the scaled tolerance for every speed/restitution pair.
5. Add a smooth frictional refinement fixture and compare `N = 250, 500, 1000, 2000`. Measure final-state distance as

   \[
   d=\max(|\Delta v_t|,|\Delta v_n|,R|\Delta\omega_t|,R|\Delta\omega_n|,R|\Delta\omega_z|),
   \]

   so every term has `in/s` units. Require successive differences to decrease and `d(state_1000, state_2000) <= 0.05 in/s`.

### Phase 3 — Correct the integration

1. Split nominal resolution, hard step cap, and root-iteration bounds.
2. Compute the shared mass-normalized impulse step from incoming speed and configured restitution.
3. Replace endpoint work with the signed pre/post trapezoidal helper.
4. Replace compression's eight halvings with the bracketed zero-normal-speed partial solve and include that final partial work.
5. Replace restitution's work-derived step and mixed endpoint accounting with the shared impulse step and signed trapezoids.
6. Replace the final constant-speed division with a bracketed partial impulse that closes remaining trapezoidal work.
7. Count every accepted partial step and accumulated impulse in diagnostics; enforce finite values, phase signs, residual tolerances, and hard bounds.

### Phase 4 — Cut over the shared caller and smoke-test behavior

1. Migrate `spin_aware_ball_cushion_collision_on_table_from_basis` to the new solve result and remove the old solve/result path.
2. Run the private diagnostic tests, then the 90 in/s regression and speed/restitution sweep, then the zero-restitution event-progress regression.
3. Confirm the existing Mathavan low-speed slope, friction response, and all-rail local-frame invariance tests still pass before any documentation cleanup.

### Phase 5 — Documentation, generated-artifact policy, and aggregate verification

1. Update `whitepapers/rail_rebound.md` to document the equation (16a) trapezoid, mass-normalized impulse units, compression root, restitution target, and available internal phase diagnostics.
2. Update the `RailModel::SpinAware` rustdoc near `collide_ball_rail_on_table_with_radius_and_profile` so it says energetic restitution is enforced by trapezoidal work integration rather than merely describing a reduced Mathavan-style solve.
3. Do not edit the primary PDF, the historical audit, or generated `agent_knowledge` files. No corpus regeneration is needed because the source literature has not changed.
4. Run the relevant aggregate rail, event, pocket, and bank suites listed below.

## Regression and acceptance tests

All tolerances are on observable physical invariants, not source text or implementation names.

| Contract | Fixture | Required invariant |
|---|---|---|
| Exact elastic reproducer | `q_in = 90 in/s`, `e = 1`, `mu_w = mu_s = 0`, `a/R = 0`, zero tangent/spin | `q_out = -90 in/s` within `1e-9 in/s`; `v_t = 0`; every angular component remains zero. The old 88.23 in/s result must fail. |
| Speed/restitution homogeneity | `q_in = 1, 30, 90 in/s`; `e = 0, 0.68, 1`; otherwise frictionless | `q_out = -e q_in` within `max(1e-9 in/s, 1e-10 q_in)` for all nine cases. In particular 30 in/s at `e=.68` returns `-20.4 in/s`, and 90 in/s returns `-61.2 in/s`. |
| Equation (16) work closure | Same sweep through private diagnostics | `w_c > 0`; `abs(w_r - e^2 w_c) <= max(1e-10 w_c, 1e-12 (in/s)^2)`; all work and impulse diagnostics are finite. |
| Compression root | Every sweep case, including `e = 0` | `abs(q_compression_end) <= max(1e-10 q_in, 1e-12 in/s)`; the final compression partial impulse and its work are included in diagnostics. |
| Zero-restitution event progress | Ball touching top plane, incoming SpinAware response, `e = 0`, frictionless impact config | Resolved normal speed is zero within tolerance and a second scheduler call does not return another zero-time incoming impact for that ball and rail. |
| Impulse dimensions | Compare diagnostics at 30 and 90 in/s with fixed `e` | `delta_p_90 / delta_p_30 = 3` within floating tolerance; no step is derived from work or scales by 9. |
| Final partial step | Choose a case whose target is not an integer number of nominal steps | Final work residual meets the equation (16) tolerance, the partial impulse is in `[0, delta_p_nominal]`, and its post-state is the returned state. |
| Bounded convergence | Smooth frictional state at `N = 250, 500, 1000, 2000` | R-scaled state differences decrease with refinement; 1000-versus-2000 difference is at most `0.05 in/s`; each phase uses at most its hard cap. |
| Existing frictional behavior | Existing rail collision tests | Mathavan low-speed slope remains within its existing tolerance; increasing cushion/cloth friction still changes response in the tested direction; results remain local-frame invariant across all four rails. |
| Downstream stability | Existing event, jaw/pocket, and bank tests | No non-finite states, repeated zero-time SpinAware rail events, or bank-path nontermination; existing observable contracts pass without retuning coefficients. |

## Risks and edge cases

- **Slip-direction discontinuities:** Cushion or cloth slip can approach the adherence band during an impulse interval. The partial root must evaluate from one pre-step state, and the frictional convergence fixture must remain away from an exact slip transition so it tests numerical order rather than a branch coincidence.
- **Sign discipline at compression/restitution:** A work helper using `abs` can hide a step that crosses zero. Assert phase signs and allow only the compression-root step to end at zero.
- **`e = 0` and very small nonzero `e`:** Treat exact zero as the no-restitution boundary. For nonzero `e`, scale work tolerance to compression work rather than dropping a physically meaningful target merely because it is near `f64::EPSILON`.
- **`e = 1`:** Roundoff must not create energy gain or leave a work deficit. The final partial solve closes the configured target; the external elastic invariant catches both errors.
- **Very low incoming speed:** The minimum impulse step can exceed the total required impulse. The bracketed compression/root and restitution/work solves must therefore work when each phase consists of one partial interval.
- **Pathological friction values:** Public validation permits nonnegative friction without a small upper bound. Preserve a finite hard cap and require a bracket/progress assertion instead of returning a partial state when the normal response is non-monotone.
- **Mixed units in convergence comparisons:** Angular components must be multiplied by radius before comparison with translational velocity; do not compare raw radians-per-second numerically with inches-per-second.
- **Post-solve heuristics:** The frictionless fixture deliberately sets zero tangent/spin and `a/R = 0` so TP 7.3 and rolling-entry guardrails cannot mask the energetic result. Existing frictional tests protect the intended heuristic layer separately.
- **Downstream golden behavior:** Corrected rail speeds can move bank endpoints and pocket-jaw exits. Update expectations only when the new value follows the corrected source invariant; do not retune restitution or friction to preserve the old integration bias.
- **Model validity:** The speed sweep tops out at 90 in/s (2.286 m/s), inside the cited 2.5 m/s normal-speed limit. Do not use an out-of-domain high-speed result to accept or reject this correction.

## Source, documentation, and generated-artifact updates

- Update `whitepapers/rail_rebound.md` only after focused behavioral tests pass, replacing the work-trace open question with the actual internal diagnostic fields and the signed-work convention.
- Update the relevant `src/lib.rs` rustdoc and private comments with `p = P_I/M`, `u_I = q cos(theta)`, and the equation (16a) trapezoid. Include units in names/comments wherever raw `f64` is retained.
- Leave `physics_audits/2026-04-24-rail-pocket.md` unchanged as a historical record.
- Leave `whitepapers/a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf` unchanged.
- Never hand-edit `agent_knowledge/whitepapers_corpus.txt` or other generated knowledge artifacts. The evidence corpus already contains the governing source and requires no regeneration for a solver-only correction.

## Verification commands

Run focused tests first:

```text
cargo test --lib rail_impact_energetic_restitution_tests
cargo test --test rail_collisions spin_aware_frictionless_restitution_matches_speed_and_e_sweep -- --exact
cargo test --test rail_event_execution spin_aware_zero_restitution_does_not_repeat_a_zero_time_rail_impact -- --exact
cargo test --test rail_collisions mathavan_perpendicular_rolling_rebound_matches_low_speed_rigid_slope -- --exact
cargo test --test rail_collisions spin_aware_rail_collision_is_local_frame_invariant_across_rails -- --exact
```

Then run the relevant aggregate suite:

```text
cargo test --test rail_collisions --test rail_event_scheduling --test rail_event_execution --test n_ball_pockets --test table_and_pocket_geometry --test bank_paths
```

## Dependencies and overlap

- **Sibling plan:** `plans/airborne-table-boundary-geometry.md` owns z-aware rail/jaw eligibility, finite table-boundary geometry, and preservation of airborne state. This plan assumes a validated on-table contact and owns only the shared SpinAware response after that contact has been selected. The plans are behaviorally independent; if both touch a nearby event caller, preserve that ownership boundary rather than duplicating geometry or response logic.
- **No dependency on calibration work:** This correction should land before any future rail coefficient fitting so calibration is not performed against a speed-dependent numerical bias.
- **Shared jaw and bank callers:** Their response changes are an intentional consequence of fixing the common solver, not separate jaw-geometry or path-tracing projects.

## Candidate commit message

```text
Fix energetic restitution integration for rail impacts

Use dimensionally correct mass-normalized impulse steps and Mathavan's
trapezoidal work rule. Solve compression and restitution partial steps so
SpinAware rail and jaw rebounds honor the configured energetic restitution.
```
