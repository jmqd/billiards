# Restore TP B.2 rolling turn while side spin outlasts translation

Date: 2026-07-10

Severity: Medium

Priority: P2

## Problem statement

The rolling branch of `raw_advance_within_phase_on_table` applies the repository's TP B.2 side-spin turn only when vertical-axis spin is predicted to stop strictly before translational rolling stops. When side spin instead survives until or beyond the translational stop, the branch advances the ball in a straight line for the entire rolling interval, including finite times at which the ball has nonzero speed and nonzero side spin. The public curve estimator repeats the same lifetime gate and returns `None`.

That gate does not follow TP B.2. Within the model already selected by this repository, the instantaneous turn exists whenever the ball is rolling with finite forward speed and active side spin. Whether the spin will stop before or after translation is not an input to the force or heading at an earlier time.

This plan fixes that finite-time lifetime bug while treating TP B.2's separate near-stop singularity explicitly. It does not use the singularity at `v -> 0` to suppress valid curvature at finite speed.

## Observed current behavior and impact

The following are observed facts in the current tree:

- `raw_advance_within_phase_on_table` computes
  - `t_roll = initial_speed / rolling_linear_deceleration`,
  - `t_spin = |wz| / spin_angular_deceleration`,
  - but enters the curved branch only when `0 < t_spin < t_roll` (`src/lib.rs:4692-4761`, especially `4711-4752`).
- If `t_spin >= t_roll`, the branch uses the initial heading for the entire rolling displacement (`src/lib.rs:4744-4751`). Vertical spin still decays, so the returned spin lifetime and returned planar trajectory disagree about whether side spin was present.
- `estimate_rolling_side_spin_curve_on_table` has the same `time_until_spin_stops >= time_until_translation_stops => None` rejection (`src/lib.rs:12075-12116`, especially `12086-12100`).
- The public estimator documentation is stale: it says rolling remains straight even though the low-spin branch already curves (`src/lib.rs:12118-12133`).
- Existing tests encode the discontinuity:
  - `advancing_a_rolling_ball_with_vertical_spin_no_longer_curls_once_it_is_in_pure_rolling_motion` expects a straight path for `wz = +6 rad/s` (`tests/advance_ball_state.rs:528-551`).
  - `advancing_a_rolling_ball_with_side_spin_follows_the_tp_b2_curve` expects a curved path for the otherwise identical `wz = +2 rad/s` state (`tests/advance_ball_state.rs:622-650`).
  - `rolling_side_spin_curve_estimate_is_none_when_translation_stops_before_spin` codifies the estimator rejection for `wz = +6 rad/s` (`tests/advance_ball_state.rs:706-719`).
  - `advancing_a_rolling_ball_with_vertical_spin_can_enter_the_spinning_phase` consequently expects the high-spin ball to stop at the straight-line endpoint (`tests/advance_ball_state.rs:721-742`).
- `PHYSICS_TODOS.md:56-64` says TP B.2 rolling turn was integrated into actual motion. That completion claim is only true for the `t_spin < t_roll` branch.

The user-visible effects are:

1. Stronger English can curve less than weaker English. With all other state equal, increasing `|wz|` across `alpha_z * t_roll` abruptly changes a curved trajectory into a straight trajectory.
2. State advancement, sampled paths, trace rendering, collision outcome continuation, and curve metadata inherit the wrong straight path.
3. The public `Option<PostContactCueBallCurve>` uses `None` to mean “no curve” even when a finite interval of TP B.2 curvature exists and is merely truncated by the linear-motion/model threshold.
4. The rolling-to-spinning transition time and residual `wz` are already correct, but their endpoint position is wrong because the preceding rolling path was suppressed.

## Exact affected files, symbols, and callers

### Implementation source

- `src/lib.rs:4623-4644`
  - `advance_vertical_axis_spin_f64`
  - `time_until_vertical_axis_spin_stops_f64`
- `src/lib.rs:4646-4799`
  - `raw_advance_within_phase_on_table`
  - defective rolling lifetime gate at `4711-4752`
  - no-slip horizontal-spin reconstruction at `4762-4788`, which must remain intact
- `src/lib.rs:4801-4867`
  - `raw_compute_next_transition_on_table`
  - rolling-to-rest versus rolling-to-spinning classification must not change
- `src/lib.rs:5110-5262`
  - `advance_within_phase_on_table`
  - `advance_motion_on_table`
  - `try_advance_ball_state`
  - `advance_ball_state`
  - `advance_spin_on_table`
  - `try_advance_angular_velocity_on_table`
  - `advance_angular_velocity_on_table`
- `src/lib.rs:12008-12073`
  - `rolling_resistance_center_of_pressure_angle_radians`
  - `rolling_side_spin_turn_coefficient`
  - `rolling_side_spin_curved_displacement`
- `src/lib.rs:12075-12165`
  - `estimate_rolling_side_spin_curve_on_table`
  - `estimate_post_contact_cue_ball_curve_on_table`
- `src/lib.rs:12168-12233`
  - `CollisionOutcome::estimate_post_contact_cue_ball_curve`
  - `CollisionOutcome::with_post_contact_cue_ball_analysis`
  - `CollisionOutcome::with_post_contact_cue_ball_bend`

### Propagating consumers

These callers should receive the corrected state through the canonical advancement API; they should not grow a second rolling-turn implementation:

- `BallPath::sampled_points` (`src/lib.rs:2480-2551`).
- `advance_on_table_ball_without_event`, two-ball/N-ball advancement, path tracing, and rendered path sampling (`src/lib.rs:8974-9006`, `11284-11355`, `15059-15122`).
- Scenario timeline construction and sampling (`src/dsl.rs:628-653`, `1304-1352`).
- Existing raw advancement callers that construct a state at a separately predicted event time, including ball-ball and rail impact state construction and pocket gap evaluation (`src/lib.rs:5820-5825`, `5944-5951`, `7304-7307`, `8297-8309`). Their event-root algorithms are a separate concern described under non-goals.

### Tests and source records

- `tests/advance_ball_state.rs:1-143`, `528-742`, and `744-767`.
- `tests/motion_transitions.rs:239-340` for unchanged rolling transition contracts.
- `tests/non_ideal_ball_collisions.rs:1564-1640` for public estimator wrappers and no-curve cases.
- `PHYSICS_TODOS.md:56-64` for the incomplete completion record.
- `whitepapers/tp_b_2_rolling_resistance_spin_resistance_and_ball_turn.pdf` for the governing model.
- `agent_knowledge/whitepapers_corpus.txt:42586-42674` for Cross's qualitative corroboration and calibration caveat.
- `agent_knowledge/whitepapers_corpus.txt:51193-51242` for the independent rolling and z-spin laws used by the current reduced model.

## Whitepaper evidence and governing equations

### TP B.2 is the repository's selected turn model

The direct source is `whitepapers/tp_b_2_rolling_resistance_spin_resistance_and_ball_turn.pdf`.

- Equation (2), extracted at PDF lines `40-46`, defines the effective rolling-resistance relation

  $$
  \mu_r = \frac{a}{g}.
  $$

- Equations (12)-(16), PDF lines `165-219`, derive the acute center-of-pressure angle from

  $$
  -\mu_r\cos\theta + \sin\theta = \frac{2}{5}\mu_r.
  $$

  The current helper evaluates the equivalent acute solution

  $$
  \theta = \arcsin\!\left(\frac{(2/5)\mu_r}{\sqrt{1+\mu_r^2}}\right) + \arctan(\mu_r).
  $$

- Equations (17)-(19), PDF lines `221-237`, use the measured spin-down angular deceleration and solid-sphere inertia:

  $$
  T_s = I\alpha_s, \qquad I=\frac{2}{5}mR^2.
  $$

- Equations (21)-(23), PDF lines `275-295`, give

  $$
  F_t=\frac{mv^2}{\rho}, \qquad \Omega_t=\frac{v}{\rho}, \qquad F_t=mv\Omega_t.
  $$

- Equation (30), with the same expression restated as a function of speed in Equation (32), PDF lines `331-377`, gives

  $$
  \Omega_t(v)=\frac{T_s\sin\theta}{mvR\left(\frac{2}{5}+\cos\theta\right)}.
  $$

  Substituting `T_s=(2/5)mR^2 alpha_s` yields the exact form used by the repository:

  $$
  \Omega_t(v)=\frac{K}{v}, \qquad
  K=\frac{(2/5)R\alpha_s\sin\theta}{(2/5)+\cos\theta}.
  $$

- Equations (33)-(35), PDF lines `391-417`, use

  $$
  a=\mu_r g, \qquad v(t)=v_0-at,
  $$

  and integrate the turn rate over travel time:

  $$
  \theta_t(v_0,L)=\int_0^{t(v_0,L)}\Omega_t(v(v_0,t))\,dt.
  $$

For a finite interval ending at `v_1 > 0`, the signed heading change is therefore

$$
\Delta\psi
=\operatorname{sgn}(\omega_z)\frac{K}{a}
 \ln\!\left(\frac{v_0}{v_1}\right).
$$

Let

$$
q=\operatorname{sgn}(\omega_z)\frac{K}{a},\quad
\lambda=\ln(v_0/v_1),\quad
\delta=q\lambda,\quad
D=\frac{v_0^2}{a},\quad
c=e^{-2\lambda}=\left(\frac{v_1}{v_0}\right)^2.
$$

The existing analytic displacement helper evaluates

$$
I_c=\frac{D\left[c(-2\cos\delta+q\sin\delta)+2\right]}{4+q^2},
$$

$$
I_s=\frac{D\left[c(-2\sin\delta-q\cos\delta)+q\right]}{4+q^2}.
$$

For initial unit heading `h=(h_x,h_y)`, its repository-coordinate displacement and finite-speed final heading are

$$
\Delta x=h_xI_c+h_yI_s,\qquad
\Delta y=h_yI_c-h_xI_s,
$$

$$
h_1=(h_x\cos\delta+h_y\sin\delta,
      -h_x\sin\delta+h_y\cos\delta).
$$

The lifetime comparison `t_spin < t_roll` appears nowhere in these instantaneous equations. It only determines whether the turn ends because side spin decays or because translational speed reaches the model's lower domain boundary.

### The source does not justify evaluating a heading at exactly zero speed

Because `Omega_t=K/v`, the heading integral diverges logarithmically as `v -> 0`. TP B.2 is explicit that the center-of-pressure and spin-down torque treatment is assumed (`PDF:221-249`) and calls the predicted effect small (`PDF:427-443`). The implementation must not claim a physically meaningful final heading at exactly zero speed.

The position integral nevertheless has a finite limit:

$$
\lim_{v_1\to0^+}I_c=\frac{2D}{4+q^2},\qquad
\lim_{v_1\to0^+}I_s=\frac{qD}{4+q^2}.
$$

This permits a finite translational endpoint if a caller configures a zero linear-speed threshold, even though no terminal heading should be inferred once velocity is zero.

### Independent corroboration and its limits

- Cross reports that a near-vertical-axis spinning ball curves in the direction set by spin, and that spin can persist after forward motion stops (`agent_knowledge/whitepapers_corpus.txt:42608-42621`). It also states that curvature increases with initial spin (`:42624-42630`). This independently rejects the current qualitative result in which longer-lived spin removes all curvature.
- Cross used a `22 mm` golf ball on low-pile carpet and measured `mu ~= 0.05` and `alpha ~= 54 rad/s^2` (`:42663-42674`). Those values are not pool-ball/cloth calibration data and must not replace repository configuration.
- Petit gives constant-direction rolling deceleration (`agent_knowledge/whitepapers_corpus.txt:51193-51222`) and a separate linear vertical-spin decay law during either sliding or rolling (`:51223-51242`). This supports the current simultaneous translation/spin evolution and the possibility that `t_spin > t_roll`; it does not support suppressing the earlier turn.

The fix is therefore an internal consistency correction for the chosen TP B.2 model, not a claim that TP B.2 is a high-precision empirical law near rest.

## Numeric discontinuity reproducer

Use the existing `tests/advance_ball_state.rs` fixture and configuration:

- ball radius `R = 1.125 in`;
- rolling deceleration `a = 5 in/s^2`;
- vertical-spin deceleration `alpha_s = 2 rad/s^2`;
- initial center `(x_0,y_0) = (10,20) in`;
- initial velocity `(v_x,v_y) = (0,10) in/s`;
- no-slip horizontal spin
  `omega_x = -10/R = -8.88888888888889 rad/s`, `omega_y = 0 rad/s`;
- compare `omega_z = +2 rad/s` and `omega_z = +6 rad/s`;
- advance `dt = 1 s`.

For both states,

$$
t_{roll}=\frac{10\ \mathrm{in/s}}{5\ \mathrm{in/s^2}}=2\ \mathrm{s}.
$$

The low-spin state has `t_spin = 2/2 = 1 s`; the high-spin state has `t_spin = 6/2 = 3 s`. Side spin is active throughout the open interval `[0,1)` for both, and the high-spin state still has `omega_z=4 rad/s` at `t=1 s`. Both have `v_1=5 in/s`.

Under the repository's current TP B.2 coefficient and analytic integral, both therefore accumulate

- heading change `Delta psi = +0.09257711527464842 deg`;
- endpoint `(10.004702077725524, 27.499997782973136) in`;
- speed `5 in/s`;
- residual `omega_z` of `0 rad/s` for the low-spin state and `4 rad/s` for the high-spin state;
- no-slip horizontal spin reconstructed from the curved velocity.

Current behavior is correct for `omega_z=+2 rad/s` but returns, for `omega_z=+6 rad/s`:

- heading change `0 deg`;
- endpoint `(10,27.5) in`;
- lateral error `0.004702077725524 in` after one second.

The code's branch boundary is

$$
|\omega_z|=\alpha_s t_{roll}
=(2\ \mathrm{rad/s^2})(2\ \mathrm{s})=4\ \mathrm{rad/s}.
$$

For a fixed `dt < t_roll`, states immediately below and above `4 rad/s` both retain side spin through `dt`; their position and heading must be continuous across that lifetime boundary. The current strict `< t_roll` gate instead changes abruptly to the straight branch at and above `4 rad/s`.

## Root cause

The previous TP B.2 integration patch conflated two independent questions:

1. **Finite-time applicability:** Is the ball currently rolling with `v > 0` and active side spin? If yes, TP B.2 supplies a turn rate for that interval.
2. **How the curve ends:** Does active spin end first, or does translation reach the configured near-rest/model cutoff first?

`raw_advance_within_phase_on_table` answers the first question by checking the answer to the second. The estimator copied the same predicate. This makes a future lifetime ordering erase valid earlier dynamics.

A second local contributor is that `rolling_side_spin_curved_displacement` currently falls back to straight motion when `final_speed <= f64::EPSILON` (`src/lib.rs:12036-12047`). That fallback protects the logarithm but discards the finite displacement limit. Near-stop handling must be explicit rather than silently switching the entire interval to a straight path.

## Non-goals

- Do not change ball-ball, rail, jaw, pocket, or scheduler event-root algorithms. Those currently solve straight/quadratic trajectories in places where advancement is curved. Their migration belongs to `plans/curved-rolling-event-detection.md`.
- Do not widen event tolerances, inflate collision radii, add scans, or otherwise compensate for event predictors in this change.
- Do not change motion-transition times or the existing `Rolling -> Rest` versus `Rolling -> Spinning` decision in `raw_compute_next_transition_on_table`.
- Do not recalibrate rolling resistance, spin angular deceleration, ball radius, or phase thresholds.
- Do not replace the chosen TP B.2 model with Cross's golf-ball/carpet model, and do not treat Cross's coefficients as pool-cloth calibration.
- Do not add massé spin, cue-elevation swerve, or alter cue-strike spin decomposition.
- Do not change sliding dynamics, z-spin decay signs/rates, or the no-slip relation `wx=-vy/R`, `wy=vx/R`.
- Do not edit generated `agent_knowledge` files by hand.

## Proposed behavior and threshold policy

The following is proposed design, not current behavior.

### 1. Finite rolling-turn interval

For a rolling input with `v_0 > 0`, define

- `t_requested = min(dt, t_roll)` as today;
- `t_spin = |wz| / alpha_s` when `|wz| > f64::EPSILON`;
- `v_floor = motion.phase.thresholds.rest_linear_speed`;
- `t_floor = (v_0-v_floor)/a` when `0 < v_floor < v_0`;
- `t_floor = 0` when `v_floor >= v_0`;
- `t_floor = t_roll` when the configured threshold is exactly zero.

Then define

$$
t_{curve}=\min(t_{requested},t_{spin},t_{floor}).
$$

The critical rule is that `t_curve` is computed whenever side spin is active. It is not conditioned on `t_spin < t_roll`.

Apply the analytic TP B.2 displacement over `t_curve`. If `t_requested > t_curve`, advance the remainder with constant heading from the finite curve endpoint. This remainder represents either motion after side spin stopped or motion below the configured TP B.2 linear-speed domain floor.

The z-spin update remains based on the full `t_requested`, not only `t_curve`. Thus a high-spin ball can curve until the linear model cutoff, stop translating, and continue in `MotionPhase::Spinning` with the already-correct residual `wz`.

### 2. Near-stop handling is not the finite-time gate

- If `v_floor > 0`, stop evaluating `ln(v_0/v)` at `v_floor`. Continue the tiny remaining translational distance straight along the last finite heading. This gives the configured threshold an observable and testable meaning and keeps all returned values finite.
- If a caller explicitly configures `v_floor == 0` and the curved interval reaches `t_roll`, evaluate the analytic displacement limit above. Return zero planar velocity and zero horizontal rolling spin, so no final heading is needed by state advancement. Do not evaluate `ln(v_0/0)` and do not fall back to the initial straight line.
- Retain the estimator's existing angular threshold: initial `|wz| <= rest_angular_speed` yields no reported curve. Zero side spin remains exact straight rolling. This change does not invent curvature from thresholded-away spin.
- Treat equality consistently: spin that reaches zero exactly at the requested endpoint contributes curvature over the preceding open interval; it is not a reason to choose a straight path.

### 3. Share the interval decision

Introduce one small private, allocation-free helper or equivalent single block of logic that computes the rolling-turn end duration/speed from `v_0`, `wz`, the maximum duration, and `OnTableMotionConfig`. Both the rolling integrator and estimator must use it. The helper must return numeric facts such as `curve_time`, `end_speed`, and whether a positive-speed heading is reportable; it must not own public state or duplicate the displacement integral.

This shared decision prevents the estimator and state solver from reintroducing different lifetime/threshold predicates. Keep the existing `f64` hot-path representation; no heap allocation or decimal conversion belongs in the raw integrator.

## API and data-model cutover

No public signature or struct-layout change is required for this defect.

- Preserve `PostContactCueBallCurve` and its existing fields:
  - `time_until_curve_starts`;
  - `time_until_curve_completes`;
  - `curve_angle_degrees`;
  - `heading_after_curve`.
- Redefine and document `time_until_curve_completes` as the end of the reportable TP B.2 curved interval: either the side-spin stop or the configured positive linear-speed cutoff, whichever comes first. It must not imply that residual `wz` is zero.
- For `t_spin > t_floor` with a positive threshold, return `Some(PostContactCueBallCurve)` containing the partial/truncated finite-speed curve instead of `None`.
- `curve_angle_degrees` and `heading_after_curve` report the finite heading at that interval endpoint. The estimator must not claim a heading at exactly zero speed. For an explicitly zero linear-speed threshold whose side spin outlasts translation, state advancement still uses the finite displacement limit; the angle-oriented estimator remains outside its reportable domain and must document that exceptional `None` result rather than describing it as “no curve.”
- Preserve the sliding caller's time offset: when `estimate_post_contact_cue_ball_curve_on_table` advances a sliding state to rolling first, both start and completion times remain relative to the original input state (`src/lib.rs:12141-12159`).
- `CollisionOutcome` convenience methods and `CollisionAnalysis::cue_ball_curve` require no code-shape migration; they inherit the corrected `Some` value. Their tests must cover that wrappers do not discard a threshold-truncated estimate.
- All advancement, trace, and rendering callers continue to call the canonical advancement functions. No compatibility shim, alternate estimator, or duplicated TP B.2 API should remain.

## Phased implementation plan

### Phase 1: Add failing finite-time regression tests

In `tests/advance_ball_state.rs`:

1. Rename
   `advancing_a_rolling_ball_with_vertical_spin_no_longer_curls_once_it_is_in_pure_rolling_motion`
   to
   `advancing_a_rolling_ball_with_vertical_spin_follows_tp_b2_before_translation_stops`.
2. Keep its `wz=+6 rad/s`, `dt=1 s` fixture, but replace straight-line expectations with the observed low-spin TP B.2 values:
   - `x=10.004702077725524 in`;
   - `y=27.499997782973136 in`;
   - speed `5 in/s`;
   - heading `+0.09257711527464842 deg`;
   - `wz=4 rad/s`;
   - `wx=-vy/R`, `wy=vx/R` within `1e-12`;
   - `MotionPhase::Rolling`.
3. Add `tp_b2_rolling_turn_is_continuous_across_spin_translation_lifetime_boundary`. Construct otherwise identical states with `wz=4-1e-6 rad/s` and `wz=4+1e-6 rad/s`, advance each by `1 s`, and assert their `x`, `y`, velocity heading, and speed agree within `1e-12`. Also assert both agree with the finite-time TP B.2 endpoint above. This test is deliberately before `t_roll`; it tests the lifetime bug without pretending TP B.2 has a finite heading at `v=0`.
4. Add `opposite_rolling_side_spin_produces_mirrored_motion`. For `wz=+6` and `-6 rad/s` at `dt=1 s`, assert equal `y` and speed, equal-and-opposite `x-10`, equal-and-opposite signed heading, opposite residual `wz`, and no-slip horizontal spin for both.
5. Retain `advancing_a_rolling_ball_updates_position_speed_and_spin_consistently` as the explicit `wz=0` straight-line identity contract. Tighten its assertions if needed so position, heading components, zero `wz`, and no-slip spin are all observable.

These tests must fail against the current `t_spin < t_roll` gate.

### Phase 2: Correct the rolling integrator

In `src/lib.rs`:

1. Remove the outer requirement `side_spin_stop_time < stop_time` from the rolling branch of `raw_advance_within_phase_on_table`.
2. Compute `t_curve` from requested phase duration, spin lifetime, and the independent near-stop/model cutoff as specified above.
3. Call `rolling_side_spin_curved_displacement` for every positive-duration, positive-speed TP B.2 interval.
4. Advance any post-curve remainder straight from the curve endpoint heading.
5. Extend `rolling_side_spin_curved_displacement` to evaluate the finite displacement limit when the final speed is zero instead of returning the initial straight-line displacement. Ensure the caller ignores a terminal heading when the returned speed is zero.
6. Preserve:
   - linear speed decay over the full rolling advance;
   - vertical spin decay over the full rolling advance;
   - `vx=vy=0` and `wx=wy=0` at translational stop;
   - `wx=-vy/R` and `wy=vx/R` at positive speed;
   - the existing transition time and residual-spin phase decision.
7. Keep all new raw calculations finite and allocation-free. Assert or branch before division/logarithm rather than relying on NaN/Infinity cleanup.

Run the Phase 1 tests before changing estimator behavior so the state solver is independently proven.

### Phase 3: Specify and test near-stop threshold behavior

In `tests/advance_ball_state.rs`:

1. Add `tp_b2_rolling_turn_stops_at_the_configured_linear_speed_threshold` using the existing `v0=10 in/s`, `a=5 in/s^2`, `wz=+6 rad/s` state but set `rest_linear_speed=2 in/s`.
2. The TP B.2 interval must end at
   `t_floor=(10-2)/5=1.6 s`, with heading
   `+0.21495740489983212 deg`.
3. Advance to `1.8 s`. Assert the heading is still the `1.6 s` cutoff heading, final speed is `1 in/s`, and the endpoint is the curved displacement to the cutoff plus `0.3 in` of straight travel:
   - `x=10.010813961614733 in`;
   - `y=29.899989411533657 in`.
4. Add or extend a zero-threshold unit test around `rolling_side_spin_curved_displacement` in the existing `src/lib.rs` test module if direct helper testing is the local convention. Assert the exact-stop position is finite and equals the analytic displacement limit, and that no logarithm of zero or non-finite state escapes. Do not assert a stopped-ball heading.
5. Update `advancing_a_rolling_ball_with_vertical_spin_can_enter_the_spinning_phase` to expect the corrected curved endpoint while preserving speed `0`, `wx=wy=0`, residual `wz=1 rad/s` at `2.5 s`, and `MotionPhase::Spinning`. Under the default positive threshold, the endpoint should be derived by the same helper rather than duplicated production constants; hard-code the independently calculated expected values in the test so the test can catch a regression.

This phase is intentionally separate from the finite-time boundary test. A future change to the near-stop cutoff policy must not be able to reintroduce the `t_spin >= t_roll => straight for all t` bug.

### Phase 4: Fix estimator semantics and tests

In `src/lib.rs` and `tests/advance_ball_state.rs`:

1. Make `estimate_rolling_side_spin_curve_on_table` use the shared curved-interval decision.
2. Remove `time_until_spin_stops >= time_until_translation_stops` as a no-curve condition.
3. Keep `speed <= rest_linear_speed`, `|wz| <= rest_angular_speed`, unavailable heading, and numerically zero turn coefficient as legitimate `None` conditions.
4. For a positive linear threshold reached before spin stops, return a partial `PostContactCueBallCurve` whose completion time, angle, and heading are evaluated at the threshold.
5. Rename
   `rolling_side_spin_curve_estimate_is_none_when_translation_stops_before_spin`
   to
   `rolling_side_spin_curve_estimate_is_truncated_when_spin_outlasts_translation`.
6. Use the explicit `rest_linear_speed=2 in/s` fixture so the expected partial estimate is not dominated by the default near-zero threshold. Assert:
   - `Some`;
   - `time_until_curve_starts=0 s`;
   - `time_until_curve_completes=1.6 s`;
   - `curve_angle_degrees=+0.21495740489983212 deg`;
   - `heading_after_curve=+0.21495740489983212 deg`;
   - the input's physical `t_spin=3 s` is later than completion, proving completion does not mean spin stopped.
7. Keep `the_curve_estimate_reports_tp_b2_rolling_side_spin_turn`, `tp_b2_rolling_side_spin_curve_estimate_matches_published_examples`, and `opposite_rolling_side_spin_turns_the_other_way` unchanged except for any shared helper setup. They protect the low-spin and published-distance cases.
8. In `tests/non_ideal_ball_collisions.rs`, add `collision_analysis_preserves_a_threshold_truncated_rolling_curve_estimate` only if a collision fixture can naturally produce a rolling cue-ball state with residual `wz` and `t_spin>t_floor`; otherwise test the `CollisionOutcome` wrapper with the smallest existing outcome fixture and avoid manufacturing collision physics solely to exercise field forwarding.

### Phase 5: Migrate callers and source descriptions

1. Confirm every public advancement wrapper listed above reaches the corrected raw rolling branch. Do not add per-caller conditionals.
2. Confirm `BallPath::sampled_points` and scenario timeline sampling use corrected advancement for interior samples; add a sampled-path assertion only if existing integration coverage exposes the former straight segment without depending on event timing.
3. Update the stale rustdoc at `src/lib.rs:12118-12133`:
   - rolling advancement itself uses TP B.2;
   - the estimator summarizes the same modeled interval;
   - completion can mean spin stop or the configured finite-speed cutoff;
   - TP B.2 is not evaluated for a heading at exact rest.
4. Amend `PHYSICS_TODOS.md:56-64` with the follow-up correction date and the formerly omitted `t_spin >= t_roll` branch. Do not erase the historical record of the first integration.
5. Do not edit `agent_knowledge/whitepapers_corpus.txt` or `agent_knowledge/whitepapers_formula_candidates.txt`; they are evidence/generated artifacts, not implementation sources. No whitepaper PDF or corpus regeneration is required because the authoritative source content is unchanged.

## Regression and acceptance tests

The implementation is accepted only when all of these observable invariants hold:

1. **Finite-time lifetime independence:** For fixed sign and a requested `dt` during which side spin remains active and speed stays above the model floor, position and heading do not depend on whether `t_spin` will eventually be less than, equal to, or greater than `t_roll`.
2. **Boundary continuity:** At fixed `dt<t_roll`, perturbing initial `|wz|` across `alpha_s*t_roll` does not jump between curved and straight paths.
3. **Numeric reproducer:** The `wz=+6 rad/s`, `dt=1 s` fixture reaches `(10.004702077725524,27.499997782973136) in` and `+0.09257711527464842 deg`, with residual `wz=4 rad/s`.
4. **Sign symmetry:** Negating `wz` mirrors lateral displacement and signed heading while preserving forward displacement and speed.
5. **Zero-spin identity:** `wz=0` remains the existing straight rolling solution exactly within current tolerances.
6. **No-slip invariant:** Every positive-speed rolling result satisfies `wx=-vy/R` and `wy=vx/R`; every zero-speed result has `wx=wy=0`.
7. **Spin invariant:** `wz` still decays linearly at the configured angular deceleration and can outlast translation into `MotionPhase::Spinning`.
8. **Transition invariant:** `compute_next_transition_on_table` continues to report the rolling stop at `v0/a` and chooses `Rest` or `Spinning` from residual `wz` using the existing angular threshold.
9. **Threshold invariant:** With `rest_linear_speed=2 in/s`, turn evaluation ends at `1.6 s`; later sub-stop translation is straight from the finite cutoff heading.
10. **Near-stop safety:** Exact-stop advancement returns finite position, zero planar velocity, and no NaN/Infinity even when the configured linear threshold is zero. No stopped heading is asserted.
11. **Estimator invariant:** The high-spin case with a positive cutoff returns a partial `Some` estimate rather than `None`; the completion time is the cutoff, not the spin-stop time.
12. **Existing calibration invariants:** The TP B.2 `2 mph/8 ft` and `5 mph/3 ft` published-example tests retain their current tolerances and results.
13. **Caller invariant:** Public advancement and collision-analysis convenience wrappers expose the same corrected state/metadata as the direct on-table functions; no caller retains the removed lifetime gate.

## Risks and edge cases

- **TP B.2 singularity at rest:** `Omega_t=K/v` does not define a finite stopped-ball heading. The positive configured cutoff and zero-threshold displacement limit must remain separate code paths.
- **Threshold sensitivity:** A very small positive `rest_linear_speed` produces a larger logarithmic reported angle. This is expected from TP B.2, should be documented, and is not a reason to restore the all-or-nothing lifetime gate.
- **Zero configured threshold:** State advancement can use the finite displacement limit, but the current angle-oriented estimator cannot truthfully report a terminal heading. Keep this exceptional domain behavior explicit. Do not substitute an arbitrary large angle.
- **Equality and roundoff:** `t_spin==t_roll`, `dt==t_spin`, and `dt==t_floor` must not take contradictory branches. Clamp computed durations to `[0,t_requested]` and speeds to nonnegative values once, then use the same values for displacement, velocity, and metadata.
- **Very small side spin:** Preserve the existing angular threshold in the estimator and exact-zero behavior in advancement. Avoid signing a zero coefficient into a spurious turn.
- **Sign convention:** Positive `wz` in the existing northward fixture produces positive `x` and a positive angle from north. Keep this repository convention; do not infer sign from an external diagram without matching axes.
- **Phase recursion:** `advance_motion_on_table` recursively consumes a rolling stop followed by spinning time. Correcting the rolling endpoint must not move the ball during the spinning remainder.
- **Event inconsistency remains until sibling work lands:** Correct advancement can make existing straight/quadratic event roots visibly inconsistent. Do not hide that issue here; coordinate merge order with `plans/curved-rolling-event-detection.md` and run its focused event tests after both changes are integrated.
- **Public estimator semantics:** Existing consumers may have interpreted `time_until_curve_completes` as “side spin is zero.” Rustdoc and tests must explicitly cut over that interpretation to “reportable TP B.2 interval ends.”
- **Performance:** This branch is on a hot sampling/simulation path. Reuse scalar helpers and avoid allocations, collections, decimal conversions, or numerical quadrature.

## Source, documentation, and generated-artifact updates

- Update `src/lib.rs` rustdoc for `estimate_post_contact_cue_ball_curve_on_table` and any nearby helper comment that calls rolling motion straight or calls the estimator analysis-only.
- Amend the existing `PHYSICS_TODOS.md:56-64` completion entry to record that the original cutover omitted spin lifetimes that reach/exceed translational stop and that this follow-up closes that branch.
- Keep direct source citations to TP B.2 equations and label its near-stop behavior/model assumptions accurately.
- Do not alter whitepaper PDFs, HTML sources, or generated `agent_knowledge` artifacts. The issue is implementation inconsistency, not missing corpus extraction.
- No README, DSL syntax, serialized schema, or changelog change is required unless the repository has a release-note mechanism that specifically tracks public semantic changes; the public Rust signature remains unchanged.

## Verification commands

Run focused regressions first, then the affected integration targets, then the relevant aggregate suite:

```sh
nix develop -c cargo test --test advance_ball_state advancing_a_rolling_ball_with_vertical_spin_follows_tp_b2_before_translation_stops -- --exact
nix develop -c cargo test --test advance_ball_state tp_b2_rolling_turn_is_continuous_across_spin_translation_lifetime_boundary -- --exact
nix develop -c cargo test --test advance_ball_state opposite_rolling_side_spin_produces_mirrored_motion -- --exact
nix develop -c cargo test --test advance_ball_state tp_b2_rolling_turn_stops_at_the_configured_linear_speed_threshold -- --exact
nix develop -c cargo test --test advance_ball_state rolling_side_spin_curve_estimate_is_truncated_when_spin_outlasts_translation -- --exact
nix develop -c cargo test --test advance_ball_state advancing_a_rolling_ball_with_vertical_spin_can_enter_the_spinning_phase -- --exact
nix develop -c cargo test --test advance_ball_state
nix develop -c cargo test --test motion_transitions
nix develop -c cargo test --test non_ideal_ball_collisions
nix develop -c cargo test --test whitepaper_validation_suite
nix develop -c cargo test
```

After `plans/curved-rolling-event-detection.md` is implemented, also run that plan's focused ball-ball/rail/jaw event tests to verify that scheduling and advancement use the same corrected curved trajectory. Event-test failure before that sibling implementation is evidence of the known overlap, not permission to weaken this plan's finite-time advancement assertions.

## Dependencies and overlap

- **Direct overlap:** `plans/curved-rolling-event-detection.md` migrates event roots to the canonical curved rolling trajectory. This plan owns the canonical free-motion lifetime gate, near-stop handling, and estimator. The event plan owns ball-ball/rail/jaw/pocket timing and root validation. Neither plan should duplicate the other's solver.
- **Historical dependency:** `PHYSICS_TODOS.md:56-64` is the original TP B.2 path-integration work. This plan is a corrective completion of its omitted branch, not a return to estimator-only motion.
- **Merge order:** Prefer landing this plan's canonical trajectory behavior before or together with curved event detection, so the event plan roots the final trajectory contract. If event detection lands first, its trajectory primitive must still consume the lifetime behavior specified here rather than preserving the defective gate.
- **No dependency:** Cue-impact/massé spin decomposition, transition scheduling itself, coefficient calibration, and source-corpus authority changes are outside this plan.

## Candidate commit message

```text
Restore rolling turn while side spin outlasts translation

Apply TP B.2 curvature over every finite-speed interval with active
side spin, truncate angle metadata at the configured linear-speed
cutoff, and preserve the finite displacement limit at exact rest.
```
