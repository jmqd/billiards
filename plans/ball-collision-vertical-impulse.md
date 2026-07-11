# Preserve vertical ball-ball impulse through translation and event routing

Date: 2026-07-10

Severity: **HIGH**  
Priority: **P1**

## Problem statement

The non-ideal ball-ball solver computes a vertical contact-friction impulse and applies its rotational effect, but discards its equal-and-opposite center-of-mass translation by returning `OnTableBallState`. The result is not the result of the solver's own three-dimensional contact equations: follow/topspin impacts cannot launch the cue ball, the object ball is not sent into its immediate supporting-surface contact, and the advertised equal-solid-sphere no-slip cap does not zero the complete contact-relative velocity.

The implementation must preserve the existing free-sphere no-slip divisor `/7`. The correct repair is to represent all two translational and all two rotational contributions to tangential contact slip, not to change `/7` to `/5` to fit the currently truncated state.

## Current behavior and impact

### Observed behavior

1. `contact_friction_impulse_per_mass` computes the two-component tangential-plane impulse

   \[
   \mathbf j_{\parallel}=(j_t,j_z)
   =\min\left(\frac{\lVert\mathbf s\rVert}{7},\mu j_n\right)
     \frac{\mathbf s}{\lVert\mathbf s\rVert},
   \qquad
   j_n=\frac{1+e}{2}g_n.
   \]

2. `transferred_spin_from_contact_impulse` applies both `j_t` and `j_z` to angular velocity.
3. `frictional_collision_outcome_on_table_with_config` applies only `j_n` and `j_t` to planar `Velocity2`. It does not apply `j_z` to either ball's `vertical_velocity`.
4. `CollisionOutcome::{a_after,b_after}` are `OnTableBallState`, so the missing component cannot be represented even though `BallState` already has `height` and `vertical_velocity`.
5. Both the legacy on-table N-ball executor and the richer `NBallSystemState` executor call tuple helpers returning `OnTableBallState`, then write those values back as cloth-bound states. No `Airborne` or zero-time `BallTableBounce` route can be selected.

### Impact

- In gross slip, the returned full contact-relative vertical slip is wrong by the missing translational change `-2j_z`.
- At the no-slip cap, rotation changes the relative contact velocity by only the `5j_z` solid-sphere rotational contribution. The omitted two center translations leave a nonzero residual even though the solver chose `|s|/7` specifically to stop slip.
- The ball-ball pair's represented vertical momentum is not the momentum implied by the applied impulse.
- A hard natural-roll/follow impact remains on cloth instead of producing an upward cue-ball ballistic branch and an immediate downward object-ball/table contact.
- Subsequent scheduling can incorrectly choose an ordinary sliding/rolling transition, rail event, jaw event, or pocket event from a state that should first be airborne or undergo table contact.
- `CollisionModel::ThrowAware` and `CollisionModel::SpinFriction` are both affected because both call the same frictional solver. `CollisionModel::Ideal` has no tangential impulse and remains planar.

## Exact affected files, symbols, and callers

Line numbers describe the 2026-07-10 tree and should be re-grounded immediately before implementation.

### Core data model and direct collision API

- `src/lib.rs:1340-1360` — `CollisionOutcome`; `a_after` and `b_after` are currently `OnTableBallState`, and its Rustdoc says full vertical COM hop is out of scope.
- `src/lib.rs:1400-1419` — `CollisionAnalysis` and `PostContactContinuation`, which embed or expose `CollisionOutcome` and assume `a_after` is an on-table continuation.
- `src/lib.rs:1438-1447` — `PredictedBallBallCollision`; its impact inputs correctly remain `OnTableBallState` because this plan changes the response, not same-height on-table collision prediction.
- `src/lib.rs:2287-2305` — `TwoBallOnTableSimulation` and `NBallOnTableSimulation`, whose state types cannot represent a non-ideal response that leaves the table.
- `src/lib.rs:2307-2357` — `NBallSystemState::{OnTable,Airborne,Pocketed}`, `From<BallState>`, `as_on_table`, and `as_ball_state`; this is the existing state boundary that can route a full post-impact `BallState`.
- `src/lib.rs:2360-2417` — `NBallSystemEvent`, including the existing `BallTableBounce` event used for an immediate downward object/table contact.
- `src/lib.rs:2739-2761` — `MotionAdvance` and `OnTableStateError`; the error already distinguishes a nonzero vertical velocity from an on-table state.
- `src/lib.rs:2770-2837` — `BallState`, including `height` and `vertical_velocity`, plus the on-table constructors.
- `src/lib.rs:11578-11609` — `ideal_collision_outcome_on_table_with_config`; it must construct a full `BallState` with zero vertical velocity under the new outcome type.
- `src/lib.rs:11611-11636` — `contact_friction_impulse_per_mass`; preserve the `/7` cap exactly.
- `src/lib.rs:11639-11656` — `transferred_spin_from_contact_impulse`; preserve the angular impulse and sign convention.
- `src/lib.rs:11658-11829` — `frictional_collision_outcome_on_table_with_config`; it computes `vertical_contact_slip` and `vertical_impulse_per_mass` at lines 11692-11712, creates only `Velocity2` at lines 11719-11746, and rebuilds `OnTableBallState` at lines 11804-11814.
- `src/lib.rs:11832-11848` — shared `ThrowAware` and `SpinFriction` wrappers.
- `src/lib.rs:11850-11964` — all direct collision entry points:
  - `collide_ball_ball_detailed_on_table_with_config`
  - `collide_ball_ball_detailed_on_table_with_radius_and_config`
  - `collide_ball_ball_detailed_on_table`
  - `collide_ball_ball_on_table_with_config`
  - `collide_ball_ball_on_table_with_radius_and_config`
  - `collide_ball_ball_on_table`
- `src/lib.rs:11967-12001` — analyzed direct collision helpers.
- `src/lib.rs:12168-12332` — `CollisionOutcome` analysis helpers and `PostContactContinuation`; their on-table-only bend/curve and next-event methods must no longer flatten or assume an airborne branch.
- `src/bin/shot_probe.rs:359-370` — direct detailed-outcome consumer; it reads post-contact speed and heading and must read `BallState` directly and report/retain vertical velocity.

### Event execution and state routing

- `src/lib.rs:1961-2042` — `PocketAwareEventCache`; an `Airborne(BallState)` at height zero and negative `vertical_velocity` already reaches `settle_airborne_ball_on_next_table_contact`, producing a zero-time table-contact candidate.
- `src/lib.rs:2278-2283` — `NBallOnTableAdvance`; the legacy return type cannot carry `Airborne`.
- `src/lib.rs:2445-2459` — `NBallSystemAdvance` and `NBallSystemSimulation`; these are the authoritative richer result types.
- `src/lib.rs:4433-4552` — ballistic advance, `time_until_airborne_ball_reaches_table`, `AirborneTableContact`, `resolve_airborne_table_contact`, and `settle_airborne_ball_on_next_table_contact`. At `height=0` with downward `v_z`, the computed table-contact time is exactly zero; this existing path should apply the configured table normal/tangential response.
- `src/lib.rs:9413-9462` — collision-progress metrics. Variant changes currently count as progress; regression coverage must ensure the new ball-ball → zero-time table-contact sequence is not mistaken for a no-progress loop.
- `src/lib.rs:9465-9512` — `OnTableKinematicDelta` and `apply_on_table_kinematic_delta`; they omit `dv_z` by construction.
- `src/lib.rs:9698-9802` — legacy shared-contact resolution.
- `src/lib.rs:9804-9931` — richer shared-contact resolution; despite receiving `NBallSystemState`, it reduces snapshots and accumulated deltas back to on-table states and therefore also discards vertical response.
- `src/lib.rs:9933-10047` — `advance_to_next_n_ball_event_with_scheduler`; ordinary and simultaneous-disjoint pair paths write tuple results into `Vec<OnTableBallState>`.
- `src/lib.rs:10050-10689` — public legacy on-table advance/simulation wrappers. They must not accept a model capable of leaving the table and then silently force an on-table result.
- `src/lib.rs:10691-10715` — `advance_n_ball_system_without_event`, which already advances `Airborne` ballistically.
- `src/lib.rs:10722-10735` — richer event computation and its current airborne scheduling contract.
- `src/lib.rs:10742-10885` — `resolve_n_ball_system_event_with_physics_and_pockets_on_table`; the ordinary and simultaneous-disjoint branches at 10800-10823 explicitly wrap pair outputs in `NBallSystemState::OnTable`.
- `src/lib.rs:10888-11130` — richer advance and simulation APIs. These rebuild the event cache after every resolved event and are the correct end-to-end route for non-ideal collisions that can hop.
- `src/dsl.rs:353-457, 675-686` — production scenario/simulation callers already use the richer `NBallSystemState` API and should receive the corrected route without a parallel simulation path.

### Tests and direct consumers to migrate

- `tests/non_ideal_ball_collisions.rs:95-132` — contact-slip helper currently omits translational `v_z` and accepts only `OnTableBallState`.
- `tests/non_ideal_ball_collisions.rs:302-313` — non-ignored defect characterization `known_limitation_high_friction_follow_draw_leaves_vertical_slip_residual`; delete it once the source defect is fixed.
- `tests/non_ideal_ball_collisions.rs:315-325` — ignored correct contract `peskin_high_friction_no_slip_cap_zeroes_full_contact_slip`; make it active and compute full three-dimensional contact slip.
- `tests/non_ideal_ball_collisions.rs:514-730` — momentum, energy-passivity, restitution, and equal-spin-increment grids. Adapt them to `BallState`, include vertical translation in momentum/energy, and retain their planar assertions.
- `tests/non_ideal_ball_collisions.rs:741-839, 1179-1308, 1309+` — gearing, spin transfer, defaults, Kim opt-in, and throw regressions that consume `CollisionOutcome`.
- `tests/ball_collision_timing.rs:82-87, 214-219` and `tests/ball_collisions.rs:138-353` — tuple direct API callers; ideal outputs can be explicitly validated back to `OnTableBallState` where an on-table motion helper is used.
- `tests/whitepaper_validation_suite.rs:126-228` — direct ideal and non-ideal whitepaper anchors plus `CollisionAnalysis` behavior.
- `tests/next_events.rs:186-397` and `tests/rail_event_scheduling.rs:157-264` — staged `PostContactContinuation` users; migrate airborne-capable follow cases to the richer scheduler or explicitly handle `OnTableStateError` rather than treating an airborne cue as on cloth.
- `tests/n_ball_advance.rs:194-250, 373-496` — explicit non-ideal legacy N-ball execution and shared-contact expectations.
- `tests/n_ball_simulation.rs:133-172` — non-ideal until-rest call through the legacy on-table simulation API.
- `tests/n_ball_pockets.rs:175-265` — existing authoritative airborne/table-contact sequencing contract; extend this test module with ball-ball-generated flight and zero-time object-table contact.

## Whitepaper evidence

### Peskin: the missing translation is part of the same contact solve

Source: `whitepapers/collision_of_billiard_balls_in_3d_with_spin_and_friction.pdf`; extracted corpus locations are in `agent_knowledge/whitepapers_corpus.txt`.

- Eq. (3), corpus `13739-13769`, gives the two relative tangential surface-velocity components as `2V + rΩ_z` and `2W - rΩ_y`.
- Eqs. (15)-(19), corpus `13831-13848`, include both center translation and spin:

  \[
  m\dot V=-\mu f\cos\theta,\qquad
  m\dot W=-\mu f\sin\theta,
  \]
  \[
  \dot\Omega_y=\frac{2\mu f}{\alpha mr}\sin\theta,\qquad
  \dot\Omega_z=-\frac{2\mu f}{\alpha mr}\cos\theta.
  \]

- Eqs. (26) and (28), corpus `13879-13896`, integrate the vertical terms to

  \[
  \Delta W=-\frac{\mu P}{m}\sin\theta,
  \qquad
  \Delta\Omega_y=\frac{2\mu P}{\alpha mr}\sin\theta.
  \]

- For a solid sphere, `α=2/5`. Across two equal balls, the two COM changes contribute `2j` and the two rotational changes contribute `5j` to the relative tangential contact velocity. Therefore the stopping impulse is `s/7`; applying only the rotational part is internally inconsistent.
- Peskin's rolling-ball special case gives opposite nonzero vertical outgoing velocities in Eqs. (69)-(70), corpus `14179-14192`, and explicitly notes conservation of total three-dimensional linear momentum.
- Corpus `14214-14248` says the upward ball leaves the table, the downward ball immediately collides with the table, and each later supporting-surface interaction can alter horizontal velocity and angular velocity.

### Doménech: ball-ball and supporting-surface interactions are distinct ordered impulses

Source: `whitepapers/non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf`.

- The abstract, corpus `35259-35268`, defines a two-step model: ball-ball interaction followed by ball-supporting-surface interaction.
- Eqs. (26)-(29), corpus `35445-35472`, impose both horizontal and vertical no-slip equalities at the sphere contact and give the corresponding two impulse components.
- Corpus `35493-35520` separately resolves the rough horizontal supporting-surface impact. This supports keeping the free-sphere pair impulse and the object/table impulse as separate observable event stages.

The isolated extracted Doménech Eq. (2) elsewhere in the corpus is not used because the audit identified a `v_2x`/`v_2y` source or extraction typo. The equations and surrounding definitions above are the safe evidence.

### Kim: hard follow shots produce visible lift and a downward object/table reaction

Source: `whitepapers/collision_of_two_spinning_billiard_balls_and_the_role_of_table_friction.pdf`.

- Corpus `14278-14296` states the paper's static-object setup and reports visible cue-ball lift after head-on topspin collision.
- Corpus `14362-14384` estimates the ball-ball impulse as `μ M U_i`, the rise as

  \[
  h=\frac{\mu^2 U_i^2}{2g},
  \]

  and explains that the object ball receives the corresponding downward motion and instant table interaction.
- Eqs. (9), (11), and (17), corpus `14525-14618`, explicitly retain center vertical velocities and vertical impulse terms.

This plan uses Kim as corroboration for vertical translation and the table reaction. It does not broaden Kim's stationary-object/static-table-friction approximation to moving object balls.

## Numeric reproducers

### Existing deterministic no-slip fixture

Observed fixture: `tests/non_ideal_ball_collisions.rs:112-132, 302-325`.

- Radius: `R = 1.125 in`.
- Cue speed: `52.8 in/s = 1.34112 m/s`.
- Natural roll into a `30°` cut.
- Object ball initially stationary.
- `e = 1`, `μ = 1`.
- Initial contact slip: `s_t = -26.4 in/s`, `s_z = -45.726141 in/s`, so `|s| = 52.8 in/s`.
- No-slip impulse:

  \[
  (j_t,j_z)=\frac{(s_t,s_z)}7
  =(-3.7714286,-6.5323059)\ \mathrm{in/s}.
  \]

- Required COM updates under the current sign convention:

  \[
  \Delta v_{A,z}=-j_z=+6.5323059\ \mathrm{in/s},
  \qquad
  \Delta v_{B,z}=+j_z=-6.5323059\ \mathrm{in/s}.
  \]

- Translation changes relative vertical contact speed by `-2j_z = +13.0646118 in/s`; rotation changes it by `-5j_z = +32.6615295 in/s`. Together they cancel the initial `-45.726141 in/s` exactly.
- Current code applies only the rotational `+32.6615295 in/s` and observes the defect residual `-13.064611805662393 in/s`.

### Material 10 m/s hard-shot example

Use a head-on naturally rolling cue ball, stationary object ball, `e=1`, and configured realistic ball-ball friction `μ=0.06`.

- Incoming speed: `U = 10 m/s = 393.7007874 in/s`.
- `j_n = U = 10 m/s`.
- Initial vertical contact slip: `s_z = -U = -10 m/s`.
- No-slip cap: `|s|/7 = 1.4285714 m/s`.
- Coulomb cap: `μj_n = 0.6 m/s`, so gross slip applies and `j_z = -0.6 m/s = -23.6220472 in/s`.
- Pair response before table contact:

  \[
  v_{A,z}'=+0.6\ \mathrm{m/s},
  \qquad
  v_{B,z}'=-0.6\ \mathrm{m/s},
  \qquad
  v_{A,z}'+v_{B,z}'=0.
  \]

- Ignoring air drag, cue-ball maximum rise:

  \[
  h=\frac{(0.6\ \mathrm{m/s})^2}{2(9.80665\ \mathrm{m/s^2})}
   =0.01835\ \mathrm m
   =0.722\ \mathrm{in}.
  \]

- Time to apex is about `0.06118 s`; return to the table is about `0.12237 s` after the pair impact.
- The object ball is already at `height=0` with `v_z<0`, so its next `BallTableBounce` must have `time_until_contact=0`. With the current default `AirborneTableContactConfig` (`e_t=0.6`, 4 in/s settle threshold), the table stage—not the ball-ball stage—determines whether the object rebounds or settles and applies the table tangential impulse.

## Root cause

The solver was extended to compute the vertical component of the ball-ball tangential impulse for spin transfer, while its response type and event executor remained planar. `OnTableBallState` made the omission structural: the solver could calculate `j_z` and its torque but had nowhere to store `v_z`. The event layer then compounded the loss by wrapping every pair result as `NBallSystemState::OnTable`, even in the richer scheduler.

This is not a coefficient-calibration defect. `/7` is correct for two equal solid free spheres when both translational and rotational responses are represented. Replacing it with `/5` would tune the impulse to the truncated implementation and cease to model the cited free-sphere equations.

## Proposed design and clean cutover

The following is proposed design; it is not current behavior.

### Direct response type

Change the existing API in place rather than introducing parallel `*_3d` aliases:

```rust
pub struct CollisionOutcome {
    pub a_after: BallState,
    pub b_after: BallState,
    pub throw_angle_degrees: Option<f64>,
    pub transferred_spin: Option<AngularVelocity3>,
    pub diagnostics: Option<CollisionDiagnostics>,
}
```

The inputs remain `&OnTableBallState` because this solver still handles a same-height collision whose centers start on the resting table plane. The immediate pair outputs become `BallState` because either ball may leave the on-table state domain.

Change all six existing direct entry points in place:

- detailed helpers continue to return `CollisionOutcome` with full states;
- tuple helpers return `(BallState, BallState)`;
- no helper may normalize nonzero `vertical_velocity` to zero;
- no compatibility alias returning `OnTableBallState` is added;
- callers needing a guaranteed planar result must explicitly attempt `OnTableBallState::try_from` and handle `OnTableStateError::VerticalVelocityPresent`.

`CollisionDiagnostics::vertical_impulse_per_mass` remains the signed `j_z` used by translation and spin. Do not add a second diagnostic with a different sign convention.

### Kinematic update

For the existing solver convention, apply the pair impulse as

\[
\Delta v_{A,z}=-j_z,\qquad \Delta v_{B,z}=+j_z.
\]

Construct both immediate states at the unchanged contact position and `height=0`, preserving horizontal velocities and angular updates already calculated. Ideal or zero-vertical-slip results have `vertical_velocity=0` and remain convertible to `OnTableBallState`.

### Direct analysis and continuation APIs

- Change `PostContactContinuation::{cue_ball,struck_ball}` to expose `&BallState`.
- Make on-table-only continuation operations explicitly fallible by returning `Result<..., OnTableStateError>` when their selected branch has nonzero vertical velocity. They must not project or flatten a branch.
- Make `CollisionOutcome` bend/curve helpers and analyzed collision helpers explicitly validate `a_after` before calling `estimate_post_contact_cue_ball_*_on_table`. An airborne cue returns `OnTableStateError` instead of receiving immediate cloth-driven bend/curve analysis. Callers wanting the post-landing continuation must use the richer event scheduler.
- Adapt `CollisionAnalysis` consistently so it is produced only after successful on-table validation; do not encode “airborne” as an indistinguishable `None` alongside “no bend/curve”.

### Rich N-ball event routing

After resolving an ordinary or simultaneous-disjoint ball-ball event in `resolve_n_ball_system_event_with_physics_and_pockets_on_table`:

1. Convert each `CollisionOutcome` branch through the existing full-state classification boundary (`NBallSystemState::from` or one shared equivalent that preserves nonzero `v_z`).
2. `v_z=0` remains `OnTable`; upward or downward nonzero `v_z` becomes `Airborne`.
3. Do not resolve the downward object's table impulse inside the ball-ball response. Rebuild the event cache and let `settle_airborne_ball_on_next_table_contact` produce a distinct zero-time `NBallSystemEvent::BallTableBounce`.
4. This separation gives an observable sequence: ball-ball impulse first, external table impulse second. Pair momentum/contact-slip assertions apply before table contact; table-contact impulse and energy assertions apply to the second event.
5. Ensure the post-impact separating pair does not produce a synthetic `UnsupportedAirborneBallBallContact` before the downward object's zero-time table contact.
6. Preserve the cue ball as `Airborne` until its positive flight time expires; its next event must not be an ordinary on-cloth `MotionTransition`.

### Legacy on-table scheduler boundary

`NBallOnTableAdvance`, `NBallOnTableSimulation`, and `TwoBallOnTableSimulation` cannot represent the valid result of a general `ThrowAware`/`SpinFriction` collision. They must not flatten it.

- Make the legacy public advance/simulation entry points that accept `CollisionModel` explicitly reject an airborne-capable model through a typed result before execution, or remove those non-ideal entry paths in favor of the existing richer `NBallSystem*` API.
- Migrate every in-repository non-ideal caller to `NBallSystemState`, `NBallSystemAdvance`, and `NBallSystemSimulation` in the same change.
- Retain the legacy on-table types only for computations guaranteed to remain planar, including `CollisionModel::Ideal`.
- Do not panic only after a vertical impulse is discovered, silently clamp, or add a legacy alias. The public contract must tell callers up front that non-ideal event execution belongs in the richer state machine.

### Shared-contact boundary

This plan owns the full-state pair-response representation and vertical impulse sign. `plans/coupled-nonideal-shared-contacts.md` owns the coupled non-ideal multi-contact algorithm.

- The current pairwise shared fallback must not continue through `OnTableKinematicDelta` after this cutover.
- The shared-contact implementation must consume/produce a generalized full-state delta including `dv_z`, or directly produce `BallState`/`NBallSystemState` results.
- If the shared-contact plan lands first, use its coupled full-state output. If this plan lands first, reject non-ideal shared contact explicitly until that solver is available rather than discarding `dv_z`.
- Do not duplicate or redesign the coupled normal/friction solve here.

## Explicit non-goals

- Do not change the no-slip cap from `|s|/7` to `|s|/5`.
- Do not change restitution, ball-ball friction, Marlow friction, table restitution, or table-friction calibration.
- Do not implement a simultaneous Kim ball-ball/table contact solve in this change. The selected behavior is an ordered free-sphere pair impulse followed by the existing explicit supporting-surface event.
- Do not broaden Kim's stationary-object static-friction approximation to moving/sliding object balls; that source-domain issue belongs to its sibling plan.
- Do not implement general airborne ball-ball collision response. The existing unsupported diagnostic remains for later collisions while one or both centers are elevated.
- Do not change airborne rail, jaw, or pocket finite-envelope geometry; `plans/airborne-table-boundary-geometry.md` owns that work.
- Do not change collision detection, root finding, impact radius, contact normal, ideal equal-mass response, or rail response.
- Do not rewrite historical audit documents to look as though the defect never existed.
- Do not manually edit generated files under `agent_knowledge/`.

## Phased implementation plan

### Phase 1 — Lock the direct three-dimensional contract with failing tests

1. In `tests/non_ideal_ball_collisions.rs`, generalize the contact-slip helper to accept `&BallState` and include translational vertical relative speed:

   \[
   s_z=(v_{A,z}-v_{B,z})
       +R\left[n_y(\omega_{A,x}+\omega_{B,x})
                -n_x(\omega_{A,y}+\omega_{B,y})\right].
   \]

2. Remove `#[ignore]` from `peskin_high_friction_no_slip_cap_zeroes_full_contact_slip` and make it assert both components independently and `hypot(s_t,s_z) <= 1e-9 in/s`.
3. Delete `known_limitation_high_friction_follow_draw_leaves_vertical_slip_residual`; an obsolete defect characterization must not coexist with the corrected contract.
4. Add the exact 10 m/s direct response fixture and assert:
   - `j_z=-0.6 m/s` within unit-converted tolerance;
   - `a_after.vertical_velocity=+0.6 m/s` and `b_after.vertical_velocity=-0.6 m/s`;
   - both heights remain zero at the instantaneous pair-response boundary;
   - vertical pair momentum is conserved before table contact;
   - both full states classify as airborne/non-on-table;
   - the diagnostic still uses `/7` versus the Coulomb cap, not `/5`.
5. Update the kinetic-energy helper to include `v_z²` and retain all three spin components. Keep the energy-passivity grid as a full immediate pair-response check.
6. Extend the impulse grid to assert all three linear-momentum components when object/table coupling is off, normal restitution, equal angular increments, and non-reversal/zeroing of the complete two-component contact-slip vector.

### Phase 2 — Cut direct APIs over to full `BallState`

1. Change `CollisionOutcome::{a_after,b_after}` to `BallState`.
2. Change the tuple direct helpers to return `(BallState, BallState)` in place.
3. Change `ideal_collision_outcome_on_table_with_config` to construct full zero-height, zero-vertical-velocity states without routing through an on-table-only response builder.
4. In `frictional_collision_outcome_on_table_with_config`, apply `-j_z` to A and `+j_z` to B while retaining the current horizontal and angular impulse equations.
5. Preserve `CollisionDiagnostics` field names/units/signs and the exact `contact_friction_impulse_per_mass` `/7` expression.
6. Update Rustdoc to distinguish the immediate free-sphere pair response from the later table response.
7. Compile-migrate direct consumers in `src/bin/shot_probe.rs` and test modules. Convert back to `OnTableBallState` only in ideal/zero-vertical fixtures that immediately invoke an on-table motion function, and make that validation explicit.

### Phase 3 — Make analysis/continuation state-domain failures explicit

1. Change `PostContactContinuation` accessors to return `&BallState`.
2. Change its phase-aware on-table collision/event methods to validate the selected branch and return `OnTableStateError` for airborne state rather than projecting it.
3. Apply the same validation to `CollisionOutcome` bend/curve helpers and analyzed collision entry points.
4. Migrate `tests/next_events.rs`, `tests/rail_event_scheduling.rs`, and `tests/whitepaper_validation_suite.rs`:
   - retain on-table expectations only for fixtures with `v_z=0`;
   - use the richer N-ball system for fixtures whose follow spin creates a hop;
   - assert the explicit domain error when a test is specifically about an on-table-only convenience API.
5. Update `shot_probe` output/handling so a vertical post-contact branch is observable instead of being silently treated as cloth-bound.

### Phase 4 — Route pair outputs through `NBallSystemState` and table events

1. Add an event-level 10 m/s regression in `tests/n_ball_pockets.rs` using `advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table`.
2. Change ordinary pair resolution at `src/lib.rs:10800-10810` to store each full output through the shared system-state classifier.
3. Apply the same conversion to simultaneous-disjoint pair resolution at `src/lib.rs:10811-10823`.
4. Rebuild the existing pocket-aware cache normally; do not special-case the object table response inside the pair branch.
5. Assert the event/state sequence:
   - first event is the expected `BallBallCollision`;
   - cue is `Airborne` with `v_z≈+0.6 m/s`;
   - object is `Airborne` at `height=0` with `v_z≈-0.6 m/s` immediately after that event;
   - next event is `BallTableBounce` for the object at zero elapsed time, not `MotionTransition` or `UnsupportedAirborneBallBallContact`;
   - resolving the table event changes the object according to `AirborneTableContactConfig` while leaving the cue airborne;
   - the cue's future table-contact time matches `2v_z/g` before any unrelated earlier event.
6. Add a low/zero vertical-slip event fixture asserting it remains `OnTable` and produces no spurious zero-time bounce.
7. Verify zero-time progress handling permits the real ball-ball → object-table cascade and does not terminate or repeat it as a no-op.

### Phase 5 — Cut over legacy N-ball callers and shared-contact representation

1. Define and enforce the on-table scheduler boundary: legacy `NBallOnTable*`/`TwoBallOnTable*` execution is planar-only and returns a typed error or no longer accepts non-ideal models.
2. Migrate all repository `ThrowAware`/`SpinFriction` event/simulation callers in `tests/n_ball_advance.rs` and `tests/n_ball_simulation.rs` to the richer system APIs.
3. Keep ideal legacy scheduler tests to protect the useful planar API.
4. Coordinate with `plans/coupled-nonideal-shared-contacts.md` so shared resolution uses generalized/full-state deltas. Until coupled non-ideal shared output is available, reject that case explicitly; never adapt it back through `OnTableKinematicDelta`.
5. Retain the existing TP B.29 coupled ideal shared-contact behavior and its momentum/energy regressions; ideal contact should continue to produce zero `dv_z`.
6. Search every in-repository direct and scheduled collision callsite and remove assumptions that `a_after`/`b_after` implement `OnTableBallState` methods. No compatibility shim or duplicate response path remains.

### Phase 6 — Cleanup source comments and project records after behavior passes

1. Update `CollisionOutcome`, direct collision, `PostContactContinuation`, and richer scheduler Rustdoc to describe full immediate pair states and ordered table contact.
2. Replace the incorrect “vertical hop remains outside scope” statements at `src/lib.rs:1241-1249, 1340-1353, 11850-11875` with the implemented state/event contract.
3. Cite the actual Kim collision paper where hard-shot lift is discussed; keep Peskin and Doménech citations for the free-sphere impulse and two-step table interaction.
4. Update `PHYSICS_TODOS.md:128-137` so its Kim default-off decision no longer claims the paired vertical hop is omitted. Preserve its historical decision and date; add current code/test evidence rather than rewriting the past.
5. Leave `physics_audits/2026-04-24-ball-ball-collisions.md` as historical evidence.
6. Do not hand-edit `agent_knowledge/whitepapers_corpus.txt`, `whitepapers_index.jsonl`, or other generated artifacts. `plans/whitepaper-authority-manifest.md` owns authoritative manifest and regeneration changes.

## Regression and acceptance tests

### Direct response invariants

- **Full contact slip:** at the high-friction no-slip cap, reconstructed `s_t` and `s_z` are each zero within `1e-9 in/s`.
- **Gross-slip direction:** below the cap, `s_after · s_before > 0`, `|s_after| < |s_before|`, and friction never reverses the slip vector.
- **Three-dimensional pair momentum:** with object/table coupling off,

  \[
  v'_{A,x}+v'_{B,x}=v_{A,x}+v_{B,x},\quad
  v'_{A,y}+v'_{B,y}=v_{A,y}+v_{B,y},\quad
  v'_{A,z}+v'_{B,z}=v_{A,z}+v_{B,z}.
  \]

- **Normal restitution:** the post-impact normal separation speed remains `e` times the pre-impact closing speed.
- **Equal contact torques:** both equal balls receive the same ball-ball angular-velocity increment when object/table coupling is off.
- **Passivity:** full translational plus rotational kinetic energy after the pair impulse does not exceed the pre-impact value within the existing deterministic tolerance.
- **Planar compatibility:** ideal and zero-vertical-slip outcomes have exactly zero `v_z` and successfully validate as `OnTableBallState`.
- **No coefficient regression:** diagnostics continue to report `min(|s|/7, μj_n)`.

### Event-routing invariants

- The 10 m/s natural-roll fixture produces `+0.6/-0.6 m/s` pair vertical velocities before any table impulse.
- The downward object/table interaction is a distinct `BallTableBounce` with zero event time.
- The cue remains in the ballistic `Airborne` branch and does not receive cloth friction or an on-table motion transition before landing.
- The table event's momentum change is treated as an external impulse; pair vertical momentum is not incorrectly asserted across it.
- Resolving the table event uses the configured `AirborneTableContactConfig`, including rebound threshold and tangential impulse, rather than clamping `v_z` ad hoc.
- The ball-ball → zero-time table-contact cascade makes measurable state progress and is not repeated indefinitely.
- Ordinary simultaneous-disjoint pair collisions route each participating branch independently.
- Shared non-ideal contact either produces generalized full-state output from the sibling coupled solver or fails explicitly; it never discards `dv_z`.

### Current-test disposition

- Activate `peskin_high_friction_no_slip_cap_zeroes_full_contact_slip`.
- Remove `known_limitation_high_friction_follow_draw_leaves_vertical_slip_residual`.
- Preserve and extend `throw_aware_impulse_grid_preserves_peskin_contact_invariants`.
- Preserve and make three-dimensional `throw_aware_impulse_grid_does_not_create_total_kinetic_energy`.
- Preserve horizontal momentum, restitution, gearing, throw, transferred-spin, playing-condition, ideal collision, timing, and Kim opt-in tests.
- Extend the existing `airborne_ball_table_contact_is_scheduled_before_later_on_table_events` test family with a collision-generated airborne/table-contact sequence rather than creating a second table-bounce model.

## Risks and edge cases

- **Sign inversion:** the vertical slip basis is easy to reverse. Lock `Δv_A,z=-j_z`, `Δv_B,z=+j_z` with both the 30° no-slip fixture and 10 m/s head-on fixture before refactoring.
- **Premature table response:** resolving the downward object within `CollisionOutcome` would destroy the free-sphere pair momentum boundary and hide the external impulse. Keep it as the next zero-time event.
- **Zero-time loops:** object table contact occurs at `t=0`. State-variant or velocity changes must count as progress, and a post-impact separating pair must not be rediscovered as a collision/unsupported contact.
- **Numerical dust:** the existing numerical-zero-slip rule should continue to produce exactly zero impulse for negligible slip. Do not create microscopic airborne states by changing that tolerance as part of this work.
- **Threshold mismatch:** classification of nonzero post-impact vertical velocity and table rebound settling must use the established state/table-contact contracts; do not use an unrelated planar rest threshold to erase pair momentum.
- **Analysis misuse:** cloth-driven bend/curve helpers are invalid before an airborne cue lands. Their new fallible boundary must distinguish this from a valid on-table outcome with no predicted bend.
- **Kim coupling:** the optional table-coupled correction introduces an external horizontal impulse during the pair solve. Its separate source-domain plan must not obscure the vertical free-sphere update required here.
- **Shared contacts:** accumulating several pair impulses can create one net `dv_z`; routing pair-by-pair through `OnTableBallState` would still lose it and be order dependent. Consume the sibling full-state coupled result.
- **Airborne geometry:** once a cue hops, it can reach a rail, jaw, pocket, or another ball. Existing diagnostics/geometry remain authoritative; improvements belong to `plans/airborne-table-boundary-geometry.md`.
- **Public API break:** changing `CollisionOutcome` and tuple return types is intentional. A same-commit migration is safer than preserving a misleading compatibility surface that silently drops physics.

## Dependencies and overlap with sibling plans

- **`plans/coupled-nonideal-shared-contacts.md`** — dependency/overlap. This plan establishes full-state pair response and vertical signs; the sibling plan owns the coupled non-ideal shared-contact solve and must not use `OnTableKinematicDelta` to discard `dv_z`.
- **`plans/airborne-table-boundary-geometry.md`** — downstream overlap only. This plan creates legitimate airborne states and uses existing table contact; the sibling plan owns airborne rail/jaw/pocket finite-envelope scheduling and unsupported boundary diagnostics.
- **`plans/kim-correction-domain.md`** — independent but adjacent. It owns fail-closed restriction of the optional Kim static-table correction to its stationary-object source domain and its citation correction. This plan owns vertical free-sphere pair translation and support-event routing regardless of whether that opt-in correction is enabled.
- **`plans/reject-overlapping-ball-states.md`** — independent validation boundary. It owns rejecting invalid pre-existing `g<0` public N-ball/DSL inputs and bounded position-only roundoff recovery; this plan owns impulses and state routing from valid contact states and must not use overlap repair to erase vertical response.
- **`plans/whitepaper-authority-manifest.md`** — source-pipeline overlap. It owns authority metadata and generated `agent_knowledge` regeneration; this plan owns runtime physics/Rustdoc citations and must not manually alter generated artifacts.

Implementation ordering: land this full-state response contract before, or atomically with, the coupled non-ideal shared-contact representation. Airborne geometry and authority-manifest work can proceed independently because this plan consumes existing event/state interfaces and source locations.

## Verification commands

Run focused contracts first:

```sh
nix develop -c cargo test --test non_ideal_ball_collisions peskin_high_friction_no_slip_cap_zeroes_full_contact_slip -- --exact
nix develop -c cargo test --test non_ideal_ball_collisions hard_natural_roll_collision_preserves_vertical_pair_impulse -- --exact
nix develop -c cargo test --test n_ball_pockets ball_collision_hop_routes_object_into_immediate_table_contact -- --exact
```

Then run the complete touched behavioral suites:

```sh
nix develop -c cargo test --test non_ideal_ball_collisions
nix develop -c cargo test --test n_ball_pockets
nix develop -c cargo test --test n_ball_advance
nix develop -c cargo test --test n_ball_simulation
nix develop -c cargo test --test next_events
nix develop -c cargo test --test rail_event_scheduling
nix develop -c cargo test --test ball_collisions
nix develop -c cargo test --test ball_collision_timing
nix develop -c cargo test --test whitepaper_validation_suite
```

Finally run the relevant aggregate test target after all callsite migrations:

```sh
nix develop -c cargo test --tests
```

## Candidate commit message

```text
Preserve vertical ball collision impulse through event routing

Return full post-impact ball states, apply the equal-and-opposite vertical
COM impulse without changing the /7 no-slip cap, and route cue hops and
zero-time object/table contacts through the N-ball system scheduler.
```
